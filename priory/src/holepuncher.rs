use anyhow::{anyhow, Context, Result};
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
use tracing::{debug, info, instrument, trace, warn};

use crate::swarm_client::SwarmClient;
use crate::{find_ipv4, MyBehaviourEvent, Peer, I_HAVE_RELAYS_PREFIX, WANT_RELAY_FOR_PREFIX};

#[derive(Debug)]
pub enum HolepunchEvent {
    GossipsubMessage { message: Message },
    IdentifySent,
    IdentifyReceived,
    DcutrConnectionSuccessful { remote_peer_id: PeerId },
    DcutrConnectionFailed { remote_peer_id: PeerId },
    ConnectionEstablished,
    OutgoingConnectionError,
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
            SwarmEvent::ConnectionEstablished { .. } => Some(HolepunchEvent::ConnectionEstablished),
            SwarmEvent::OutgoingConnectionError { .. } => {
                Some(HolepunchEvent::OutgoingConnectionError)
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
            .context("hole punch request sender shouldn't drop")?;

        // try to hole punch
        // TODO: do we need to block or anything fancy?  We just want to attempt one hole punch at
        // a time
        holepunch(holepunch_target, event_receiver, &swarm_client)
            .await
            .context("holepunch for {holepunch_target}")?;
    }
}

#[instrument(skip(event_receiver, swarm_client))]
pub async fn holepunch(
    target_peer_id: PeerId,
    event_receiver: &mut Receiver<HolepunchEvent>,
    swarm_client: &SwarmClient,
) -> Result<()> {
    info!("initiating holepunch");

    let query = format!("{WANT_RELAY_FOR_PREFIX}{target_peer_id}");
    swarm_client.gossipsub_publish(query).await?;

    // Wait until we hear a response from a relay claiming they know this target_peer_id (or timeout)
    let mut possible_relays: Vec<Multiaddr> = Vec::new();
    // TODO: add a timeout (in case nobody is connected to this node)
    loop {
        if let HolepunchEvent::GossipsubMessage { message, .. } = event_receiver
            .recv()
            .await
            .context("holepunch event sender shouldn't drop")?
        {
            let message = String::from_utf8_lossy(&message.data);
            // should respond with {prefix}{target_target_peer_id} {relay_multiaddr}
            if let Some(str) = message.strip_prefix(I_HAVE_RELAYS_PREFIX) {
                let str: Vec<&str> = str.split(" ").collect();

                // peer doesn't have any relays or isn't willing to share
                // TODO: == 1 or <= 1??
                if str.len() <= 1 {
                    warn!("Hole punch target responded with 0 relay addresses.  Holepunch unsuccessful.",);
                    return Ok(());
                }

                // TODO: how to ensure the message came from the target peer?
                let responded_peer_id: PeerId = str
                    .first()
                    .context("get the responded peer id")?
                    .parse()
                    .context("parse responsded peer id into PeerId")?;

                // if the message is about the peer we care about, break and try to dial that
                // multiaddr
                if responded_peer_id == target_peer_id {
                    // add all the relays to the list
                    for multiaddr_str in str.iter().skip(1) {
                        // skip localhost addrs
                        if find_ipv4(multiaddr_str) == Some("127.0.0.1".into()) {
                            continue;
                        }

                        possible_relays.push(
                            multiaddr_str
                                .parse()
                                .context("parse relay addr str as Multiaddr")?,
                        );
                    }
                    info!("Peer responded with its relays");
                    debug!(?possible_relays);
                    break;
                }
            }
        }
    }

    let my_relays = swarm_client.my_relays().await?;
    trace!(?my_relays);

    // first check if we already are connected to any of these relays
    let (common_relays, possible_relays) = compare_relay_lists(my_relays, possible_relays);
    debug!(
        "Have {} relays in common with the holepunch target",
        common_relays.len()
    );
    trace!(?common_relays);
    trace!(?possible_relays);

    for relay in common_relays {
        debug!(?relay, "Using common relay for holepunch");

        let relay_address_with_target_peer_id =
            if let Ok(multiaddr) = relay.clone().multiaddr.with_p2p(relay.peer_id) {
                multiaddr
            } else {
                return Err(anyhow!(
                    "Couldn't add peer_id {} onto the end of multiaddr {}",
                    relay.peer_id,
                    relay.multiaddr
                ));
            };

        // attempt to holepunch with one of the relays we know
        if exec_holepunch(
            relay_address_with_target_peer_id.clone(),
            target_peer_id,
            event_receiver,
            swarm_client,
        )
        .await?
        {
            return Ok(());
        }
    }

    for relay_address in possible_relays {
        debug!(relay=%relay_address, "Dialing possible relay for holepunch target");
        swarm_client
            .dial(relay_address.clone())
            .await
            .context(format!(
                "Dial possible relay {} for holepunching to target peer {}",
                relay_address, target_peer_id,
            ))?;

        // wait until we make or don't make connection
        loop {
            match event_receiver
                .recv()
                .await
                .context("holepunch event receiver shouldn't drop")?
            {
                HolepunchEvent::ConnectionEstablished | HolepunchEvent::OutgoingConnectionError => {
                    // TODO: how to ensure if this peer_id is the relays peer_id?
                    break;
                }
                _ => continue,
            }
        }

        // attempt to holepunch to the target peer with the relay we just connected to
        if exec_holepunch(relay_address, target_peer_id, event_receiver, swarm_client).await? {
            break;
        }
    }

    Ok(())
}

#[instrument(skip(event_receiver, swarm_client))]
async fn exec_holepunch(
    relay_addr: Multiaddr,
    target_peer_id: PeerId,
    event_receiver: &mut Receiver<HolepunchEvent>,
    swarm_client: &SwarmClient,
) -> Result<bool> {
    // attempt to hole punch to the node we failed to dial earlier
    let multiaddr = if let Ok(multiaddr) = relay_addr
        .clone()
        .with(Protocol::P2pCircuit)
        .with_p2p(target_peer_id)
    {
        multiaddr
    } else {
        return Err(anyhow!(
            "Couldn't add peer_id {} onto the end of multiaddr {}",
            target_peer_id,
            relay_addr
        ));
    };

    info!("Attempting to holepunch");
    swarm_client.dial(multiaddr).await?;

    // TODO: add a timeout
    loop {
        match event_receiver
            .recv()
            .await
            .context("event sender shouldn't drop")?
        {
            // dcutr events.  If its successful break out of the for loop, if its a failure
            // break out of this loop
            HolepunchEvent::DcutrConnectionSuccessful { remote_peer_id } => {
                if remote_peer_id == target_peer_id {
                    info!("Holepunch successful");
                    return Ok(true);
                }
            }
            HolepunchEvent::DcutrConnectionFailed { remote_peer_id } => {
                if remote_peer_id == target_peer_id {
                    warn!("Holepunch unsuccessful");
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
