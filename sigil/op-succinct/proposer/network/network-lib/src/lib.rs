pub mod proof_cache;
pub mod proof_request_cache;
pub mod worker_registry;

use alloy_primitives::B256;
use anyhow::anyhow;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose, Engine as _};
use log::error;
pub use proof_cache::ProofCache;
pub use proof_request_cache::ProofRequestCacheClient;
use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sp1_sdk::{
    network::{
        proto::network::{ExecutionStatus, FulfillmentStatus},
        FulfillmentStrategy,
    },
    EnvProver, NetworkProver, SP1ProofMode, SP1ProvingKey, SP1VerifyingKey,
};
use std::{collections::HashMap, fmt::Display, future::Future, sync::Arc};
use tokio::sync::RwLock;
pub use worker_registry::{WorkerRegistryClient, WorkerState};

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

#[derive(Deserialize, Serialize, Debug, Default, Clone, Copy)]
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

#[derive(Deserialize, Serialize, Debug, Default, Clone, Copy)]
pub struct WorkerSpanProofRequest {
    pub mock_mode: bool,
    pub proof_id: B256,
    pub start: u64,
    pub end: u64,
}

// TODO: remove Clone from this.  It's big
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct AggProofRequest {
    #[serde(deserialize_with = "deserialize_base64_vec")]
    pub subproofs: Vec<Vec<u8>>,
    pub head: String,
}

// TODO: remove Clone from this.  It's big
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WorkerAggProofRequest {
    pub mock_mode: bool,
    pub proof_id: B256,
    pub subproofs: Vec<Vec<u8>>,
    pub head: String,
}

impl WorkerAggProofRequest {
    pub fn split_to_generic(self) -> (bool, B256, GenericProofRequest) {
        let mock_mode = self.mock_mode;
        let proof_id = self.proof_id;
        let agg_request = AggProofRequest {
            subproofs: self.subproofs,
            head: self.head,
        };

        (mock_mode, proof_id, GenericProofRequest::Agg(agg_request))
    }
}

#[derive(Clone)]
pub enum GenericProofRequest {
    Span(SpanProofRequest),
    Agg(AggProofRequest),
}

impl From<AggProofRequest> for GenericProofRequest {
    fn from(agg_request: AggProofRequest) -> Self {
        GenericProofRequest::Agg(agg_request)
    }
}

impl From<SpanProofRequest> for GenericProofRequest {
    fn from(span_request: SpanProofRequest) -> Self {
        GenericProofRequest::Span(span_request)
    }
}

impl From<WorkerSpanProofRequest> for GenericProofRequest {
    fn from(span_request: WorkerSpanProofRequest) -> Self {
        let span_request = SpanProofRequest {
            start: span_request.start,
            end: span_request.end,
        };
        GenericProofRequest::Span(span_request)
    }
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

#[derive(Serialize, Deserialize, Debug)]
/// The status of a proof request.
pub struct ProofStatus {
    // Note: Can't use `FulfillmentStatus`/`ExecutionStatus` directly because `Serialize_repr` and `Deserialize_repr` aren't derived on it.
    pub fulfillment_status: i32,
    pub execution_status: i32,
    pub proof: Vec<u8>,
}

impl ProofStatus {
    pub fn lost() -> Self {
        Self {
            fulfillment_status: FulfillmentStatus::UnspecifiedFulfillmentStatus.into(),
            execution_status: ExecutionStatus::UnspecifiedExecutionStatus.into(),
            proof: vec![],
        }
    }

    pub fn is_lost(&self) -> bool {
        self.fulfillment_status == FulfillmentStatus::Unfulfillable as i32
            && self.execution_status == ExecutionStatus::UnspecifiedExecutionStatus as i32
    }
}

impl Display for ProofStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fulfillment_status = match self.fulfillment_status {
            0 => "UnspecifiedFulfillmentStatus",
            1 => "Requested",
            2 => "Assigned",
            3 => "Fulfilled",
            4 => "Unfulfillable",
            _ => "Error: Unknown fulfillment status",
        };

        let execution_status = match self.execution_status {
            0 => "UnspecifiedExecutionStatus",
            1 => "Unexecuted",
            2 => "Executed",
            3 => "Unexecutable",
            _ => "Error: Unknown execution execution status",
        };
        let proof_display = if self.proof.is_empty() {
            "Empty"
        } else {
            "Non-empty"
        };

        write!(
            f,
            "FulfillmentStatus: {}, ExecutionStatus: {}, Proof: {}",
            fulfillment_status, execution_status, proof_display
        )
    }
}

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
    pub local_proving_only: bool,
    pub proof_cache: ProofCacheWrapper,
    pub worker_registry_client: WorkerRegistryClient,
    pub proof_request_cache_client: ProofRequestCacheClient,
    pub mock_mode: bool,
}

pub type ProofStore = Arc<RwLock<HashMap<B256, ProofStatus>>>;

#[derive(Clone)]
pub struct WorkerConfig {
    pub range_vk: Arc<SP1VerifyingKey>,
    pub range_pk: Arc<SP1ProvingKey>,
    pub agg_pk: Arc<SP1ProvingKey>,
    // pub agg_vk: Arc<SP1VerifyingKey>,
    // pub agg_vkey_hash: B256,
    // pub range_vkey_commitment: B256,
    // pub rollup_config_hash: B256,
    // pub range_proof_strategy: FulfillmentStrategy,
    // pub agg_proof_strategy: FulfillmentStrategy,
    pub agg_proof_mode: SP1ProofMode,
    pub proof_store: ProofStore,
    pub prover: Arc<EnvProver>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub ip: String,
    pub port: u16,
}

impl Display for WorkerInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
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
                    "Request retry {}/{} failed: {}",
                    retry_num, max_retries, err
                );
                error!("{}", error_msg);

                last_error = Some(anyhow!("{}", err));
            }
        }
        retry_num += 1;
    }

    Err(anyhow!(
        "All {} requests failed. Last error: {}",
        max_retries,
        last_error.unwrap_or_else(|| anyhow!("Unknown error"))
    ))
}

pub struct AppError(pub anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", self.0)).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
