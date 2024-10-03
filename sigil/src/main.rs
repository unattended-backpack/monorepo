use anyhow::{Context, Result};
use futures::stream::StreamExt;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{RpcModule, ServerBuilder};
use priory::{P2pNode, SwarmClient};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::{io, io::AsyncBufReadExt, select};
use tracing_subscriber::EnvFilter;

mod config;
use config::Config;

const CONFIG_FILE_PATH: &str = "sigil.toml";

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::parse(CONFIG_FILE_PATH)?;

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

    Ok(())
}
