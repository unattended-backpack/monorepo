use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use serial_test::serial;
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
#[serial]
async fn test_2_mdns_connections() {
    let sigil_a = SigilTestInstance::new("a.toml").await;
    let sigil_b = SigilTestInstance::new("b.toml").await;

    let sigil_a_peer_id = sigil_a.rpc("my_peer_id", None).await.unwrap();
    let sigil_b_peer_id = sigil_b.rpc("my_peer_id", None).await.unwrap();

    sigil_a
        .rpc_with_expected("connected_peers", None, &sigil_b_peer_id)
        .await
        .unwrap();

    sigil_a
        .rpc_with_expected("gossipsub_mesh_peers", None, &sigil_b_peer_id)
        .await
        .unwrap();

    sigil_a
        .rpc_with_expected("kademlia_routing_table_peers", None, &sigil_b_peer_id)
        .await
        .unwrap();

    sigil_b
        .rpc_with_expected("connected_peers", None, &sigil_a_peer_id)
        .await
        .unwrap();

    sigil_b
        .rpc_with_expected("gossipsub_mesh_peers", None, &sigil_a_peer_id)
        .await
        .unwrap();

    sigil_b
        .rpc_with_expected("kademlia_routing_table_peers", None, &sigil_a_peer_id)
        .await
        .unwrap();
}

#[tokio::test]
#[serial]
async fn test_no_connections_default_config() {
    let sigil = SigilTestInstance::new("default.toml").await;

    sigil
        .rpc_with_expected("connected_peers", None, "[]")
        .await
        .unwrap();

    sigil
        .rpc_with_expected("gossipsub_mesh_peers", None, "[]")
        .await
        .unwrap();

    sigil
        .rpc_with_expected("kademlia_routing_table_peers", None, "{}")
        .await
        .unwrap();
}

#[tokio::test]
#[serial]
async fn test_hello_sigil() {
    let sigil = SigilTestInstance::new("default.toml").await;

    sigil
        .rpc_with_expected("say_hello", Some("Sigil"), "Hello, Sigil!")
        .await
        .unwrap();
}
