use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::panic::AssertUnwindSafe;
use std::string::String;
use testcontainers::{
    core::{ContainerAsync, IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage,
};

mod test_helper;
use test_helper::SigilTestInstance;

#[tokio::test]
async fn test_zero_connections() {
    let sigil = SigilTestInstance::new().await;

    sigil
        .execute_rpc_call_with_expected("connected_peers", None, "[]")
        .await
        .unwrap();

    sigil
        .execute_rpc_call_with_expected("gossipsub_mesh_peers", None, "[]")
        .await
        .unwrap();

    sigil
        .execute_rpc_call_with_expected("kademlia_routing_table_peers", None, "{}")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_hello_sigil() {
    let sigil = SigilTestInstance::new().await;

    sigil
        .execute_rpc_call_with_expected("say_hello", Some("Sigil"), "Hello, Sigil!")
        .await
        .unwrap();
}
