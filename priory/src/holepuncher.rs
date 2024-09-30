use anyhow::{Context, Result};
use libp2p::{
    core::{
        multiaddr::{Multiaddr, Protocol},
        PeerId,
    },
    gossipsub::{self, Message},
    identify,
    swarm::SwarmEvent,
};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc::Receiver;
use tracing::info;

use crate::swarm_client::SwarmClient;
use crate::{find_ipv4, MyBehaviourEvent, Peer, I_HAVE_RELAYS_PREFIX, WANT_RELAY_FOR_PREFIX};

pub enum HolepunchEvent {
    GossipsubMessage { message: Message },
    IdentifySent,
    IdentifyReceived,
    DcutrConnectionSuccessful { remote_peer_id: PeerId },
    DcutrConnectionFailed { remote_peer_id: PeerId },
}

impl HolepunchEvent {
    pub fn try_from_swarm_event(event: &SwarmEvent<MyBehaviourEvent>) -> Option<HolepunchEvent> {
        match event {
            SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                message,
                ..
            })) => Some(HolepunchEvent::GossipsubMessage {
                message: message.clone(),
            }),
            SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Sent { .. })) => {
                Some(HolepunchEvent::IdentifySent)
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {
                ..
            })) => Some(HolepunchEvent::IdentifyReceived),
            SwarmEvent::Behaviour(MyBehaviourEvent::Dcutr(event)) => {
                let remote_peer_id = event.remote_peer_id;
                match event.result {
                    Ok(_) => Some(HolepunchEvent::DcutrConnectionSuccessful { remote_peer_id }),
                    Err(_) => Some(HolepunchEvent::DcutrConnectionFailed { remote_peer_id }),
                }
            }
            _ => None,
        }
    }
}

pub async fn watch_for_holepunch_request(
    swarm_client: SwarmClient,
    receiver: &mut Receiver<PeerId>,
    event_receiver: &mut Receiver<HolepunchEvent>,
) -> Result<()> {
    loop {
        // loop until there's a request to holepunch
        let holepunch_target = receiver
            .recv()
            .await
            .context("hole punch request sender shouldn't drop")
            .unwrap();

        // try to hole punch
        // TODO: do we need to block or anything fancy?  We just want to attempt one hole punch at
        // a time
        holepunch(holepunch_target, event_receiver, &swarm_client).await?;
    }
}

pub async fn holepunch(
    target_peer_id: PeerId,
    event_receiver: &mut Receiver<HolepunchEvent>,
    swarm_client: &SwarmClient,
) -> Result<()> {
    let query = format!("{WANT_RELAY_FOR_PREFIX}{target_peer_id}");
    swarm_client.gossipsub_publish(query).await.unwrap();

    // Wait until we hear a response from a relay claiming they know this target_peer_id (or timeout)
    let mut possible_relays: Vec<Multiaddr> = Vec::new();
    // TODO: add a timeout (in case nobody is connected to this node)
    loop {
        if let HolepunchEvent::GossipsubMessage { message, .. } = event_receiver
            .recv()
            .await
            .context("event sender shouldn't drop")
            .unwrap()
        {
            let message = String::from_utf8_lossy(&message.data);
            // should respond with {prefix}{target_target_peer_id} {relay_multiaddr}
            if let Some(str) = message.strip_prefix(I_HAVE_RELAYS_PREFIX) {
                let str: Vec<&str> = str.split(" ").collect();
                assert!(
                    !str.is_empty(),
                    "must return at least its own target_peer_id"
                );

                // peer doesn't have any relays or isn't willing to share
                if str.len() == 1 {
                    break;
                }

                let target_target_peer_id: PeerId = str[0].parse().unwrap();

                // if the message is about the peer we care about, break and try to dial that
                // multiaddr
                if target_target_peer_id == target_peer_id {
                    // add all the relays to the list
                    for multiaddr_str in str.iter().skip(1) {
                        // skip localhost addrs
                        if find_ipv4(multiaddr_str) == Some("127.0.0.1".into()) {
                            continue;
                        }

                        possible_relays.push(multiaddr_str.parse().unwrap());
                    }
                    info!("Found relays for peer {}", target_peer_id);
                    break;
                }
            }
        }
    }

    let my_relays = swarm_client.my_relays().await.unwrap();

    // first check if we already are connected to any of these relays
    let (common_relays, possible_relays) = compare_relay_lists(my_relays, possible_relays);

    for relay in common_relays {
        let relay_address_with_target_peer_id = relay.multiaddr.with_p2p(relay.peer_id).unwrap();
        // attempt to holepunch with one of the relays we know
        if exec_holepunch(
            relay_address_with_target_peer_id.clone(),
            target_peer_id,
            event_receiver,
            swarm_client,
        )
        .await
        .unwrap()
        {
            info!("\nHOLEPUNCH SUCCESSFUL\n");
            return Ok(());
        }
    }

    // TODO: we should only attempt to dial the non-common relays
    for relay_address in possible_relays {
        // attempt to holepunch to the target peer with the relay we just connected to
        if exec_holepunch(relay_address, target_peer_id, event_receiver, swarm_client)
            .await
            .unwrap()
        {
            info!("\nHOLEPUNCH SUCCESSFUL\n");
            break;
        }
    }

    Ok(())
}

async fn exec_holepunch(
    relay_address: Multiaddr,
    target_peer_id: PeerId,
    event_receiver: &mut Receiver<HolepunchEvent>,
    swarm_client: &SwarmClient,
) -> Result<bool> {
    // attempt to hole punch to the node we failed to dial earlier
    let multiaddr = relay_address
        .with(Protocol::P2pCircuit)
        .with_p2p(target_peer_id)
        .unwrap();
    swarm_client.dial(multiaddr).await.unwrap();

    info!(peer = ?target_peer_id, "Attempting to hole punch");

    // TODO: add a timeout
    loop {
        match event_receiver
            .recv()
            .await
            .context("event sender shouldn't drop")
            .unwrap()
        {
            // dcutr events.  If its successful break out of the for loop, if its a failure
            // break out of this loop
            HolepunchEvent::DcutrConnectionSuccessful { remote_peer_id } => {
                if remote_peer_id == target_peer_id {
                    return Ok(true);
                }
            }
            HolepunchEvent::DcutrConnectionFailed { remote_peer_id } => {
                if remote_peer_id == target_peer_id {
                    // holepunch unsuccessful
                    return Ok(false);
                }
            }

            // ignore other events
            _ => (),
        }
    }
}

// returns a vector of relays (as peers) that the two lists have in common
// AND a vector of relays (multiaddr) that we aren't connected with (to dial)
fn compare_relay_lists(
    my_relays: HashSet<Peer>,
    their_relays: Vec<Multiaddr>,
) -> (Vec<Peer>, Vec<Multiaddr>) {
    let mut common_relays = Vec::new();
    let mut relays_to_dial = Vec::new();

    let my_relay_map: HashMap<Multiaddr, Peer> = my_relays
        .iter()
        .map(|peer| (peer.multiaddr.clone(), peer.clone()))
        .collect();

    // Iterate through their_relays once
    for multiaddr in their_relays.iter() {
        if let Some(peer) = my_relay_map.get(multiaddr) {
            common_relays.push(peer.clone());
        } else {
            relays_to_dial.push(multiaddr.clone());
        }
    }

    (common_relays, relays_to_dial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_same_relay_lists() {
        let peer_id_a = PeerId::random();

        let my_relays = vec![Peer {
            multiaddr: "/ip4/127.0.0.1/tcp/4001".parse().unwrap(),
            peer_id: peer_id_a,
        }]
        .into_iter()
        .collect();

        let their_relays = vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()];

        let (common_relays, relays_to_dial) = compare_relay_lists(my_relays, their_relays);

        assert_eq!(common_relays.len(), 1);
        assert!(relays_to_dial.is_empty());
        assert!(common_relays.contains(&Peer {
            multiaddr: "/ip4/127.0.0.1/tcp/4001".parse().unwrap(),
            peer_id: peer_id_a
        }));
    }

    #[test]
    fn test_empty_my_relay_list() {
        let my_relays = HashSet::new();
        let their_relays = vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()];

        let (common_relays, relays_to_dial) = compare_relay_lists(my_relays, their_relays);

        assert!(common_relays.is_empty());
        assert_eq!(relays_to_dial.len(), 1);
        let multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().unwrap();
        assert!(relays_to_dial.contains(&multiaddr));
    }

    #[test]
    fn test_tattered_relay_lists() {
        let peer_id_a = PeerId::random();
        let peer_id_b = PeerId::random();

        let my_relays = vec![
            Peer {
                multiaddr: "/ip4/127.0.0.1/tcp/4001".parse().unwrap(),
                peer_id: peer_id_a,
            },
            Peer {
                multiaddr: "/ip4/142.93.2.49/tcp/4021".parse().unwrap(),
                peer_id: peer_id_b,
            },
        ]
        .into_iter()
        .collect();

        let their_relays = vec![
            "/ip4/127.0.0.1/tcp/4001".parse().unwrap(),
            "/ip4/142.93.53.125/tcp/4021".parse().unwrap(),
        ];

        let (common_relays, relays_to_dial) = compare_relay_lists(my_relays, their_relays);

        assert_eq!(common_relays.len(), 1);
        assert_eq!(relays_to_dial.len(), 1);
        assert!(common_relays.contains(&Peer {
            multiaddr: "/ip4/127.0.0.1/tcp/4001".parse().unwrap(),
            peer_id: peer_id_a,
        }));
        let multiaddr = "/ip4/142.93.53.125/tcp/4021".parse().unwrap();
        assert!(relays_to_dial.contains(&multiaddr));
    }

    #[test]
    fn test_find_ipv4() {
        assert_eq!(find_ipv4("ip4/1000"), Some("1000".into()));

        assert_eq!(find_ipv4("ip4/1000/abc/hello"), Some("1000".into()));

        assert_eq!(find_ipv4("helllo world/ip4/1000"), Some("1000".into()));

        assert_eq!(find_ipv4("helllo world/"), None);

        assert_eq!(find_ipv4(""), None);
    }
}
