use futures::stream::StreamExt;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{RpcModule, ServerBuilder};
use libp2p::{
    core::Multiaddr,
    dns, gossipsub, identify, mdns, noise, quic,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, tls, yamux, SwarmBuilder,
};
use libp2p_identity::Keypair;
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::{io, io::AsyncBufReadExt, select};
use tracing_subscriber::EnvFilter;

#[rpc(server)]
pub trait MyApi {
    #[method(name = "say_hello")]
    async fn say_hello(&self, name: String) -> jsonrpsee::core::RpcResult<String>;
}

pub struct MyApiImpl;

#[async_trait]
impl MyApiServer for MyApiImpl {
    async fn say_hello(&self, name: String) -> jsonrpsee::core::RpcResult<String> {
        Ok(format!("Hello, {}!", name))
    }
}

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    identify: identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // TODO: clap/tracing/various env stuff

    // Start an RPC server.
    let server = ServerBuilder::default().build("127.0.0.1:3030").await?;
    let mut module = RpcModule::new(());
    module.merge(MyApiImpl.into_rpc())?;
    let handle = server.start(module);

    // Wait for server to finish or Ctrl-C
    // tokio::signal::ctrl_c().await?;
    // handle.stopped().await;

    // Generate a private key for this node.
    let key = Keypair::generate_ed25519();
    println!("peer id {:?}", key.public().to_peer_id());

    // TODO: defaults, pull from env.
    // Prepare TCP connection management configuration.
    let tcp_config = tcp::Config::new()
        .ttl(64)
        .nodelay(true)
        .listen_backlog(1024)
        .port_reuse(false);

    // TODO: defaults, pull from env.
    // Prepare QUIC connection management configuration.
    let mut quic_config = quic::Config::new(&key);
    quic_config.handshake_timeout = Duration::from_secs(5);
    quic_config.max_idle_timeout = 10 * 1000;
    quic_config.keep_alive_interval = Duration::from_secs(5);
    quic_config.max_concurrent_stream_limit = 256;
    quic_config.max_stream_data = 10_000_000;
    quic_config.max_connection_data = 15_000_000;

    // TODO: test DNS resolution when attempting to connect to peer.
    // Prepare DNS configuration.
    let dns_config = dns::ResolverConfig::new();
    let dns_opts = dns::ResolverOpts::default();

    let mut swarm = SwarmBuilder::with_existing_identity(key)
        .with_tokio()
        .with_tcp(
            tcp_config,
            (tls::Config::new, noise::Config::new),
            yamux::Config::default,
        )
        .expect("swarm TCP configuration should have succeeded")
        .with_quic_config(|_| quic_config)
        .with_dns_config(dns_config, dns_opts)
        // with relay_client
        .with_behaviour(|key| {
            // To content-address message, we can take the hash of message and use it as an ID.
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            };

            // Set a custom gossipsub configuration
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
                .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
                .build()
                .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?; // Temporary hack because `build` does not return a proper `std::error::Error`.

            // build a gossipsub network behaviour
            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            let agent_string = "sigil/1.0.0".to_string();
            let mdns_string = agent_string.replace(['/', '.'], "_");
            let mdns_config = mdns::Config::default().set_name(&mdns_string)?;
            let mdns = mdns::tokio::Behaviour::new(mdns_config, key.public().to_peer_id())?;

            // Prepare a means to identify this client.
            // TODO: expose full config options.
            let identify = identify::Behaviour::new(
                identify::Config::new(agent_string.clone(), key.public())
                    .with_agent_version(agent_string.clone()),
            );

            Ok(MyBehaviour {
                gossipsub,
                mdns,
                identify,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    // Create a Gossipsub topic
    let topic = gossipsub::IdentTopic::new("test-net");
    // subscribes to our topic
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Explicitly dial a remote peer.
    // let remote_peer: Multiaddr = "/ip4/95.217.163.246/udp/3888/quic-v1".parse()?;
    // let dial_result = swarm.dial(remote_peer);
    // println!("dial result {:?}", dial_result);

    println!("Sigil is alive.");

    // TODO: add JSON-RPC server that runs in parallel such that we can issue method requests for
    // peer discovery.

    // Kick it off
    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                if let Err(e) = swarm
                    .behaviour_mut().gossipsub
                    .publish(topic.clone(), line.as_bytes()) {
                        println!("Publish error: {e:?}");
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                },
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("Successfully connected to {:?}", peer_id);
                },
                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    println!("Connection closed with {:?}, cause: {:?}", peer_id, cause);
                },
                SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                    println!("Failed to connect to {:?}: {:?}", peer_id, error);
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received { connection_id, peer_id, info })) => {
                    println!("Identified Peer: {}, AgentVersion: {}", peer_id, info.agent_version);
                    // TODO: Add some rules about peer rejection based on semver plus environment
                    // overrides.
                    if !info.agent_version.contains("vigil/1.") {
                        // If the AgentVersion indicates an IPFS client, ignore or disconnect
                        println!("rejecting client: {}", peer_id);
                        swarm.disconnect_peer_id(peer_id).unwrap_or_else(|err| {
                            println!("Failed to disconnect: {:?}", err);
                        });
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        if peer_id != *swarm.local_peer_id() {
                            println!("mDNS discovered a new peer: {peer_id}");
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                })) => println!(
                    "Got message: '{}' with id: {id} from peer: {peer_id}",
                    String::from_utf8_lossy(&message.data),
                ),
                _ => {}
            }
        }
    }
}
