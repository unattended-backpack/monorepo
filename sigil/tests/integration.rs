use reqwest::Client;
use serde_json::json;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage,
};

#[tokio::test]
async fn test_sigil() {
    let container = GenericImage::new("sigil", "dev")
        .with_exposed_port(3030.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Local node is listening on"))
        .start()
        .await
        .expect("sigil should start");

    println!("sigil is alive");

    // Get the host port that's mapped to the container's port 3030.
    let host_port = container
        .get_host_port_ipv4(3030)
        .await
        .expect("container port should be exposed");

    println!("container exposed on {}", host_port);

    // Create a client to interact with the RPC endpoint.
    let client = Client::new();

    // Make an RPC call
    let response = client
        .post(&format!("http://localhost:{}", host_port))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "say_hello",
            "params": ["Sigil"],
            "id": 1
        }))
        .send()
        .await
        .expect("request should send");

    let body = response.text().await.expect("response should arrive");

    // Assert on the response
    assert!(body.contains("Hello, Sigil!"));
}
