use anyhow::{Context, Result};
use futures::stream::StreamExt;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{RpcModule, ServerBuilder};
use priory::{P2pNode, SwarmClient};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use std::{env, error::Error};
use tokio::{io, io::AsyncBufReadExt, select};
use tracing::debug;
use tracing_subscriber::EnvFilter;

mod config;
use config::Config;

const ENV_KEY_CONFIG_TOML_PATH: &str = "CONFIG_TOML_PATH";

#[rpc(server)]
pub trait MyApi {
    #[method(name = "say_hello")]
    async fn say_hello(&self, name: String) -> RpcResult<String>;
    #[method(name = "connected_peers")]
    async fn connected_peers(&self) -> RpcResult<String>;
    #[method(name = "gossipsub_mesh_peers")]
    async fn gossipsub_mesh_peers(&self) -> RpcResult<String>;
    #[method(name = "kademlia_routing_table_peers")]
    async fn kademlia_routing_table_peers(&self) -> RpcResult<String>;
    #[method(name = "my_peer_id")]
    async fn my_peer_id(&self) -> RpcResult<String>;
}

pub struct MyApiImpl {
    p2p_node_client: SwarmClient,
}

#[async_trait]
impl MyApiServer for MyApiImpl {
    async fn say_hello(&self, name: String) -> RpcResult<String> {
        Ok(format!("Hello, {}!", name))
    }

    // TODO: purge unwraps and such
    async fn connected_peers(&self) -> RpcResult<String> {
        let connected_peers = self
            .p2p_node_client
            .connected_peers()
            .await
            .context("request connected peers from p2p node client")
            .unwrap();

        Ok(format!("{:?}", connected_peers))
    }

    async fn gossipsub_mesh_peers(&self) -> RpcResult<String> {
        let gossipsub_mesh_peers = self
            .p2p_node_client
            .gossipsub_mesh_peers()
            .await
            .context("request gossipsub_mesh_peers from p2p node client")
            .unwrap();

        Ok(format!("{:?}", gossipsub_mesh_peers))
    }

    async fn kademlia_routing_table_peers(&self) -> RpcResult<String> {
        let kademlia_routing_table_peers = self
            .p2p_node_client
            .kademlia_routing_table_peers()
            .await
            .context("request kademlia_routing_table_peers from p2p node client")
            .unwrap();

        Ok(format!("{:?}", kademlia_routing_table_peers))
    }

    async fn my_peer_id(&self) -> RpcResult<String> {
        let my_peer_id = self
            .p2p_node_client
            .my_peer_id()
            .await
            .context("request my_peer_id from p2p node client")
            .unwrap();

        Ok(format!("{:?}", my_peer_id))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_toml_path = env::var(ENV_KEY_CONFIG_TOML_PATH).unwrap_or("sigil.toml".into());
    let cfg = Config::parse(&config_toml_path)?;
    debug!("found config file {config_toml_path}");

    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // TODO: clap/tracing/various env stuff

    // create and run priory node.  Returns a client that can be cloned.
    let p2p_node_client = P2pNode::start(cfg.priory)?;

    // call a priory function
    // let peers = p2p_node_client.connected_peers().await.unwrap();
    // println!("connected peers: {:?}", peers);

    // Start an RPC server.
    let server = ServerBuilder::default().build("0.0.0.0:3030").await?;
    let mut module = RpcModule::new(());
    let my_api_impl = MyApiImpl { p2p_node_client };
    module.merge(my_api_impl.into_rpc())?;
    let handle = server.start(module);

    // Wait for server to finish or Ctrl-C
    // tokio::signal::ctrl_c().await?;
    // handle.stopped().await;

    // simulate doing things
    loop {}
}
