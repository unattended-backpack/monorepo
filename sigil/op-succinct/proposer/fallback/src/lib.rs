pub mod proof_cache;
use alloy_primitives::B256;
use anyhow::anyhow;
use base64::{engine::general_purpose, Engine as _};
use log::error;
pub use proof_cache::ProofCache;
use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sp1_sdk::{
    network::FulfillmentStrategy, CudaProver, NetworkProver, SP1ProofMode, SP1ProvingKey,
    SP1VerifyingKey,
};
use std::{collections::HashMap, fmt::Display, future::Future, sync::Arc};
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Debug)]
pub struct ValidateConfigRequest {
    pub address: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ValidateConfigResponse {
    pub rollup_config_hash_valid: bool,
    pub agg_vkey_valid: bool,
    pub range_vkey_valid: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SpanProofRequest {
    pub start: u64,
    pub end: u64,
}

impl Display for SpanProofRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "proof start block: {}, proof end block: {}",
            self.start, self.end
        )
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AggProofRequest {
    #[serde(deserialize_with = "deserialize_base64_vec")]
    pub subproofs: Vec<Vec<u8>>,
    pub head: String,
}

pub enum GenericProofRequest {
    Span(SpanProofRequest),
    Agg(AggProofRequest),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MockProofResponse {
    pub proof_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProofResponse {
    pub proof_id: Vec<u8>,
}

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
/// The type of error that occurred when unclaiming a proof. Based off of the `unclaim_description`
/// field in the `ProofStatus` struct.
pub enum UnclaimDescription {
    UnexpectedProverError = 0,
    ProgramExecutionError = 1,
    CycleLimitExceeded = 2,
    Other = 3,
}

/// Convert a string to an `UnclaimDescription`. These cover the common reasons why a proof might
/// be unclaimed.
impl From<String> for UnclaimDescription {
    fn from(description: String) -> Self {
        match description.as_str().to_lowercase().as_str() {
            "unexpected prover error" => UnclaimDescription::UnexpectedProverError,
            "program execution error" => UnclaimDescription::ProgramExecutionError,
            "cycle limit exceeded" => UnclaimDescription::CycleLimitExceeded,
            _ => UnclaimDescription::Other,
        }
    }
}

pub enum ProofType {
    Span,
    Agg,
}

impl Display for ProofType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofType::Span => write!(f, "span"),
            ProofType::Agg => write!(f, "agg"),
        }
    }
}

#[derive(Serialize, Deserialize)]
/// The status of a proof request.
pub struct ProofStatus {
    // Note: Can't use `FulfillmentStatus`/`ExecutionStatus` directly because `Serialize_repr` and `Deserialize_repr` aren't derived on it.
    pub fulfillment_status: i32,
    pub execution_status: i32,
    pub proof: Vec<u8>,
}

pub type ProofStore = Arc<RwLock<HashMap<B256, ProofStatus>>>;
pub type ProofCacheWrapper = Arc<RwLock<ProofCache>>;

/// Configuration of the L2 Output Oracle contract. Created once at server start-up, monitors if there are any changes
/// to the contract's configuration.
#[derive(Clone)]
pub struct SuccinctProposerConfig {
    pub range_vk: Arc<SP1VerifyingKey>,
    pub range_pk: Arc<SP1ProvingKey>,
    pub agg_pk: Arc<SP1ProvingKey>,
    pub agg_vk: Arc<SP1VerifyingKey>,
    pub agg_vkey_hash: B256,
    pub range_vkey_commitment: B256,
    pub rollup_config_hash: B256,
    pub range_proof_strategy: FulfillmentStrategy,
    pub agg_proof_strategy: FulfillmentStrategy,
    pub agg_proof_mode: SP1ProofMode,
    pub network_prover: Arc<NetworkProver>,
    // how many retries on prover network requests until we fall back to local proof.
    pub prover_network_retries: usize,
    // for local proving mode
    pub proof_store: ProofStore,
    pub cuda_prover: Arc<CudaProver>,
    pub local_proving_only: bool,
    pub proof_cache: ProofCacheWrapper,
}

/// Deserialize a vector of base64 strings into a vector of vectors of bytes. Go serializes
/// the subproofs as base64 strings.
fn deserialize_base64_vec<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Vec<String> = Deserialize::deserialize(deserializer)?;
    s.into_iter()
        .map(|base64_str| {
            general_purpose::STANDARD
                .decode(base64_str)
                .map_err(serde::de::Error::custom)
        })
        .collect()
}

pub async fn request_with_retries<F, Fut, T, E>(
    max_retries: usize,
    mut request_fn: F,
) -> Result<T, anyhow::Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Display,
{
    let mut retry_num = 0;
    let mut last_error = None;

    while retry_num < max_retries {
        let request = request_fn();
        match request.await {
            Ok(res) => return Ok(res),
            Err(err) => {
                let error_msg = format!(
                    "Prover network request retry {}/{} failed: {}",
                    retry_num, max_retries, err
                );
                error!("{}", error_msg);

                last_error = Some(anyhow!("{}", err));
            }
        }
        retry_num += 1;
    }

    Err(anyhow!(
        "All {} requests to the prover network failed. Last error: {}",
        max_retries,
        last_error.unwrap_or_else(|| anyhow!("Unknown error"))
    ))
}
