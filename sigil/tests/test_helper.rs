use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use testcontainers::{
    core::{ContainerAsync, IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
use tokio::time::Duration;

// TODO: there are a lot of constansts in here.  They will either be params or real CONST vars.

pub struct SigilTestInstance {
    container: ContainerAsync<GenericImage>,
    pub host_port: u16,
    reqwest_client: reqwest::Client,
}

impl SigilTestInstance {
    pub async fn new(config_file: &str) -> Self {
        let config_toml_path = format!("test_configs/{config_file}");
        let port = 3030;

        // TODO: how can we pass in different sigil.toml files to test different configurations?
        // TODO: also how can we run sigil in the container with RUST_LOG=priory=trace,warn ?
        let container = GenericImage::new("sigil", "dev")
            .with_exposed_port(port.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Sigil is alive."))
            .with_env_var("RUST_LOG", "priory=trace,warn")
            .with_env_var("CONFIG_TOML_PATH", config_toml_path)
            .start()
            .await
            .expect("Failed to start sigil container");

        // give it a chance to make connections or crash
        tokio::time::sleep(Duration::from_millis(1000)).await;

        let host_port = container
            .get_host_port_ipv4(port)
            .await
            .expect("Failed to get host port");

        let internal_port = container
            .ports()
            .await
            .context("get internal ports")
            .unwrap();

        let reqwest_client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap();

        Self {
            container,
            host_port,
            reqwest_client,
        }
    }

    // make an rpc call and dump container logs if the response doesn't contain some expected value
    pub async fn rpc_with_expected(
        &self,
        method: &str,
        params: Option<&str>,
        expected: &str,
    ) -> Result<()> {
        let response = self.rpc(method, params).await?;

        if !response.contains(expected) {
            anyhow::bail!(
                "Response to method {method} with params {params:?} does not contain expected text. \nexpected: {}\nactual: {}\n",
                expected,
                response
            );
        }

        Ok(())
    }

    // makes the rpc call and returns just the body
    pub async fn rpc(&self, method: &str, params: Option<&str>) -> Result<String> {
        let params = params.unwrap_or("");

        match self.rpc_impl(method, params).await {
            Ok(res) => Ok(res),
            Err(e) => {
                let logs = self.get_container_logs().await;
                Err(anyhow!("Test failed: {}\nContainer logs:\n{}", e, logs))
            }
        }
    }

    async fn rpc_impl(&self, method: &str, params: &str) -> Result<String> {
        let response = self
            .reqwest_client
            .post(&format!("http://localhost:{}", self.host_port))
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": [params],
                "id": 1
            }))
            .send()
            .await
            .context(format!(
                "Failed to send rpc request for {} with params {}",
                method, params
            ))?;

        let body = response
            .text()
            .await
            .context("Failed to get response body")?;

        let json: Value = serde_json::from_str(&body)
            .context(format!("parse rpc response from {method} as json"))?;

        json["result"]
            .as_str()
            .context(format!(
                "extract result field from json response to {method}"
            ))
            .map(|s| s.into())
    }

    pub async fn get_container_logs(&self) -> String {
        let std_out = match self.container.stdout_to_vec().await {
            Ok(log_bytes) => String::from_utf8_lossy(&log_bytes).into_owned(),
            Err(e) => format!("Failed to retrieve container logs: {}", e),
        };

        let std_err = match self.container.stderr_to_vec().await {
            Ok(log_bytes) => String::from_utf8_lossy(&log_bytes).into_owned(),
            Err(e) => format!("Failed to retrieve container logs: {}", e),
        };

        format!("{std_out}\n{std_err}")
    }
}
