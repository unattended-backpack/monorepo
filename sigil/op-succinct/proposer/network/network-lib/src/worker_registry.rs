use crate::{GenericProofRequest, WorkerAggProofRequest, WorkerSpanProofRequest};
use alloy_primitives::B256;
use anyhow::Result;
use log::{error, info};
use reqwest::Client;
use std::collections::HashMap;

pub struct WorkerRegistry {
    // Using a HashMap is a fine complexity tradeoff because we'll never have >20 workers
    pub workers: HashMap<String, WorkerStatus>,
    pub reqwest_client: Client,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        let workers = HashMap::new();
        let reqwest_client = Client::new();
        Self {
            workers,
            reqwest_client,
        }
    }

    pub fn register_worker(&mut self, addr: &str) {
        self.workers.insert(addr.into(), WorkerStatus::Idle);
    }

    // Return Some(worker_id) on successful proof assignment.
    // otherwise, return None
    pub async fn assign_proof(
        &mut self,
        proof_id: &B256,
        proof_request: GenericProofRequest,
    ) -> String {
        let mut assigned_worker = None;
        // take first idle worker
        // TODO: this could end in deadlock but the proposer currently only requests a new proof
        // when it gets a response back from another
        while assigned_worker.is_none() { 
        for (worker_addr, _) in self
            .workers
            .iter()
            .filter(|(_, status)| **status == WorkerStatus::Idle).collect() {

                let worker_response = match proof_request {
                    GenericProofRequest::Agg(agg_proof_request) => {
                        let worker_agg_proof_request = WorkerAggProofRequest {
                            proof_id: *proof_id,
                            subproofs: agg_proof_request.subproofs.clone(),
                            head: agg_proof_request.head,
                        };
                        self.reqwest_client
                            .post(format!("{}/request_agg_proof", idle_worker_addr))
                            .json(&worker_agg_proof_request)
                            .send()
                            .await
                    }
                    GenericProofRequest::Span(span_proof_request) => {
                        let worker_span_proof_request = WorkerSpanProofRequest {
                            proof_id: *proof_id,
                            start: span_proof_request.start,
                            end: span_proof_request.end,
                        };
                        self.reqwest_client
                            .post(format!("{}/request_span_proof", idle_worker_addr))
                            .json(&worker_span_proof_request)
                            .send()
                            .await
                    }

                match worker_response {
                    Ok(response) => {
                        if response.status().is_success() {
                            info!(
                                "Successfully assigned proof {} to worker {}",
                                proof_id, idle_worker_addr
                            );
                        } else {
                            error!(
                                "Failed to assign proof {} to worker {}",
                                proof_id, idle_worker_addr
                            );
                        }
                    }
                    Err(err) => {
                        // TODO: 3 strikes, remove the worker
                        error!(
                            "Failed to send request for proof {} to worker {}",
                            proof_id, idle_worker_addr
                        );
                    }
                }

                Some(idle_worker_addr.clone())
            }
            None => None,
        }
    }
    // loops until this is true
    assigned_worker.unwrap()
    }
}

#[derive(Eq, PartialEq)]
pub enum WorkerStatus {
    Idle,
    Busy { proof_id: B256 },
}
