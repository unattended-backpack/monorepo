use anyhow::Result;
use libp2p::{
    core::multiaddr::Multiaddr,
    gossipsub::{IdentTopic, TopicHash},
};
use std::collections::HashSet;
use tokio::sync::{mpsc::Sender, oneshot};

use crate::Peer;

#[derive(Clone)]
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
            .unwrap();
        Ok(())
    }

    pub async fn dial(&self, multiaddr: Multiaddr) -> Result<()> {
        self.command_sender
            .send(SwarmCommand::Dial { multiaddr })
            .await
            .unwrap();
        Ok(())
    }

    pub async fn my_relays(&self) -> Result<HashSet<Peer>> {
        let (sender, receiver) = oneshot::channel();
        self.command_sender
            .send(SwarmCommand::MyRelays { sender })
            .await
            .unwrap();

        let my_relays = receiver.await.unwrap();

        Ok(my_relays)
    }
}

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
