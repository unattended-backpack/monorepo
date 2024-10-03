/*

TODO:

[] directly dialing people on dns (/dns/<address> in rust-libp2p/examples/ipfs-kad)
[] remove asserts, panics, and unwraps
[] all levels of error handling
[] all levels of tracing logs.  Re-read zero-to-prod logging approach
[] auto bootstrap when it hits a certain low threshold or receives some error (not enough peers, etc)
[] proper error handling, not just bubbling up anyhow!()

*/

use anyhow::{Context, Result};
use futures::{executor::block_on, future::FutureExt, StreamExt};
use libp2p::{
    dcutr,
    gossipsub::{self, IdentTopic},
    identify, identity, kad,
    kad::store::MemoryStore,
    mdns,
    multiaddr::{Multiaddr, Protocol},
    noise, relay,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, Swarm,
};
use serde::Deserialize;
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
    net::Ipv4Addr,
};
use tokio::{
    io::{self, AsyncBufReadExt},
    select,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};
use tracing::{debug, instrument, trace, warn};

mod config;

mod bootstrap;
use bootstrap::BootstrapEvent;

mod event_handler;
use event_handler::handle_swarm_event;

mod holepuncher;
use holepuncher::HolepunchEvent;

mod swarm_client;
use swarm_client::SwarmCommand;

pub use config::Config;
pub use swarm_client::SwarmClient;

const MDNS_AGENT_STRING: &str = "sigil/1.0.0";
const IDENTIFY_PROTOCOL_VERSION: &str = "TODO/0.0.1";
pub const GOSSIPSUB_TOPIC: &str = "test-net";

pub const WANT_RELAY_FOR_PREFIX: &str = "WANT RELAY FOR ";
pub const I_HAVE_RELAYS_PREFIX: &str = "I HAVE RELAYS ";

// custom network behavious that combines gossipsub and mdns
#[derive(NetworkBehaviour)]
pub struct MyBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub relay_client: relay::client::Behaviour,
    // some nodes are relay servers for routing messages
    // Some nodes are not relays
    pub toggle_relay: Toggle<relay::Behaviour>,
    // for learning our own addr and telling other nodes their addr
    pub identify: identify::Behaviour,
    // hole punching
    pub dcutr: dcutr::Behaviour,
    // bootstrapping connections
    pub kademlia: kad::Behaviour<MemoryStore>,
    // TODO: can use connection_limits::Behaviour to limit connections by a % of max memory
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Peer {
    pub multiaddr: Multiaddr,
    pub peer_id: PeerId,
}

pub struct P2pNode {
    pub swarm: Swarm<MyBehaviour>,
    pub topic: gossipsub::IdentTopic,
    pub cfg: Config,
    // relays that we're listening on
    pub relays: HashSet<Peer>,
}

impl P2pNode {
    pub fn start(cfg: Config) -> Result<SwarmClient> {
        let (command_sender, command_receiver) = mpsc::channel(16);

        let client = SwarmClient::new(command_sender.clone());

        tokio::spawn(async move {
            let mut p2p_node = Self::init(cfg).context("init p2p_node").unwrap();
            p2p_node
                .run(command_sender, command_receiver)
                .await
                .unwrap();
        });

        Ok(client)
    }

    fn init(cfg: Config) -> Result<Self> {
        let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);
        let swarm = build_swarm(&cfg, topic.clone())?;
        let relays = HashSet::new();

        trace!("P2pNode created");

        Ok(Self {
            swarm,
            topic,
            cfg,
            relays,
        })
    }

    async fn run(
        &mut self,
        swarm_command_sender: Sender<SwarmCommand>,
        mut swarm_command_receiver: Receiver<SwarmCommand>,
    ) -> Result<()> {
        // start listening
        self.listen_on_addrs()
            .await
            .context("listen on all addrs")?;

        // TODO: how big should the channels be?
        let (bootstrap_event_sender, bootstrap_event_receiver) = mpsc::channel(16);
        let (holepunch_event_sender, holepunch_event_receiver) = mpsc::channel(16);
        let (holepunch_req_sender, holepunch_req_receiver) = mpsc::channel(16);

        let swarm_client = SwarmClient::new(swarm_command_sender);

        // start concurrent process to dial all nodes in the config
        trace!("starting initial bootstrap");
        Self::bootstrap(
            self.cfg.clone(),
            bootstrap_event_receiver,
            holepunch_req_sender.clone(),
            swarm_client.clone(),
        )
        .context("initial bootstrap")?;

        // start concurrent process to handle requests to hole punch
        trace!("starting to watch for holepunch requests");
        Self::watch_for_holepunch_request(
            swarm_client.clone(),
            holepunch_req_receiver,
            holepunch_event_receiver,
        )
        .context("watching for holepunch requests")?;

        // read full lines from stdin
        trace!("reading liens from stdin");
        let mut stdin = io::BufReader::new(io::stdin()).lines();

        // let it rip
        debug!("setup done, entering main event loop");
        loop {
            select! {
                Some(command) = swarm_command_receiver.recv() => self.exec_swarm_command(command).context("exec swarm command {command}")?,
                event = self.swarm.select_next_some() => handle_swarm_event(self, event, &bootstrap_event_sender, &holepunch_event_sender, &holepunch_req_sender).await.context("handle swarm event")?,
                // Writing & line stuff is just for debugging & dev
                Ok(Some(line)) = stdin.next_line() => handle_input_line(self, line).context("handle input line")?,
            };
        }
    }

    /// returns the peer_id of the local P2pNode
    pub fn local_peer_id(&self) -> String {
        self.swarm.local_peer_id().to_string()
    }

    fn bootstrap(
        cfg: Config,
        mut event_receiver: Receiver<BootstrapEvent>,
        holepunch_req_sender: Sender<PeerId>,
        swarm_client: SwarmClient,
    ) -> Result<()> {
        tokio::spawn(async move {
            bootstrap::bootstrap(cfg, &mut event_receiver, holepunch_req_sender, swarm_client)
                .await
                .unwrap();
        });

        Ok(())
    }

    fn watch_for_holepunch_request(
        swarm_client: SwarmClient,
        mut receiver: Receiver<PeerId>,
        mut event_receiver: Receiver<HolepunchEvent>,
    ) -> Result<()> {
        tokio::spawn(async move {
            holepuncher::watch_for_holepunch_request(
                swarm_client,
                &mut receiver,
                &mut event_receiver,
            )
            .await
            .unwrap();
        });

        Ok(())
    }

    async fn listen_on_addrs(&mut self) -> Result<()> {
        // Listen on all interfaces and the specified port
        let listen_addr_tcp = Multiaddr::empty()
            .with(Protocol::from(Ipv4Addr::UNSPECIFIED))
            .with(Protocol::Tcp(self.cfg.port));
        self.swarm
            .listen_on(listen_addr_tcp.clone())
            .context("Listen on tcp addr {:?listen_addr_tcp}")?;
        debug!(%listen_addr_tcp, "listening on tcp address");

        let listen_addr_quic = Multiaddr::empty()
            .with(Protocol::from(Ipv4Addr::UNSPECIFIED))
            .with(Protocol::Udp(self.cfg.port))
            .with(Protocol::QuicV1);
        self.swarm
            .listen_on(listen_addr_quic.clone())
            .context("Listen on quic addr {listen_addr_quic}")?;
        debug!(%listen_addr_quic, "listening on quic address");

        block_on(async {
            let mut delay = futures_timer::Delay::new(std::time::Duration::from_secs(1)).fuse();
            let mut listening_on_tcp = false;
            let mut listening_on_quic = false;
            loop {
                futures::select! {
                    event = self.swarm.next() => {
                        match event.unwrap() {
                            SwarmEvent::NewListenAddr { address, .. } => {
                                if address == listen_addr_tcp {
                                    listening_on_tcp = true;
                                } else if address == listen_addr_quic {
                                    listening_on_quic = true;
                                }

                                if listening_on_quic && listening_on_tcp {
                                    break;
                                }
                            }
                            _ => continue,
                        }
                    }
                    _ = delay => {
                        // Likely listening on all interfaces now, thus continuing by breaking the loop.
                        break;
                    }
                }
            }
        });

        // TODO: this is for the test in monorepo/sigil/tests.  Will probably remove later
        println!("Sigil is alive.");

        Ok(())
    }

    pub(crate) fn add_relay(&mut self, relay: Peer) {
        trace!(?relay, "adding connected relay");
        self.relays.insert(relay);
    }

    #[instrument(skip_all, level = "debug")]
    fn exec_swarm_command(self: &mut P2pNode, command: SwarmCommand) -> Result<()> {
        let swarm = &mut self.swarm;
        // TODO: remove upwraps
        match command {
            // Gossipsub commands
            SwarmCommand::GossipsubPublish { data } => {
                debug!(?data, "GossipsubPublish");
                let topic = self.topic.clone();
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic, data)
                    .unwrap();
            }
            // Swarm commands
            SwarmCommand::Dial { multiaddr } => {
                debug!(%multiaddr, "Dial");
                swarm.dial(multiaddr).unwrap();
            }
            SwarmCommand::MyRelays { sender } => {
                let my_relays = self.relays.clone();
                debug!(?my_relays, "MyRelays");
                sender.send(my_relays).unwrap();
            }
            SwarmCommand::ConnectedPeers { sender } => {
                let connected_peers: Vec<PeerId> = swarm.connected_peers().copied().collect();
                debug!(?connected_peers, "ConnectedPeers");
                sender.send(connected_peers).unwrap();
            }
            SwarmCommand::GossipsubMeshPeers { sender } => {
                let topic = self.topic.clone();
                let gossipsub_mesh_peers: Vec<PeerId> = swarm
                    .behaviour()
                    .gossipsub
                    .mesh_peers(&topic.into())
                    .copied()
                    .collect();
                debug!(?gossipsub_mesh_peers, "GossipsubMeshPeers");
                sender.send(gossipsub_mesh_peers).unwrap();
            }
            SwarmCommand::KademliaRoutingTablePeers { sender } => {
                let kademlia_routing_table_peers = swarm.behaviour_mut().kademlia.kbuckets().fold(
                    HashMap::new(),
                    |mut acc, kbucket| {
                        kbucket.iter().for_each(|entry| {
                            let peer_id = entry.node.key.into_preimage();
                            let peer_multiaddrs: Vec<Multiaddr> =
                                entry.node.value.iter().cloned().collect();
                            acc.insert(peer_id, peer_multiaddrs);
                        });
                        acc
                    },
                );
                debug!(?kademlia_routing_table_peers, "KademliaRoutingTablePeers");
                sender.send(kademlia_routing_table_peers).unwrap();
            }
        };

        Ok(())
    }
}

fn generate_ed25519(secret_key_seed: u8) -> identity::Keypair {
    let mut bytes = [0u8; 32];
    bytes[0] = secret_key_seed;

    identity::Keypair::ed25519_from_bytes(bytes).expect("only errors on wrong length")
}

fn handle_input_line(p2p_node: &mut P2pNode, line: String) -> Result<()> {
    if let Err(e) = p2p_node
        .swarm
        .behaviour_mut()
        .gossipsub
        .publish(p2p_node.topic.clone(), line.as_bytes())
    {
        warn!("Publish error: {e:?}");
    }
    // }
    /*
        let mut args = line.split(' ');
        let kademlia = swarm.behaviour_mut().kademlia;

        let _ = match args.next() {
            Some("GET") => {
                let key = {
                    match args.next() {
                        Some(key) => kad::RecordKey::new(&key),
                        None => {
                            eprintln!("Expected key");
                        }
                    }
                };
                kademlia.get_record(key);
            }
            Some("GET_PROVIDERS") => {
                let key = {
                    match args.next() {
                        Some(key) => kad::RecordKey::new(&key),
                        None => {
                            eprintln!("Expected key");
                        }
                    }
                };
                kademlia.get_providers(key);
            }
            Some("PUT") => {
                let key = {
                    match args.next() {
                        Some(key) => kad::RecordKey::new(&key),
                        None => {
                            eprintln!("Expected key");
                        }
                    }
                };
                let value = {
                    match args.next() {
                        Some(value) => value.as_bytes().to_vec(),
                        None => {
                            eprintln!("Expected value");
                        }
                    }
                };
                let record = kad::Record {
                    key,
                    value,
                    publisher: None,
                    expires: None,
                };
                kademlia
                    .put_record(record, kad::Quorum::One)
                    .expect("Failed to store record locally.");
            }
            Some("PUT_PROVIDER") => {
                let key = {
                    match args.next() {
                        Some(key) => kad::RecordKey::new(&key),
                        None => {
                            eprintln!("Expected key");
                        }
                    }
                };

                kademlia
                    .start_providing(key)
                    .expect("Failed to start providing key");
            }
            _ => {
                eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
            }
        };

        Ok(())
    */
    Ok(())
}

fn build_swarm(cfg: &Config, topic: IdentTopic) -> Result<Swarm<MyBehaviour>> {
    // deterministically generate a PeerId based on given seed for development ease.
    let local_key: identity::Keypair = generate_ed25519(cfg.secret_key_seed);

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_dns()?
        .with_relay_client(noise::Config::new, yamux::Config::default)?
        .with_behaviour(|keypair, relay_behaviour| {
            // To content-address messave, we can take the hash of the message and use it as an ID.
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            };

            // Set a custom gossipsub configuration
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(15)) // This is set to aid debugging by not cluttering the log space
                .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
                .mesh_n(cfg.num_gossipsub_connections.mesh_n())
                .mesh_n_low(cfg.num_gossipsub_connections.mesh_n_low())
                .mesh_n_high(cfg.num_gossipsub_connections.mesh_n_high())
                // TODO: figure out what this is about
                // .support_floodsub()
                // .flood_publish(true)
                .build()
                .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                gossipsub_config,
            )?;

            let agent_string = MDNS_AGENT_STRING.to_string();
            let mdns_string = agent_string.replace(['/', '.'], "_");
            let mdns_config = mdns::Config::default().set_name(&mdns_string)?;
            let mdns = mdns::tokio::Behaviour::new(mdns_config, keypair.public().to_peer_id())?;

            let relay_client = relay_behaviour;

            // if user has indicated they don't want to be a relay, toggle the relay off
            let toggle_relay = if cfg.is_relay {
                Toggle::from(Some(relay::Behaviour::new(
                    keypair.public().to_peer_id(),
                    Default::default(),
                )))
            } else {
                Toggle::from(None)
            };

            let identify = identify::Behaviour::new(identify::Config::new(
                IDENTIFY_PROTOCOL_VERSION.to_string(),
                keypair.public(),
            ));

            let dcutr = dcutr::Behaviour::new(keypair.public().to_peer_id());

            let kademlia = kad::Behaviour::new(
                keypair.public().to_peer_id(),
                MemoryStore::new(keypair.public().to_peer_id()),
            );
            Ok(MyBehaviour {
                gossipsub,
                mdns,
                relay_client,
                toggle_relay,
                identify,
                dcutr,
                kademlia,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm
        .behaviour_mut()
        .gossipsub
        .subscribe(&topic)
        .context("subscribe to gossipsub topic {topic}")?;
    Ok(swarm)
}

// extract the ipv4 as a &str from a multiaddr
pub fn find_ipv4(multiaddr_str: &str) -> Option<String> {
    // break it up into protocol & addresses
    let multiaddr_parts: Vec<&str> = multiaddr_str.split("/").collect();

    // find location of the string "ip4"
    let ipv4_prefix_index = multiaddr_parts.iter().position(|part| *part == "ip4");

    // the ip follows the prefix "ip4"
    let ipv4_index = match ipv4_prefix_index {
        Some(index) => index + 1,
        None => return None,
    };

    multiaddr_parts.get(ipv4_index).map(|ipv4| ipv4.to_string())
}

// #[cfg(test)]
// mod tests {
//
//     use super::*;
//
//     #[tokio::test]
//     async fn smoke_test_swarm() {
//         let toml_str = r#""#;
//         let cfg: Config = toml::from_str(toml_str).unwrap();
//         let client = P2pNode::start(cfg).unwrap();
//         let connected_peers = client.connected_peers().await.unwrap();
//         assert!(connected_peers.is_empty());
//     }
// }
