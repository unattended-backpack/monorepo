use anyhow::{Context, Result};
use libp2p::{
    core::multiaddr::Multiaddr,
    gossipsub::{IdentTopic, TopicHash},
    PeerId,
};
use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc::Sender, oneshot};

use crate::Peer;

#[derive(Clone, Debug)]
pub struct SwarmClient {
    gossipsub_topic: IdentTopic,
    command_sender: Sender<SwarmCommand>,
}

impl SwarmClient {
    pub fn new(command_sender: Sender<SwarmCommand>, gossipsub_topic: IdentTopic) -> Self {
        Self {
            command_sender,
            gossipsub_topic,
        }
    }

    pub async fn gossipsub_publish(&self, data: String) -> Result<()> {
        self.command_sender
            .send(SwarmCommand::GossipsubPublish {
                topic: self.gossipsub_topic.clone().into(),
                data: data.into(),
            })
            .await
            .context("send command GossipsubPublish {data}")
    }

    pub async fn dial(&self, multiaddr: Multiaddr) -> Result<()> {
        self.command_sender
            .send(SwarmCommand::Dial { multiaddr })
            .await
            .context("send command Dial {multiaddr}")
    }

    pub async fn my_relays(&self) -> Result<HashSet<Peer>> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(SwarmCommand::MyRelays { sender })
            .await
            .context("send command MyRelays")?;

        receiver.await.context("receive my_relays")
    }

    pub async fn connected_peers(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(SwarmCommand::ConnectedPeers { sender })
            .await
            .context("send command ConnectedPeers")?;

        receiver.await.context("receive connected peers")
    }

    pub async fn gossipsub_mesh_peers(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(SwarmCommand::GossipsubMeshPeers { sender })
            .await
            .context("send command GossipsubMeshPeers")?;

        receiver.await.context("receive gossipsub mesh peers")
    }

    pub async fn kademlia_routing_table_peers(&self) -> Result<HashMap<PeerId, Vec<Multiaddr>>> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(SwarmCommand::KademliaRoutingTablePeers { sender })
            .await
            .context("send command KademliaRoutingTablePeers")?;

        receiver
            .await
            .context("receive kademlia routing table peers")
    }
}

#[derive(Debug)]
pub enum SwarmCommand {
    // publish data to the gossipsub network
    GossipsubPublish {
        topic: TopicHash,
        data: Vec<u8>,
    },
    // dial an address
    Dial {
        multiaddr: Multiaddr,
    },
    // share the relays that the node is listening to
    MyRelays {
        sender: oneshot::Sender<HashSet<Peer>>,
    },
    // shares the PeerIds of all connected peers
    ConnectedPeers {
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    // shares all gossipsub mesh peers that are subscribed to the swarm's topic
    GossipsubMeshPeers {
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    // shares the swarm's kademlia routing table peers
    KademliaRoutingTablePeers {
        sender: oneshot::Sender<HashMap<PeerId, Vec<Multiaddr>>>,
    },
}
