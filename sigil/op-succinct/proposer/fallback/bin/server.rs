use alloy_primitives::{hex, Address, B256};
use anyhow::{Context, Result};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use log::{error, info};
use op_succinct_client_utils::{
    boot::{hash_rollup_config, BootInfoStruct},
    types::u32_to_u8,
};
use op_succinct_fallback_proposer::{
    request_with_retries, AggProofRequest, ProofCache, ProofResponse, ProofStatus, ProofStore,
    ProofType, SpanProofRequest, SuccinctProposerConfig, ValidateConfigRequest,
    ValidateConfigResponse,
};
use op_succinct_host_utils::{
    fetcher::{CacheMode, OPSuccinctDataFetcher, RunContext},
    get_agg_proof_stdin, get_proof_stdin, start_server_and_native_client,
    stats::ExecutionStats,
    L2OutputOracle, ProgramType,
};
use sp1_sdk::{
    network::{
        proto::network::{ExecutionStatus, FulfillmentStatus},
        FulfillmentStrategy,
    },
    utils, CudaProver, HashableKey, Prover, ProverClient, SP1Proof, SP1ProofMode,
    SP1ProofWithPublicValues, SP1Stdin, SP1_CIRCUIT_VERSION,
};
use std::{
    collections::HashMap,
    env, fs,
    str::FromStr,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tower_http::limit::RequestBodyLimitLayer;

pub const RANGE_ELF: &[u8] = include_bytes!("../../../elf/range-elf");
pub const AGG_ELF: &[u8] = include_bytes!("../../../elf/aggregation-elf");

#[tokio::main]
async fn main() -> Result<()> {
    // Enable logging.
    env::set_var("RUST_LOG", "info");

    // Set up the SP1 SDK logger.
    utils::setup_logger();
    dotenv::dotenv().ok();

    // network prover setup
    let network_prover = Arc::new(ProverClient::builder().network().build());
    let (range_pk, range_vk) = network_prover.setup(RANGE_ELF);
    let (agg_pk, agg_vk) = network_prover.setup(AGG_ELF);
    let multi_block_vkey_u8 = u32_to_u8(range_vk.vk.hash_u32());
    let range_vkey_commitment = B256::from(multi_block_vkey_u8);
    let agg_vkey_hash = B256::from_str(&agg_vk.bytes32()).unwrap();

    // local cuda prover setup for fallback
    let cuda_prover = Arc::new(ProverClient::builder().cuda().build());
    let proof_store = Arc::new(RwLock::new(HashMap::new()));

    let fetcher = OPSuccinctDataFetcher::new_with_rollup_config(RunContext::Docker).await?;
    // Note: The rollup config hash never changes for a given chain, so we can just hash it once at
    // server start-up. The only time a rollup config changes is typically when a new version of the
    // [`RollupConfig`] is released from `op-alloy`.
    let rollup_config_hash = hash_rollup_config(fetcher.rollup_config.as_ref().unwrap());

    // Set the proof strategies based on environment variables. Default to reserved to keep existing behavior.
    let range_proof_strategy = match env::var("RANGE_PROOF_STRATEGY") {
        Ok(strategy) if strategy.to_lowercase() == "hosted" => FulfillmentStrategy::Hosted,
        _ => FulfillmentStrategy::Reserved,
    };
    let agg_proof_strategy = match env::var("AGG_PROOF_STRATEGY") {
        Ok(strategy) if strategy.to_lowercase() == "hosted" => FulfillmentStrategy::Hosted,
        _ => FulfillmentStrategy::Reserved,
    };

    // Set the aggregation proof type based on environment variable. Default to groth16.
    let agg_proof_mode = match env::var("AGG_PROOF_MODE") {
        Ok(proof_type) if proof_type.to_lowercase() == "plonk" => SP1ProofMode::Plonk,
        _ => SP1ProofMode::Groth16,
    };

    // defaults to false, but can be set to true to never make succinct prover network requests
    let local_proving_only = match env::var("LOCAL_PROVING_ONLY") {
        Ok(on) if on.to_lowercase() == "true" => true,
        _ => false,
    };

    // defaults to 3 retries
    let prover_network_retries: usize = match env::var("PROVER_NETWORK_RETRIES") {
        Ok(retries) => retries
            .parse()
            .context("parse PROVER_NETWORK_RETRIES as usize")?,
        _ => 3,
    };

    // the amount of proofs to cache on-disk for persistence
    // Stores the most recent PROOF_CACHE_MAX_SIZE completed proofs
    // defaults to 10
    // setting it to 0 disables the cache
    let proof_cache_size: usize = match env::var("PROOF_CACHE_MAX_SIZE") {
        Ok(size) => size
            .parse()
            .context("parse PROOF_CACHE_MAX_SIZE as usize")?,
        _ => 10,
    };

    let proof_cache = Arc::new(RwLock::new(
        ProofCache::new(proof_cache_size).context("Create proof cache")?,
    ));

    // Initialize global hashes.
    let global_hashes = SuccinctProposerConfig {
        agg_vkey_hash,
        range_vkey_commitment,
        rollup_config_hash,
        range_vk: Arc::new(range_vk),
        range_pk: Arc::new(range_pk),
        agg_vk: Arc::new(agg_vk),
        agg_pk: Arc::new(agg_pk),
        range_proof_strategy,
        agg_proof_strategy,
        agg_proof_mode,
        network_prover,
        prover_network_retries,
        proof_store,
        cuda_prover,
        local_proving_only,
        proof_cache,
    };

    let app = Router::new()
        .route("/request_span_proof", post(request_span_proof))
        .route("/request_agg_proof", post(request_agg_proof))
        .route("/request_mock_span_proof", post(request_mock_span_proof))
        .route("/request_mock_agg_proof", post(request_mock_agg_proof))
        .route("/status/:proof_id", get(get_proof_status))
        .route("/validate_config", post(validate_config))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(102400 * 1024 * 1024))
        .with_state(global_hashes);

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    info!("Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await?;
    Ok(())
}

/// Validate the configuration of the L2 Output Oracle.
async fn validate_config(
    State(state): State<SuccinctProposerConfig>,
    Json(payload): Json<ValidateConfigRequest>,
) -> Result<(StatusCode, Json<ValidateConfigResponse>), AppError> {
    info!("Received validate config request: {:?}", payload);
    let fetcher = OPSuccinctDataFetcher::default();

    let address = Address::from_str(&payload.address).unwrap();
    let l2_output_oracle = L2OutputOracle::new(address, fetcher.l1_provider);

    let agg_vkey = l2_output_oracle.aggregationVkey().call().await?;
    let range_vkey = l2_output_oracle.rangeVkeyCommitment().call().await?;
    let rollup_config_hash = l2_output_oracle.rollupConfigHash().call().await?;

    let agg_vkey_valid = agg_vkey.aggregationVkey == state.agg_vkey_hash;
    let range_vkey_valid = range_vkey.rangeVkeyCommitment == state.range_vkey_commitment;
    let rollup_config_hash_valid = rollup_config_hash.rollupConfigHash == state.rollup_config_hash;

    Ok((
        StatusCode::OK,
        Json(ValidateConfigResponse {
            rollup_config_hash_valid,
            agg_vkey_valid,
            range_vkey_valid,
        }),
    ))
}

/// Request a proof for a span of blocks.
async fn request_span_proof(
    State(state): State<SuccinctProposerConfig>,
    Json(payload): Json<SpanProofRequest>,
) -> Result<(StatusCode, Json<ProofResponse>), AppError> {
    info!("Received span proof request: {:?}", payload);
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

    // get read only copy of proof cache
    let proof_cache = state.proof_cache.read().await;
    let proof_exists_on_disk = proof_cache.does_proof_exist_by_request(&payload);
    let proof_exists_in_memory = proof_cache.drop(proof_cache);

    let proof_id = if proof_exists_on_disk {
        // a proof with these parameters exists on disk from a previous run, skip proof generation
        // generate a new proof_id, put it into the cache,
        B256::random()
    } else {
        route_proof(ProofType::Span, &state, sp1_stdin).await?
    };

    // get write lock
    let mut proof_cache = state.proof_cache.write().await;
    proof_cache.record_proof_request(&proof_id, &payload);

    Ok((
        StatusCode::OK,
        Json(ProofResponse {
            proof_id: proof_id.to_vec(),
        }),
    ))
}

/// Request an aggregation proof for a set of subproofs.
async fn request_agg_proof(
    State(state): State<SuccinctProposerConfig>,
    Json(payload): Json<AggProofRequest>,
) -> Result<(StatusCode, Json<ProofResponse>), AppError> {
    info!("Received agg proof request");
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

    let proof_id = route_proof(ProofType::Agg, &state, sp1_stdin).await?;

    Ok((
        StatusCode::OK,
        Json(ProofResponse {
            proof_id: proof_id.to_vec(),
        }),
    ))
}

/// Request a mock proof for a span of blocks.
async fn request_mock_span_proof(
    State(state): State<SuccinctProposerConfig>,
    Json(payload): Json<SpanProofRequest>,
) -> Result<(StatusCode, Json<ProofStatus>), AppError> {
    info!("Received mock span proof request: {:?}", payload);
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
            return Err(AppError(e));
        }
    };

    let start_time = Instant::now();
    let oracle = start_server_and_native_client(host_args.clone()).await?;
    let witness_generation_duration = start_time.elapsed();

    let sp1_stdin = match get_proof_stdin(oracle) {
        Ok(stdin) => stdin,
        Err(e) => {
            error!("Failed to get proof stdin: {}", e);
            return Err(AppError(e));
        }
    };

    let start_time = Instant::now();

    // Note(ratan): In a future version of the server which only supports mock proofs, Arc<MockProver> should be used to reduce memory usage.
    let prover = ProverClient::builder().mock().build();
    let (pv, report) = prover.execute(RANGE_ELF, &sp1_stdin).run().unwrap();
    let execution_duration = start_time.elapsed();

    let block_data = fetcher
        .get_l2_block_data_range(payload.start, payload.end)
        .await?;

    let l1_head = host_args.kona_args.l1_head;
    // Get the L1 block number from the L1 head.
    let l1_block_number = fetcher.get_l1_header(l1_head.into()).await?.number;
    let stats = ExecutionStats::new(
        l1_block_number,
        &block_data,
        &report,
        witness_generation_duration.as_secs(),
        execution_duration.as_secs(),
    );

    let l2_chain_id = fetcher.get_l2_chain_id().await?;
    // Save the report to disk.
    let report_dir = format!("execution-reports/{}", l2_chain_id);
    if !std::path::Path::new(&report_dir).exists() {
        fs::create_dir_all(&report_dir)?;
    }

    let report_path = format!("{}/{}-{}.json", report_dir, payload.start, payload.end);
    // Write to CSV.
    let mut csv_writer = csv::Writer::from_path(report_path)?;
    csv_writer.serialize(&stats)?;
    csv_writer.flush()?;

    let proof = SP1ProofWithPublicValues::create_mock_proof(
        &state.range_pk,
        pv.clone(),
        SP1ProofMode::Compressed,
        SP1_CIRCUIT_VERSION,
    );

    let proof_bytes = bincode::serialize(&proof).unwrap();

    Ok((
        StatusCode::OK,
        Json(ProofStatus {
            fulfillment_status: FulfillmentStatus::Fulfilled.into(),
            execution_status: ExecutionStatus::UnspecifiedExecutionStatus.into(),
            proof: proof_bytes,
        }),
    ))
}

/// Request mock aggregation proof.
async fn request_mock_agg_proof(
    State(state): State<SuccinctProposerConfig>,
    Json(payload): Json<AggProofRequest>,
) -> Result<(StatusCode, Json<ProofStatus>), AppError> {
    info!("Received mock agg proof request!");

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

    let l1_head_bytes = match hex::decode(
        payload
            .head
            .strip_prefix("0x")
            .expect("Invalid L1 head, no 0x prefix."),
    ) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to decode L1 head: {}", e);
            return Err(AppError(anyhow::anyhow!("Failed to decode L1 head: {}", e)));
        }
    };
    let l1_head: [u8; 32] = l1_head_bytes.try_into().unwrap();

    let fetcher = match OPSuccinctDataFetcher::new_with_rollup_config(RunContext::Docker).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create data fetcher: {}", e);
            return Err(AppError(e));
        }
    };
    let headers = match fetcher
        .get_header_preimages(&boot_infos, l1_head.into())
        .await
    {
        Ok(h) => h,
        Err(e) => {
            error!("Failed to get header preimages: {}", e);
            return Err(AppError(e));
        }
    };

    let stdin =
        match get_agg_proof_stdin(proofs, boot_infos, headers, &state.range_vk, l1_head.into()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get aggregation proof stdin: {}", e);
                return Err(AppError(e));
            }
        };

    // Note(ratan): In a future version of the server which only supports mock proofs, Arc<MockProver> should be used to reduce memory usage.
    let prover = ProverClient::builder().mock().build();
    let proof = match prover
        .prove(&state.agg_pk, &stdin)
        .mode(state.agg_proof_mode)
        .deferred_proof_verification(false)
        .run()
    {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to generate proof: {}", e);
            return Err(AppError(e));
        }
    };

    Ok((
        StatusCode::OK,
        Json(ProofStatus {
            fulfillment_status: FulfillmentStatus::Fulfilled.into(),
            execution_status: ExecutionStatus::UnspecifiedExecutionStatus.into(),
            proof: proof.bytes(),
        }),
    ))
}

/// Get the status of a proof.
async fn get_proof_status(
    State(state): State<SuccinctProposerConfig>,
    Path(proof_id): Path<String>,
) -> Result<(StatusCode, Json<ProofStatus>), AppError> {
    info!("Received proof status request: {:?}", proof_id);

    let proof_id_bytes = hex::decode(&proof_id)?;
    let proof_id = B256::from_slice(&proof_id_bytes);

    // request read-only copy of proof_store
    let proof_store = state.proof_store.read().await;

    // first check if this is a proof we're generating locally.  Otherwise check the network for it
    if let Some(status) = proof_store.get(&proof_id) {
        return Ok((
            StatusCode::OK,
            Json(ProofStatus {
                fulfillment_status: status.fulfillment_status,
                execution_status: status.execution_status,
                proof: status.proof.clone(),
            }),
        ));
    }

    if state.local_proving_only {
        // we should never get here.  If we're local proving only and a proof that was requested
        // wasn't found locally we should send a response that prompts the proposer to retry that
        // proof request.
        error!(
            "Status of proof {} not found locally.  Returning response to proposer to prompt retry",
            proof_id
        );

        // When the proposer sees a FulfillmentStatus of Unfulfillable it will retry the proof
        // request.  If the prover network is really down, the new request will fail inside
        // `request_agg/span_proof()` and will be re-routed as a local proof
        return Ok((
            StatusCode::OK,
            Json(ProofStatus {
                fulfillment_status: FulfillmentStatus::Unfulfillable.into(),
                // if execution status is also `Unexecutable`, then the retry proof request
                // will be half the block size.  This keeps the request the same
                execution_status: ExecutionStatus::UnspecifiedExecutionStatus.into(),
                proof: vec![],
            }),
        ));
    }

    let network_proof_status_request = || state.network_prover.get_proof_status(proof_id);

    // try to get the proof status, returning a failure to the proposer if it seems that the prover
    // network is down
    let (status, maybe_proof) = match request_with_retries(
        state.prover_network_retries,
        network_proof_status_request,
    )
    .await
    {
        Ok(res) => res,
        Err(_) => {
            error!(
                "Prover network request for status of proof {} failed",
                proof_id
            );

            // When the proposer sees a FulfillmentStatus of Unfulfillable it will retry the proof
            // request.  If the prover network is really down, the new request will fail inside
            // `request_agg/span_proof()` and will be re-routed as a local proof
            return Ok((
                StatusCode::OK,
                Json(ProofStatus {
                    fulfillment_status: FulfillmentStatus::Unfulfillable.into(),
                    // if execution status is also `Unexecutable`, then the retry proof request
                    // will be half the block size.  This keeps the request the same
                    execution_status: ExecutionStatus::UnspecifiedExecutionStatus.into(),
                    proof: vec![],
                }),
            ));
        }
    };

    /* FOR REFERENCE
    #[repr(i32)]
    pub enum ExecutionStatus {
        UnspecifiedExecutionStatus = 0,
        /// The request has not been executed.
        Unexecuted = 1,
        /// The request has been executed.
        Executed = 2,
        /// The request cannot be executed.
        Unexecutable = 3,
    }

    #[repr(i32)]
    pub enum FulfillmentStatus {
        UnspecifiedFulfillmentStatus = 0,
        /// The request has been requested.
        Requested = 1,
        /// The request has been assigned to a fulfiller.
        Assigned = 2,
        /// The request has been fulfilled.
        Fulfilled = 3,
        /// The request cannot be fulfilled.
        Unfulfillable = 4,
    }
    * */

    // Check the deadline.
    if status.deadline
        < SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    {
        error!(
            "Proof request timed out on the server. Default timeout is set to 4 hours. Returning status as Unfulfillable."
        );
        return Ok((
            StatusCode::OK,
            Json(ProofStatus {
                fulfillment_status: FulfillmentStatus::Unfulfillable.into(),
                execution_status: ExecutionStatus::Executed.into(),
                proof: vec![],
            }),
        ));
    }

    let fulfillment_status = status.fulfillment_status;
    let execution_status = status.execution_status;
    if fulfillment_status == FulfillmentStatus::Fulfilled as i32 {
        let proof: SP1ProofWithPublicValues = maybe_proof.unwrap();

        match proof.proof {
            SP1Proof::Compressed(_) => {
                // If it's a compressed proof, we need to serialize the entire struct with bincode.
                // Note: We're re-serializing the entire struct with bincode here, but this is fine
                // because we're on localhost and the size of the struct is small.
                let proof_bytes = bincode::serialize(&proof).unwrap();
                return Ok((
                    StatusCode::OK,
                    Json(ProofStatus {
                        fulfillment_status,
                        execution_status,
                        proof: proof_bytes,
                    }),
                ));
            }
            SP1Proof::Groth16(_) => {
                // If it's a groth16 proof, we need to get the proof bytes that we put on-chain.
                let proof_bytes = proof.bytes();
                return Ok((
                    StatusCode::OK,
                    Json(ProofStatus {
                        fulfillment_status,
                        execution_status,
                        proof: proof_bytes,
                    }),
                ));
            }
            SP1Proof::Plonk(_) => {
                // If it's a plonk proof, we need to get the proof bytes that we put on-chain.
                let proof_bytes = proof.bytes();
                return Ok((
                    StatusCode::OK,
                    Json(ProofStatus {
                        fulfillment_status,
                        execution_status,
                        proof: proof_bytes,
                    }),
                ));
            }
            _ => (),
        }
    } else if fulfillment_status == FulfillmentStatus::Unfulfillable as i32 {
        return Ok((
            StatusCode::OK,
            Json(ProofStatus {
                fulfillment_status,
                execution_status,
                proof: vec![],
            }),
        ));
    }
    Ok((
        StatusCode::OK,
        Json(ProofStatus {
            fulfillment_status,
            execution_status,
            proof: vec![],
        }),
    ))
}

// if LOCAL_PROVING_ONLY is set to true, request a local proof
// otherwise, make a request to the prover network.  If this request
// fails PROVER_NETWORK_RETRIES times, then fall back to local proving
async fn route_proof(
    proof_type: ProofType,
    state: &SuccinctProposerConfig,
    sp1_stdin: SP1Stdin,
) -> Result<B256, AppError> {
    if state.local_proving_only {
        locally_prove(
            proof_type,
            state.proof_store.clone(),
            state.cuda_prover.clone(),
            sp1_stdin,
        )
        .await
    } else {
        let prover_network_response = match proof_type {
            ProofType::Span => {
                let network_request = || {
                    state
                        .network_prover
                        .prove(&state.range_pk, &sp1_stdin)
                        .compressed()
                        .strategy(state.range_proof_strategy)
                        .skip_simulation(true)
                        .cycle_limit(1_000_000_000_000)
                        .request_async()
                };
                request_with_retries(state.prover_network_retries, network_request).await
            }
            ProofType::Agg => {
                let network_request = || {
                    state
                        .network_prover
                        .prove(&state.agg_pk, &sp1_stdin)
                        .mode(state.agg_proof_mode)
                        .strategy(state.agg_proof_strategy)
                        .request_async()
                };
                request_with_retries(state.prover_network_retries, network_request).await
            }
        };

        // request the future a number of times
        match prover_network_response {
            // success
            Ok(proof_id) => Ok(proof_id),
            // failed after PROVER_NETWORK_RETRIES retries, make the proof locally
            Err(_) => {
                error!(
                    "Prover network request for {} proof failed. Requesting proof locally.",
                    proof_type
                );

                locally_prove(
                    proof_type,
                    state.proof_store.clone(),
                    state.cuda_prover.clone(),
                    sp1_stdin,
                )
                .await
            }
        }
    }
}

// spawns a process that creates a proof locally
// runs proof in a background thread.  Only needs to be async because of
// proof_store.write().await.  It isn't blocked by anything else.
async fn locally_prove(
    proof_type: ProofType,
    proof_store: ProofStore,
    cuda_prover: Arc<CudaProver>,
    sp1_stdin: SP1Stdin,
) -> Result<B256, AppError> {
    let proof_id = B256::random();

    let initial_status = ProofStatus {
        fulfillment_status: FulfillmentStatus::Assigned.into(),
        execution_status: ExecutionStatus::Unexecuted.into(),
        proof: Vec::new(),
    };

    proof_store.write().await.insert(proof_id, initial_status);

    tokio::spawn(async move {
        let start_time = tokio::time::Instant::now();
        info!("computing {proof_type} proof with id {:?}", proof_id);

        let proof_res = match proof_type {
            ProofType::Span => {
                // the cuda prover keeps state of the last `setup()` that was called on it.
                // You must call `setup()` then `prove` *each* time you intend to
                // prove a certain program
                let (proving_key, _) = cuda_prover.setup(RANGE_ELF);
                cuda_prover
                    .prove(&proving_key, &sp1_stdin)
                    .compressed()
                    .run()
            }
            ProofType::Agg => {
                // the cuda prover keeps state of the last `setup()` that was called on it.
                // You must call `setup()` then `prove` *each* time you intend to
                // prove a certain program
                let (proving_key, _) = cuda_prover.setup(AGG_ELF);
                cuda_prover.prove(&proving_key, &sp1_stdin).groth16().run()
            }
        };

        let proof_status = match proof_res {
            // proof is done, can return it
            Ok(proof) => {
                match proof.proof {
                    SP1Proof::Compressed(_) => {
                        // If it's a compressed proof, we need to serialize the entire struct with bincode.
                        // Note: We're re-serializing the entire struct with bincode here, but this is fine
                        // because we're on localhost and the size of the struct is small.
                        let proof_bytes = bincode::serialize(&proof).unwrap();
                        ProofStatus {
                            fulfillment_status: FulfillmentStatus::Fulfilled.into(),
                            execution_status: ExecutionStatus::Executed.into(),
                            proof: proof_bytes,
                        }
                    }
                    SP1Proof::Groth16(_) => {
                        // If it's a groth16 proof, we need to get the proof bytes that we put on-chain.
                        let proof_bytes = proof.bytes();
                        ProofStatus {
                            fulfillment_status: FulfillmentStatus::Fulfilled.into(),
                            execution_status: ExecutionStatus::Executed.into(),
                            proof: proof_bytes,
                        }
                    }
                    SP1Proof::Plonk(_) => {
                        // If it's a plonk proof, we need to get the proof bytes that we put on-chain.
                        let proof_bytes = proof.bytes();
                        ProofStatus {
                            fulfillment_status: FulfillmentStatus::Fulfilled.into(),
                            execution_status: ExecutionStatus::Executed.into(),
                            proof: proof_bytes,
                        }
                    }
                    _ => {
                        log::error!("unknown proof type: {proof:?}");
                        return Err(AppError(anyhow::anyhow!("unknown proof type: {proof:?}")));
                    }
                }
            }
            Err(e) => {
                log::error!("error proving {e}");
                return Err(AppError(anyhow::anyhow!("error proving {e}")));
            }
        };

        info!("proof completed. id {:?}", proof_id);
        let minutes = start_time.elapsed().as_secs_f64() / 60.0;
        info!("Time to compute {proof_type} proof: {} minutes", minutes);

        // update proof store
        proof_store.write().await.insert(proof_id, proof_status);

        Ok(())
    });

    Ok(proof_id)
}

pub struct AppError(anyhow::Error);

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
