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

// #[tokio::test]
// async fn test_2_mdns_connections() {
//     let sigil_a = SigilTestInstance::new().await;
//     let sigil_b = SigilTestInstance::new().await;
//
//     let peer_id_a = sigil_a.rpc("my_peer_id", None).await.unwrap();
//     println!("peer a id: {}", peer_id_a);
//
//     let connected_peers_a = sigil_a.rpc("connected_peers", None).await.unwrap();
//     println!("connected peers a: {}", connected_peers_a);
//
//     let gossipsub_peers_a = sigil_a.rpc("gossipsub_mesh_peers", None).await.unwrap();
//     println!("gossipsub mesh peers a: {}", gossipsub_peers_a);
//
//     let peer_id_b = sigil_b.rpc("my_peer_id", None).await.unwrap();
//     println!("peer b id: {}", peer_id_b);
//
//     let connected_peers_b = sigil_b.rpc("connected_peers", None).await.unwrap();
//     println!("connected peers b: {}", connected_peers_b);
//
//     let gossipsub_peers_b = sigil_b.rpc("gossipsub_mesh_peers", None).await.unwrap();
//     println!("gossipsub mesh peers b: {}", gossipsub_peers_b);
// }

#[tokio::test]
async fn test_no_connections_default_config() {
    let sigil = SigilTestInstance::new("default.toml", 3030).await;

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
async fn test_hello_sigil() {
    let sigil = SigilTestInstance::new("default.toml", 3030).await;

    sigil
        .rpc_with_expected("say_hello", Some("Sigil"), "Hello, Sigil!")
        .await
        .unwrap();
}
