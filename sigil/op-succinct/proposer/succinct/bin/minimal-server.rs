use alloy_primitives::B256;
use anyhow::Result;
use log::{error, info};
use op_succinct_client_utils::{boot::BootInfoStruct, types::u32_to_u8};
use op_succinct_host_utils::{
    fetcher::{CacheMode, OPSuccinctDataFetcher, RunContext},
    get_agg_proof_stdin, get_proof_stdin,
    witnessgen::{WitnessGenExecutor, WITNESSGEN_TIMEOUT},
    ProgramType,
};
use op_succinct_proposer::SpanProofRequest;
use sp1_sdk::{utils, HashableKey, ProverClient, SP1Proof, SP1ProofWithPublicValues};
use std::{fs, str::FromStr};

pub const RANGE_ELF: &[u8] = include_bytes!("../../../elf/range-elf");
pub const AGG_ELF: &[u8] = include_bytes!("../../../elf/aggregation-elf");

#[tokio::main]
async fn main() -> Result<()> {
    let l2_start_block = 12500;
    let l2_end_block = 12501;

    // dummy payload
    let payload = SpanProofRequest {
        start: l2_start_block,
        end: l2_end_block,
    };

    utils::setup_logger();

    dotenv::dotenv().ok();

    let prover = ProverClient::from_env();
    let (range_pk, range_vk) = prover.setup(RANGE_ELF);
    // let (_agg_pk, agg_vk) = prover.setup(AGG_ELF);
    let multi_block_vkey_u8 = u32_to_u8(range_vk.vk.hash_u32());
    let _range_vkey_commitment = B256::from(multi_block_vkey_u8);
    // let _agg_vkey_hash = B256::from_str(&agg_vk.bytes32()).unwrap();
    let fetcher = match OPSuccinctDataFetcher::new_with_rollup_config(RunContext::Docker).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create data fetcher: {}", e);
            todo!();
        }
    };

    let host_cli = match fetcher
        .get_host_cli_args(
            payload.start,
            payload.end,
            ProgramType::Multi,
            CacheMode::DeleteCache,
        )
        .await
    {
        Ok(cli) => cli,
        Err(e) => {
            error!("Failed to get host CLI args: {}", e);
            return Err(anyhow::anyhow!("Failed to get host CLI args: {}", e));
        }
    };

    // Start the server and native client with a timeout.
    // Note: Ideally, the server should call out to a separate process that executes the native
    // host, and return an ID that the client can poll on to check if the proof was submitted.
    let mut witnessgen_executor = WitnessGenExecutor::new(WITNESSGEN_TIMEOUT, RunContext::Docker);
    if let Err(e) = witnessgen_executor.spawn_witnessgen(&host_cli).await {
        error!("Failed to spawn witness generation: {}", e);
        return Err(anyhow::anyhow!("Failed to spawn witness generation: {}", e));
    }
    // Log any errors from running the witness generation process.
    if let Err(e) = witnessgen_executor.flush().await {
        error!("Failed to generate witness: {}", e);
        return Err(anyhow::anyhow!("Failed to generate witness: {}", e));
    }

    let sp1_stdin = match get_proof_stdin(&host_cli) {
        Ok(stdin) => stdin,
        Err(e) => {
            error!("Failed to get proof stdin: {}", e);
            return Err(anyhow::anyhow!("Failed to get proof stdin: {}", e));
        }
    };

    info!("executing span proof");

    let proof = prover
        .prove(&range_pk, &sp1_stdin)
        .compressed()
        .run()
        .unwrap();

    info!("done with span proof");

    // Create a proof directory for the chain ID if it doesn't exist.
    let proof_dir = "proofs/".to_string();
    if !std::path::Path::new(&proof_dir).exists() {
        fs::create_dir_all(&proof_dir).unwrap();
    }
    let proof_path = format!("{}/{}-{}.bin", proof_dir, l2_start_block, l2_end_block);

    // Save the proof to the proof directory corresponding to the chain ID.
    proof.save(&proof_path).expect("saving proof failed");

    info!("saved proof to {}", proof_path);
    drop(prover);
    info!("sleeping for 10 seconds");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    info!("good morning");
    let proof_path = "proofs/12500-12501.bin".to_string();

    let (proofs, boot_infos) = load_aggregation_proof_data(proof_path);

    info!("loaded saved proof");

    let prover = ProverClient::from_env();
    let (_, vkey) = prover.setup(RANGE_ELF);

    let header = fetcher.get_latest_l1_head_in_batch(&boot_infos).await?;
    let headers = fetcher
        .get_header_preimages(&boot_infos, header.hash_slow())
        .await?;
    // let multi_block_vkey_u8 = u32_to_u8(vkey.vk.hash_u32());
    // let multi_block_vkey_b256 = B256::from(multi_block_vkey_u8);

    // println!(
    //     "Range ELF Verification Key Commitment: {}",
    //     multi_block_vkey_b256
    // );
    let stdin =
        get_agg_proof_stdin(proofs, boot_infos, headers, &vkey, header.hash_slow()).unwrap();

    let (agg_pk, _) = prover.setup(AGG_ELF);
    // println!("Aggregate ELF Verification Key: {:?}", agg_vk.vk.bytes32());
    //
    //
    info!("executing agg proof");
    let _proof_res = prover
        .prove(&agg_pk, &stdin)
        .groth16()
        .run()
        .expect("proving failed");

    info!("done with agg proof");

    Ok(())
}

/// Load the aggregation proof data.
fn load_aggregation_proof_data(proof_path: String) -> (Vec<SP1Proof>, Vec<BootInfoStruct>) {
    if fs::metadata(&proof_path).is_err() {
        panic!("Proof file not found: {}", proof_path);
    }

    let mut deserialized_proof =
        SP1ProofWithPublicValues::load(proof_path).expect("loading proof failed");

    // The public values are the BootInfoStruct.
    let boot_info = deserialized_proof.public_values.read();

    (vec![deserialized_proof.proof], vec![boot_info])
}
