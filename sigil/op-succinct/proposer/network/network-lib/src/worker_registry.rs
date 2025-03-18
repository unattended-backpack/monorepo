use crate::{GenericProofRequest, WorkerAggProofRequest, WorkerSpanProofRequest};
use alloy_primitives::B256;
use anyhow::{Context, Result};
use log::{debug, error, info};
use reqwest::Client;
use std::{collections::HashMap, fmt::Display};
use tokio::sync::{mpsc, oneshot};

// Worker strikes start at 0 and increment by 1 on every failed request.  When a worker's strikes
// are >= this value, the worker is removed from the registry
const MAX_WORKER_STRIKES: u16 = 3;

pub struct WorkerRegistryClient {
    pub sender: mpsc::Sender<WorkerRegistryCommand>,
}

impl Default for WorkerRegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerRegistryClient {
    pub fn new() -> Self {
        let workers = HashMap::new();
        let reqwest_client = Client::new();

        let (sender, receiver) = mpsc::channel(100);

        let worker_registry = WorkerRegistry {
            workers,
            reqwest_client,
            receiver,
            self_command_sender: sender.clone(),
        };

        tokio::task::spawn(async move { worker_registry.background_event_loop().await });

        Self { sender }
    }
}
// pub fn register_worker(&mut self, addr: &str) {
//     self.workers.insert(addr.into(), WorkerStatus::Idle);
// }

/*
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
*/

pub struct WorkerRegistry {
    // Using a HashMap is a fine complexity tradeoff because we'll never have >20 workers, so
    // iterating isn't horrible in reality.
    pub workers: HashMap<String, WorkerState>,
    pub reqwest_client: Client,
    pub receiver: mpsc::Receiver<WorkerRegistryCommand>,
    // for sending the task to the back of the channel queue to allow for other events to
    // process
    pub self_command_sender: mpsc::Sender<WorkerRegistryCommand>,
}

impl WorkerRegistry {
    // TODO: to get around deadlock when waiting for a worker to become available, could I simply
    // send an event to the channel from inside the channel?
    async fn background_event_loop(mut self) {
        while let Some(command) = self.receiver.recv().await {
            debug!(
                "{} messages in worker registry channel",
                self.receiver.len()
            );
            match command {
                WorkerRegistryCommand::AssignProofRequest {
                    proof_id,
                    ref proof_request,
                    ref resp_sender,
                } => {
                    // TODO: should we do this here?
                    self.trim_workers();
                    info!("{} workers found", self.workers.len());

                    // iterate over all idle workers
                    for (worker_addr, worker_state) in self.workers.iter_mut() {
                        // if this worker isn't idle, skip
                        if worker_state.is_busy() {
                            continue;
                        }

                        let worker_response = match &proof_request {
                            GenericProofRequest::Agg(agg_proof_request) => {
                                let worker_agg_proof_request = WorkerAggProofRequest {
                                    proof_id,
                                    // TODO: this is an expensive clone
                                    subproofs: agg_proof_request.subproofs.clone(),
                                    head: agg_proof_request.head.clone(),
                                };
                                self.reqwest_client
                                    .post(format!("{}/request_agg_proof", worker_addr))
                                    .json(&worker_agg_proof_request)
                                    .send()
                                    .await
                            }
                            GenericProofRequest::Span(span_proof_request) => {
                                let worker_span_proof_request = WorkerSpanProofRequest {
                                    proof_id,
                                    start: span_proof_request.start,
                                    end: span_proof_request.end,
                                };
                                self.reqwest_client
                                    .post(format!("{}/request_span_proof", worker_addr))
                                    .json(&worker_span_proof_request)
                                    .send()
                                    .await
                            }
                        };

                        match worker_response {
                            Ok(response) => {
                                // match so we can get the error code
                                if response.status().is_success() {
                                    info!(
                                        "Successfully assigned proof {} to worker {}",
                                        proof_id, worker_addr
                                    );

                                    // TODO: handle result
                                    resp_sender.send(worker_addr.clone()).await;
                                    return;
                                } else {
                                    // TODO: 3 strikes, remove the worker
                                    worker_state.add_strike();
                                    error!(
                                        "Failed to assign proof {} to worker {}. Status code {}: {:?}",
                                        proof_id,
                                        worker_addr,
                                        response.status().as_u16(),
                                        response.status().canonical_reason()
                                    );
                                }
                            }
                            Err(err) => {
                                // TODO: 3 strikes, remove the worker
                                worker_state.add_strike();
                                error!(
                                    "Failed to send request for proof {} to worker {}. Error: {}",
                                    proof_id, worker_addr, err
                                );
                            }
                        }
                    }
                    // We iterated through all the workers and couldn't find an idle one who could
                    // receive the request.

                    // Push the AssignProofRequest event to the end of the channel queue so we have
                    // a chance to process other events we received in the meantime (like freeing
                    // up a worker).
                    // TODO: handle result
                    self.self_command_sender.send(command).await;
                }
                WorkerRegistryCommand::WorkerReady { worker_addr } => {
                    let default_state = WorkerState::default();
                    match self
                        .workers
                        .insert(worker_addr.clone(), default_state.clone())
                    {
                        Some(old_status) => {
                            info!(
                                "Known worker {} re-started, resetting state from {} to {}",
                                worker_addr, old_status, default_state
                            );
                        }
                        None => {
                            info!("New worker {} added to registry", worker_addr);
                        }
                    }
                }
                WorkerRegistryCommand::ProofComplete { worker_addr } => {
                    // move worker from "busy" to "idle"
                    if let Some(worker_state) = self.workers.get_mut(&worker_addr) {
                        worker_state.status = WorkerStatus::Idle;
                    }
                }
            }
        }
    }

    // iterate through workers and remove any who have > MAX_STRIKES strikes
    fn trim_workers(&mut self) {
        let dead_workers: Vec<String> = self
            .workers
            .iter_mut()
            .filter_map(|(worker_addr, worker_state)| {
                if worker_state.should_drop() {
                    Some(worker_addr.clone())
                } else {
                    None
                }
            })
            .collect();

        for dead_worker_addr in dead_workers {
            // remove them from the mapping & re-request any proofs they were working on
            if let Some(dead_worker_state) = self.workers.remove(&dead_worker_addr) {
                if let Some(dangling_proof) = dead_worker_state.current_proof_id() {
                    // TODO: re-request the proof somehow.  Depends on if we want this to bubble up
                    // to coordinator or not.  Probably should to check if we already have the
                    // proof locally

                    // Might need to send the proof_id to proof_store thread
                    todo!()
                }
            }
        }
    }
}

pub enum WorkerRegistryCommand {
    AssignProofRequest {
        proof_id: B256,
        proof_request: GenericProofRequest,
        // returns the address of the assigned worker
        resp_sender: mpsc::Sender<String>,
    },
    WorkerReady {
        worker_addr: String,
    },
    ProofComplete {
        worker_addr: String,
    },
}

#[derive(Eq, PartialEq, Clone)]
pub struct WorkerState {
    status: WorkerStatus,
    strikes: u16,
}

impl Default for WorkerState {
    fn default() -> Self {
        Self {
            status: WorkerStatus::Idle,
            strikes: 0,
        }
    }
}

impl WorkerState {
    fn is_busy(&self) -> bool {
        self.status != WorkerStatus::Idle
    }

    fn add_strike(&mut self) {
        self.strikes += 1;
        debug!("Strike added to worker.  New strikes: {}", self.strikes);
    }

    fn should_drop(&self) -> bool {
        self.strikes >= MAX_WORKER_STRIKES
    }

    // returns the proof it's currently working on, if any
    fn current_proof_id(&self) -> Option<B256> {
        match self.status {
            WorkerStatus::Idle => None,
            WorkerStatus::Busy { proof_id } => Some(proof_id),
        }
    }
}

impl Display for WorkerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Worker status: {}, Worker strikes: {}",
            self.status, self.strikes
        )
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum WorkerStatus {
    Idle,
    Busy { proof_id: B256 },
}

impl Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Busy { proof_id } => write!(f, "Busy with proof {proof_id}"),
        }
    }
}
