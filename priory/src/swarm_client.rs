use anyhow::{Context, Result};
use libp2p::{
    core::multiaddr::Multiaddr,
    gossipsub::{IdentTopic, TopicHash},
};
use std::collections::HashSet;
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
            .context("send command gossipsub publish {data}")
    }

    pub async fn dial(&self, multiaddr: Multiaddr) -> Result<()> {
        self.command_sender
            .send(SwarmCommand::Dial { multiaddr })
            .await
            .context("send command dial {multiaddr}")
    }

    pub async fn my_relays(&self) -> Result<HashSet<Peer>> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(SwarmCommand::MyRelays { sender })
            .await
            .context("send command my_relays")?;

        receiver.await.context("receive my_relays")
    }
}

#[derive(Debug)]
pub enum SwarmCommand {
    // Gossipsub commands
    GossipsubPublish {
        topic: TopicHash,
        data: Vec<u8>,
    },
    // Swarm commands
    Dial {
        multiaddr: Multiaddr,
    },
    // P2p node commands
    MyRelays {
        sender: oneshot::Sender<HashSet<Peer>>,
    },
}
