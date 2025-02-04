use anyhow::{Context, Result};
use libp2p::{core::PeerId, swarm::SwarmEvent};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, error, info, instrument, trace, warn};

use crate::config::Config;
use crate::swarm_client::SwarmClient;
use crate::{MyBehaviourEvent, Peer};

// These are the events that we need some information from during bootstrapping.
// When encountered in the main thread, the specified data is copied here and the
// event is also handled by the common handler.
#[derive(Debug)]
pub enum BootstrapEvent {
    ConnectionEstablished { peer_id: PeerId },
    OutgoingConnectionError,
}

impl BootstrapEvent {
    pub fn try_from_swarm_event(event: &SwarmEvent<MyBehaviourEvent>) -> Option<BootstrapEvent> {
        match event {
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                Some(BootstrapEvent::ConnectionEstablished { peer_id: *peer_id })
            }
            SwarmEvent::OutgoingConnectionError { .. } => {
                Some(BootstrapEvent::OutgoingConnectionError)
            }
            _ => None,
        }
    }
}

#[instrument(skip_all)]
pub async fn bootstrap(
    cfg: Config,
    event_receiver: &mut Receiver<BootstrapEvent>,
    holepunch_req_sender: Sender<PeerId>,
    swarm_client: SwarmClient,
) -> Result<()> {
    info!("Bootstrapping");

    // keep track of the nodes that we'll later have to hole punch into
    let mut failed_to_dial: Vec<Peer> = Vec::new();

    debug!("dialing {} peers", &cfg.peers.len());
    // try to dial all peers in config
    for peer in &cfg.peers {
        let peer_multiaddr = &peer.multiaddr;

        // dial peer
        // if successful add to DHT
        // if failure wait until we've made contact with the dht and find a peer to holepunch
        swarm_client
            .dial(peer_multiaddr.clone())
            .await
            .context("bootstrap dial of {:peer_multiaddr?}")?;
        debug!(?peer_multiaddr, peer_id = ?peer.peer_id, "dialing");

        // loop until we either connect or fail to connect
        loop {
            match event_receiver
                .recv()
                .await
                .context("bootstrap event sender shouldn't drop")?
            {
                BootstrapEvent::ConnectionEstablished { peer_id, .. } => {
                    // have to make sure this event is about the node we just dialed
                    if peer_id == peer.peer_id {
                        trace!(?peer_id, "Connection Established");
                        break;
                    }
                }
                BootstrapEvent::OutgoingConnectionError => {
                    warn!(dialed_peer_id=?peer.peer_id, "Connection error after dialing, possibly firewall");
                    // TODO: have to make sure this event is about the node we just dialed (how???)
                    failed_to_dial.push(peer.clone());
                    break;
                }
            }
        }
    }

    // unable to dial any of the peers you listed
    if failed_to_dial.len() == cfg.peers.len() && !cfg.peers.is_empty() {
        error!("Couldn't connect to any address listed as a peer in the config.  Exiting.");
        panic!("Couldn't connect to any address listed as a peer in the config.  Exiting.");
    }

    let successful_dials = cfg.peers.len() - failed_to_dial.len();
    let unsuccessful_dials = failed_to_dial.len();
    info!(successful_dials, unsuccessful_dials, "Bootstrap complete.");

    for peer in failed_to_dial {
        let peer_id = peer.peer_id;
        info!(peer_multiaddr=?peer.multiaddr, ?peer_id, "sending holepunch request");
        holepunch_req_sender
            .send(peer.peer_id)
            .await
            .context("request hole punch peer that failed bootstrap dial {:peer_id?}")?;
    }

    Ok(())
}
