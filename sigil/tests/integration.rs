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

async fn get_container_logs(container: &ContainerAsync<GenericImage>) -> String {
    match container.stdout_to_vec().await {
        Ok(log_bytes) => String::from_utf8_lossy(&log_bytes).into_owned(),
        Err(e) => format!("Failed to retrieve container logs: {}", e),
    }
}

#[tokio::test]
async fn test_sigil() {
    let container = GenericImage::new("sigil", "dev")
        .with_exposed_port(3030.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Sigil is alive."))
        .start()
        .await
        .expect("Failed to start sigil container");

    if let Err(e) = async {
        let host_port = container
            .get_host_port_ipv4(3030)
            .await
            .context("Failed to get host port")?;

        let client = reqwest::Client::new();
        let response = client
            .post(&format!("http://localhost:{}", host_port))
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "say_hello",
                "params": ["Sigil"],
                "id": 1
            }))
            .send()
            .await
            .context("Failed to send request")?;

        let body = response
            .text()
            .await
            .context("Failed to get response body")?;

        if !body.contains("Hello, Sigil!") {
            anyhow::bail!("Response does not contain expected text");
        }

        Ok::<(), anyhow::Error>(())
    }
    .await
    {
        let logs = get_container_logs(&container).await;
        panic!("Test failed: {}. Container logs:\n{}", e, logs);
    }
}
