use crate::bootstrap::BootstrapEvent;
use crate::holepuncher::HolepunchEvent;
use crate::{
    find_ipv4, MyBehaviourEvent, P2pNode, Peer, I_HAVE_RELAYS_PREFIX, WANT_RELAY_FOR_PREFIX,
};
use anyhow::Result;
use libp2p::{
    core::{multiaddr::Protocol, ConnectedPoint, PeerId},
    gossipsub::{self, IdentTopic, Message},
    identify,
    kad::{self, BootstrapError, BootstrapOk},
    mdns,
    swarm::SwarmEvent,
};
use std::collections::HashSet;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

const RELAY_SERVER_PROTOCOL_ID: &str = "/libp2p/circuit/relay/0.2.0/hop";

pub async fn handle_swarm_event(
    p2p_node: &mut P2pNode,
    event: SwarmEvent<MyBehaviourEvent>,
    bootstrap_event_sender: &Sender<BootstrapEvent>,
    holepunch_event_sender: &Sender<HolepunchEvent>,
    holepunch_req_sender: &Sender<PeerId>,
) -> Result<()> {
    // make sure we're still bootstrapping
    if !bootstrap_event_sender.is_closed() {
        // if it's a bootstrap event, send the relevant info to the bootstrap thread
        if let Some(bootstrap_event) = BootstrapEvent::try_from_swarm_event(&event) {
            bootstrap_event_sender.send(bootstrap_event).await.unwrap();
        }
    }

    // if it's an event that holepuncher cares about, send the relevant info to the holepuncher
    // thread
    if let Some(holepunch_event) = HolepunchEvent::try_from_swarm_event(&event) {
        holepunch_event_sender.send(holepunch_event).await.unwrap();
    }

    handle_common_event(p2p_node, event, holepunch_req_sender).await
}

pub async fn handle_common_event(
    p2p_node: &mut P2pNode,
    event: SwarmEvent<MyBehaviourEvent>,
    holepunch_req_sender: &Sender<PeerId>,
) -> Result<()> {
    let topic = p2p_node.topic.clone();

    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            let p2p_address = address.with(Protocol::P2p(*p2p_node.swarm.local_peer_id()));
            info!("Listening on {p2p_address}");
        }
        SwarmEvent::ConnectionEstablished {
            peer_id,
            endpoint,
            num_established,
            ..
        } => {
            info!(%peer_id, ?endpoint, %num_established, "Connection Established");
            // TODO: not sure if I need to add both address and send_back_addr.  Seems to
            // work for now
            let multiaddr = match endpoint {
                ConnectedPoint::Dialer { address, .. } => address,
                ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr,
            };
            p2p_node
                .swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, multiaddr);
        }
        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            warn!("Failed to dial {peer_id:?}: {error}");
        }
        SwarmEvent::IncomingConnectionError { error, .. } => {
            warn!("{:#}", anyhow::Error::from(error));
        }
        SwarmEvent::ConnectionClosed {
            peer_id,
            cause,
            endpoint,
            num_established,
            ..
        } => {
            info!(%peer_id, ?endpoint, %num_established, ?cause, "Connection Closed");
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Dcutr(event)) => {
            info!("dcutr: {:?}", event);
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::RelayClient(event)) => {
            info!("Relay client: {event:?}");
        } // SwarmEvent::Behaviour(MyBehaviourEvent::RelayClient(
        //     relay::client::Event::ReservationReqAccepted { .. },
        // )) => {
        //     info!("Relay accepted our reservation request");
        // }
        // we sent information about ourselves to a peer
        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Sent { .. })) => {
            tracing::info!("Sent identify info to a peer");
        }
        // A peer sent us information about ourselves
        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {
            info:
                identify::Info {
                    observed_addr,
                    listen_addrs,
                    protocols,
                    ..
                },
            peer_id,
            ..
        })) => {
            tracing::info!(address=%observed_addr, "Received identify info from a peer");
            // TODO: if we only ever receive this event from peers we're connected to, we can
            // listen to nodes who claim to be relays
            for protocol in protocols {
                // if they have a relay protocol, listen to them and add them to list of relays
                if protocol == RELAY_SERVER_PROTOCOL_ID {
                    for relay_multiaddr in &listen_addrs {
                        // skip if relay shared their localhost address
                        if find_ipv4(&relay_multiaddr.to_string()) == Some("127.0.0.1".to_string())
                        {
                            continue;
                        }

                        // circuit multiaddr to listen to
                        let circuit_multiaddr = relay_multiaddr
                            .clone()
                            .with(Protocol::P2p(peer_id))
                            .with(Protocol::P2pCircuit);
                        p2p_node.swarm.listen_on(circuit_multiaddr.clone()).unwrap();

                        // non-circuit multiaddr to advertise to others
                        p2p_node.add_relay(Peer {
                            multiaddr: relay_multiaddr.clone(),
                            peer_id,
                        })
                    }
                }
            }

            // add them to kademlia
            for multiaddr in listen_addrs {
                p2p_node
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, multiaddr);
            }

            p2p_node.swarm.add_external_address(observed_addr);
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
            for (peer_id, _multiaddr) in list {
                // println!("mDNS discovered a new peer: {peer_id}");
                // Explicit peers are peers that remain connected and we unconditionally
                // forward messages to, outside of the scoring system.
                p2p_node
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .add_explicit_peer(&peer_id);

                // Dial this known peer so the logic in Identify is executed (add to kademlia,
                // holepunch, etc)
                p2p_node.swarm.dial(peer_id).unwrap();
            }
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
            for (peer_id, _multiaddr) in list {
                // println!("mDNS discovered peer has expired: {peer_id}");
                p2p_node
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);
                // swarm.behaviour_mut().kademlia.remove_address(&peer_id, &multiaddr);
            }
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
            propagation_source,
            message_id: _id,
            message,
        })) => {
            handle_message(p2p_node, message, topic, propagation_source).unwrap();
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
            peer_id,
            topic: _,
        })) => {
            info!("{peer_id} subscribed to the topic!");
        }

        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed { result, .. },
        )) => match result {
            kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                key,
                providers,
                ..
            })) => {
                for peer in providers {
                    println!(
                        "Peer {peer:?} provides key {:?}",
                        std::str::from_utf8(key.as_ref()).unwrap()
                    );
                }
            }
            kad::QueryResult::GetProviders(Err(err)) => {
                eprintln!("Failed to get providers: {err:?}");
            }
            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(kad::PeerRecord {
                record: kad::Record { key, value, .. },
                ..
            }))) => {
                println!(
                    "Got record {:?} {:?}",
                    std::str::from_utf8(key.as_ref()).unwrap(),
                    std::str::from_utf8(&value).unwrap(),
                );
            }
            // kad::QueryResult::GetRecord(Ok(_)) => {}
            kad::QueryResult::GetRecord(Err(err)) => {
                eprintln!("Failed to get record: {err:?}");
            }
            kad::QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                println!(
                    "Successfully put record {:?}",
                    std::str::from_utf8(key.as_ref()).unwrap()
                );
            }
            kad::QueryResult::PutRecord(Err(err)) => {
                eprintln!("Failed to put record: {err:?}");
            }
            kad::QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                println!(
                    "Successfully put provider record {:?}",
                    std::str::from_utf8(key.as_ref()).unwrap()
                );
            }
            kad::QueryResult::StartProviding(Err(err)) => {
                eprintln!("Failed to put provider record: {err:?}");
            }
            kad::QueryResult::Bootstrap(Ok(BootstrapOk { .. })) => (),
            kad::QueryResult::Bootstrap(Err(BootstrapError::Timeout { peer, .. })) => {
                // if we failed to bootstrap to a node, it is most likely behind a firewall.  Hole
                // punch to it
                holepunch_req_sender.send(peer).await.unwrap();
            }
            _ => {
                info!("KAD: {:?}", result)
            }
        },

        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
            peer,
            addresses,
            ..
        })) => {
            info!( peer=%peer, addresses=?addresses, "KAD routing table updated");
        }
        _ => (),
    };
    Ok(())
}

// TODO: in the future this function will have a lot more logic to handle message about different
// subjects (consensus, bootstrapping, mempool)
fn handle_message(
    p2p_node: &mut P2pNode,
    message: Message,
    topic: IdentTopic,
    propagation_source: PeerId,
) -> Result<()> {
    let message = String::from_utf8_lossy(&message.data);

    println!("Got message from peer: {propagation_source}\n{}", message);

    if let Some(target_peer_id) = message.strip_prefix(WANT_RELAY_FOR_PREFIX) {
        let my_peer_id = p2p_node.swarm.local_peer_id().to_string();

        if my_peer_id == target_peer_id {
            // send a response with a relay that you're listening on

            // space separated list of multiaddrs of the relays you listen to
            let relays_i_listen_to = stringify_relays_multiaddr(&p2p_node.relays);

            // TODO: could later make this all relays I listen to
            let response = format!("{I_HAVE_RELAYS_PREFIX}{my_peer_id} {relays_i_listen_to}");

            if let Err(e) = p2p_node
                .swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic, response.as_bytes())
            {
                warn!("Publish error: {e:?}");
            }
        }
    }

    Ok(())
}

fn stringify_relays_multiaddr(relays: &HashSet<Peer>) -> String {
    let stringified = relays.iter().fold("".to_string(), |acc, relay_peer| {
        format!("{} {acc}", relay_peer.multiaddr)
    });
    stringified.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;

    #[test]
    fn test_stringify_relays() {
        // none relay
        let relays = Vec::new();
        let correct_stringified_relays = "";
        assert_eq!(
            stringify_relays_multiaddr(&relays.into_iter().collect()),
            correct_stringified_relays
        );

        // one relay
        let relays = vec![Peer {
            multiaddr: "/ip4/127.0.0.1/tcp/4001".parse().unwrap(),
            peer_id: PeerId::random(),
        }];

        let correct_stringified_relays = "/ip4/127.0.0.1/tcp/4001";
        assert_eq!(
            stringify_relays_multiaddr(&relays.into_iter().collect()),
            correct_stringified_relays
        );
    }
}
