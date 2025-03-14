use alloy_primitives::{hex, B256};

use anyhow::Result;
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use log::{error, info};
use network_lib::{
    AggProofRequest, AppError, ProofResponse, ProofStatus, SpanProofRequest, WorkerState,
};
use op_succinct_client_utils::boot::BootInfoStruct;
use op_succinct_host_utils::{
    fetcher::{CacheMode, OPSuccinctDataFetcher, RunContext},
    get_agg_proof_stdin, get_proof_stdin, start_server_and_native_client, ProgramType,
};
use sp1_sdk::{utils, Prover, ProverClient, SP1Proof, SP1ProofWithPublicValues};
use std::{env, sync::Arc};
use tower_http::limit::RequestBodyLimitLayer;
pub const RANGE_ELF: &[u8] = include_bytes!("../../../../elf/range-elf");
pub const AGG_ELF: &[u8] = include_bytes!("../../../../elf/aggregation-elf");

#[tokio::main]
async fn main() -> Result<()> {
    // Enable logging.
    env::set_var("RUST_LOG", "info");

    // Set up the SP1 SDK logger.
    utils::setup_logger();
    dotenv::dotenv().ok();

    let cuda_prover = Arc::new(ProverClient::builder().cuda().build());
    let (_range_pk, range_vk) = cuda_prover.setup(RANGE_ELF);
    let (_agg_pk, _agg_vk) = cuda_prover.setup(AGG_ELF);

    // local cuda prover setup for fallback
    let cuda_prover = Arc::new(ProverClient::builder().cuda().build());

    let worker_state = WorkerState {
        range_vk: Arc::new(range_vk),
        cuda_prover,
    };

    let app = Router::new()
        .route("/request_span_proof", post(request_span_proof))
        .route("/request_agg_proof", post(request_agg_proof))
        .route("/status/:proof_id", get(get_proof_status))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(102400 * 1024 * 1024))
        .with_state(worker_state);

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    // TODO: send a ping of 'ready' to the coordinator
    info!("Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await?;
    Ok(())
}

async fn request_span_proof(
    State(state): State<WorkerState>,
    Json(payload): Json<SpanProofRequest>,
) -> Result<(StatusCode, Json<ProofResponse>), AppError> {
    let fetcher = match OPSuccinctDataFetcher::new_with_rollup_config(RunContext::Docker).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create data fetcher: {}", e);
            return Err(AppError(e));
        }
    };

    let host_args = match fetcher
        .get_host_args(
            payload.start,
            payload.end,
            None,
            ProgramType::Multi,
            CacheMode::DeleteCache,
        )
        .await
    {
        Ok(cli) => cli,
        Err(e) => {
            error!("Failed to get host CLI args: {}", e);
            return Err(AppError(anyhow::anyhow!(
                "Failed to get host CLI args: {}",
                e
            )));
        }
    };

    let mem_kv_store = start_server_and_native_client(host_args).await?;

    let sp1_stdin = match get_proof_stdin(mem_kv_store) {
        Ok(stdin) => stdin,
        Err(e) => {
            error!("Failed to get proof stdin: {}", e);
            return Err(AppError(anyhow::anyhow!(
                "Failed to get proof stdin: {}",
                e
            )));
        }
    };

    let (proving_key, _) = state.cuda_prover.setup(RANGE_ELF);
    // TODO: don't await, spawn a task
    state
        .cuda_prover
        .prove(&proving_key, &sp1_stdin)
        .compressed()
        .run()?;

    let proof_id = B256::random();
    Ok((
        StatusCode::OK,
        Json(ProofResponse {
            proof_id: proof_id.to_vec(),
        }),
    ))
}

async fn request_agg_proof(
    State(state): State<WorkerState>,
    Json(payload): Json<AggProofRequest>,
) -> Result<(StatusCode, Json<ProofResponse>), AppError> {
    let mut proofs_with_pv: Vec<SP1ProofWithPublicValues> = payload
        .subproofs
        .iter()
        .map(|sp| bincode::deserialize(sp).unwrap())
        .collect();

    let boot_infos: Vec<BootInfoStruct> = proofs_with_pv
        .iter_mut()
        .map(|proof| proof.public_values.read())
        .collect();

    let proofs: Vec<SP1Proof> = proofs_with_pv
        .iter_mut()
        .map(|proof| proof.proof.clone())
        .collect();

    let l1_head_bytes = match payload.head.strip_prefix("0x") {
        Some(hex_str) => match hex::decode(hex_str) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to decode L1 head hex string: {}", e);
                return Err(AppError(anyhow::anyhow!(
                    "Failed to decode L1 head hex string: {}",
                    e
                )));
            }
        },
        None => {
            error!("Invalid L1 head format: missing 0x prefix");
            return Err(AppError(anyhow::anyhow!(
                "Invalid L1 head format: missing 0x prefix"
            )));
        }
    };

    let l1_head: [u8; 32] = match l1_head_bytes.clone().try_into() {
        Ok(array) => array,
        Err(_) => {
            error!(
                "Invalid L1 head length: expected 32 bytes, got {}",
                l1_head_bytes.len()
            );
            return Err(AppError(anyhow::anyhow!(
                "Invalid L1 head length: expected 32 bytes, got {}",
                l1_head_bytes.len()
            )));
        }
    };

    let fetcher = match OPSuccinctDataFetcher::new_with_rollup_config(RunContext::Docker).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create fetcher: {}", e);
            return Err(AppError(anyhow::anyhow!("Failed to create fetcher: {}", e)));
        }
    };

    let headers = match fetcher
        .get_header_preimages(&boot_infos, l1_head.into())
        .await
    {
        Ok(h) => h,
        Err(e) => {
            error!("Failed to get header preimages: {}", e);
            return Err(AppError(anyhow::anyhow!(
                "Failed to get header preimages: {}",
                e
            )));
        }
    };

    let sp1_stdin =
        match get_agg_proof_stdin(proofs, boot_infos, headers, &state.range_vk, l1_head.into()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get agg proof stdin: {}", e);
                return Err(AppError(anyhow::anyhow!(
                    "Failed to get agg proof stdin: {}",
                    e
                )));
            }
        };

    let (proving_key, _) = state.cuda_prover.setup(AGG_ELF);
    // TODO: spawn a task
    state
        .cuda_prover
        .prove(&proving_key, &sp1_stdin)
        .groth16()
        .run()?;

    let proof_id = B256::random();
    Ok((
        StatusCode::OK,
        Json(ProofResponse {
            proof_id: proof_id.to_vec(),
        }),
    ))
}

async fn get_proof_status(
    State(state): State<WorkerState>,
    Path(proof_id): Path<String>,
) -> Result<(StatusCode, Json<ProofStatus>), AppError> {
    todo!()
}
