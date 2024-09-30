use anyhow::{Context, Result};
use libp2p::{
    core::PeerId,
    swarm::{DialError, SwarmEvent},
};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;

use crate::config::Config;
use crate::swarm_client::SwarmClient;
use crate::{MyBehaviourEvent, Peer};

// These are the events that we need some information from during bootstrapping.
// When encountered in the main thread, the specified data is copied here and the
// event is also handled by the common handler.
pub enum BootstrapEvent {
    ConnectionEstablished { peer_id: PeerId },
    OutgoingConnectionErrorLikelyFirewall,
    OutgoingConnectionErrorOther,
}

impl BootstrapEvent {
    pub fn try_from_swarm_event(event: &SwarmEvent<MyBehaviourEvent>) -> Option<BootstrapEvent> {
        match event {
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                Some(BootstrapEvent::ConnectionEstablished { peer_id: *peer_id })
            }
            SwarmEvent::OutgoingConnectionError { error, .. } => match error {
                DialError::Transport(_) => {
                    Some(BootstrapEvent::OutgoingConnectionErrorLikelyFirewall)
                }
                _ => Some(BootstrapEvent::OutgoingConnectionErrorOther),
            },
            _ => None,
        }
    }
}

pub async fn bootstrap(
    cfg: Config,
    event_receiver: &mut Receiver<BootstrapEvent>,
    holepunch_req_sender: Sender<PeerId>,
    swarm_client: SwarmClient,
) -> Result<()> {
    info!("BOOTSTRAPPING");

    // keep track of the nodes that we'll later have to hole punch into
    let mut failed_to_dial: Vec<Peer> = Vec::new();

    // try to dial all peers in config
    for peer in &cfg.peers.clone() {
        let peer_multiaddr = &peer.multiaddr;

        // dial peer
        // if successful add to DHT
        // if failure wait until we've made contact with the dht and find a peer to holepunch
        swarm_client.dial(peer_multiaddr.clone()).await.unwrap();

        // loop until we either connect or fail to connect
        loop {
            match event_receiver
                .recv()
                .await
                .context("bootstrap event sender shouldn't drop")
                .unwrap()
            {
                BootstrapEvent::ConnectionEstablished { peer_id, .. } => {
                    // have to make sure this event is about the node we just dialed
                    if peer_id == peer.peer_id {
                        break;
                    }
                }
                BootstrapEvent::OutgoingConnectionErrorLikelyFirewall => {
                    // TODO: have to make sure this event is about the node we just dialed (how???)
                    failed_to_dial.push(peer.clone());
                    break;
                }
                BootstrapEvent::OutgoingConnectionErrorOther => {
                    // TODO: have to make sure this event is about the node we just dialed (how???)
                    break;
                }
            }
        }
    }

    // unable to dial any of the peers you listed
    if failed_to_dial.len() == cfg.peers.len() && !cfg.peers.is_empty() {
        panic!("Couldn't connect to any adress listed as a peer in the config");
    }

    for peer in failed_to_dial {
        holepunch_req_sender.send(peer.peer_id).await.unwrap();
    }

    info!("BOOTSTRAP COMPLETE");
    Ok(())
}
