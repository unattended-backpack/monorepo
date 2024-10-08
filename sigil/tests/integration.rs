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
async fn test_3_mdns_connections() {
    let sigil_a = SigilTestInstance::new("a.toml").await;
    let sigil_b = SigilTestInstance::new("b.toml").await;
    let sigil_c = SigilTestInstance::new("c.toml").await;

    let sigil_a_peer_id = sigil_a.rpc("my_peer_id", None).await.unwrap();
    let sigil_b_peer_id = sigil_b.rpc("my_peer_id", None).await.unwrap();
    let sigil_c_peer_id = sigil_c.rpc("my_peer_id", None).await.unwrap();

    let sigil_instances = vec![sigil_a, sigil_b, sigil_c];
    let sigil_peer_ids = vec![sigil_a_peer_id, sigil_b_peer_id, sigil_c_peer_id];

    for (i, sigil_instance) in sigil_instances.iter().enumerate() {
        for (j, sigil_peer_id) in sigil_peer_ids.iter().enumerate() {
            // skip checking for the the peer_id of the current sigil_instance
            if i == j {
                continue;
            }

            sigil_instance
                .rpc_with_expected("connected_peers", None, sigil_peer_id)
                .await
                .unwrap();

            sigil_instance
                .rpc_with_expected("kademlia_routing_table_peers", None, sigil_peer_id)
                .await
                .unwrap();

            sigil_instance
                .rpc_with_expected("gossipsub_mesh_peers", None, sigil_peer_id)
                .await
                .unwrap();
        }
    }
}

#[tokio::test]
#[serial]
async fn test_2_mdns_connections() {
    let sigil_a = SigilTestInstance::new("a.toml").await;
    let sigil_b = SigilTestInstance::new("b.toml").await;

    let sigil_a_peer_id = sigil_a.rpc("my_peer_id", None).await.unwrap();
    let sigil_b_peer_id = sigil_b.rpc("my_peer_id", None).await.unwrap();

    let sigil_instances = vec![sigil_a, sigil_b];
    let sigil_peer_ids = vec![sigil_a_peer_id, sigil_b_peer_id];

    for (i, sigil_instance) in sigil_instances.iter().enumerate() {
        for (j, sigil_peer_id) in sigil_peer_ids.iter().enumerate() {
            // skip checking for the the peer_id of the current sigil_instance
            if i == j {
                continue;
            }

            sigil_instance
                .rpc_with_expected("connected_peers", None, sigil_peer_id)
                .await
                .unwrap();

            sigil_instance
                .rpc_with_expected("kademlia_routing_table_peers", None, sigil_peer_id)
                .await
                .unwrap();

            sigil_instance
                .rpc_with_expected("gossipsub_mesh_peers", None, sigil_peer_id)
                .await
                .unwrap();
        }
    }
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
