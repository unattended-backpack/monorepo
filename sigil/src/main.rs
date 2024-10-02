use anyhow::Result;
use futures::stream::StreamExt;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{RpcModule, ServerBuilder};
use priory::P2pNode;
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
    async fn say_hello(&self, name: String) -> jsonrpsee::core::RpcResult<String>;
}

pub struct MyApiImpl;

#[async_trait]
impl MyApiServer for MyApiImpl {
    async fn say_hello(&self, name: String) -> jsonrpsee::core::RpcResult<String> {
        Ok(format!("Hello, {}!", name))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::parse(CONFIG_FILE_PATH)?;

    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // TODO: clap/tracing/various env stuff

    // Start an RPC server.
    let server = ServerBuilder::default().build("0.0.0.0:3030").await?;
    let mut module = RpcModule::new(());
    module.merge(MyApiImpl.into_rpc())?;
    let handle = server.start(module);

    // Wait for server to finish or Ctrl-C
    // tokio::signal::ctrl_c().await?;
    // handle.stopped().await;

    // create and run priory node
    let p2p_node_client = P2pNode::init_and_run(cfg.priory)?;

    Ok(())
}
