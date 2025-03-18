use crate::{GenericProofRequest, ProofStatus, WorkerAggProofRequest, WorkerSpanProofRequest};
use alloy_primitives::B256;
use anyhow::{Context, Result};
use log::{debug, error, info};
use reqwest::Client;
use std::{collections::HashMap, fmt::Display};
use tokio::sync::{mpsc, oneshot};

// Worker strikes start at 0 and increment by 1 on every failed request.  When a worker's strikes
// are >= this value, the worker is removed from the registry
const MAX_WORKER_STRIKES: u16 = 3;

#[derive(Clone)]
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

    pub async fn worker_ready(&self, worker_addr: String) -> Result<()> {
        self.sender.send(WorkerRegistryCommand::WorkerReady { worker_addr}).await.map_err(|e| anyhow::anyhow!("Failed to send command WorkerReady: {}", e))
    }

    pub async fn assign_proof_request(&self, proof_id: B256, proof_request: GenericProofRequest) -> Result<()>{
        self.sender.send(WorkerRegistryCommand::AssignProofRequest { proof_id, proof_request }).await.map_err(|e| anyhow::anyhow!("Failed to send command AssignProofRequest: {}", e))
    }
}

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

// TODO: extract into `handle_xyz()` functions
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
                } => {
                    // TODO: should we do this here?
                    self.trim_workers();
                    info!("{} workers found", self.workers.len());

                    // first check if there's already a worker working on this proof
                    if let Some((worker_addr, _))  = self.workers.iter().find(|(_, worker_state)| {
                        if let WorkerStatus::Busy { proof_id: workers_proof_id } = worker_state.status {
                            workers_proof_id == proof_id
                        } else {
                            false
                        }
                    }) {
                        info!("Received proof request for proof {} but worker {} is already busy with it", proof_id, worker_addr);
                        // there's already a worker proving this.  We can return
                        // early
                        return;
                    }

                    // iterate over all idle workers
                    for (worker_addr, worker_state) in self.workers.iter_mut() {
                        debug!("Worker {} state {}", worker_addr, worker_state);

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

                                    worker_state.assigned_proof(proof_id);
                                    return;
                                } else {
                                    // TODO: could make a StrikeWorker command then make handling
                                    // reqwest responses more async by moving them to a tokio task
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
                                // TODO: could make a StrikeWorker command then make handling
                                // reqwest responses more async by moving them to a tokio task
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
                        Some(old_state) => {
                            // if this worker was working on a proof but we didn't drop it
                            if old_state.is_busy() && !old_state.should_drop() {
                                // TODO: what should happen to the proof it was working on?
                                error!("Worker {} re-started but wasn't dropped yet.  Worker State: {}", worker_addr, old_state);
                                todo!()
                            } else {
                                info!(
                                    "Known worker {} re-started, resetting state from {} to {}",
                                    worker_addr, old_state, default_state
                                );
                            }
                        }
                        None => {
                            info!("New worker {} added to registry", worker_addr);
                        }
                    }
                }
                WorkerRegistryCommand::ProofComplete { worker_addr } => {
                    debug!("Worker {} completed a proof and is now Idle.", worker_addr);
                    // move worker from "busy" to "idle"
                    if let Some(worker_state) = self.workers.get_mut(&worker_addr) {
                        worker_state.status = WorkerStatus::Idle;
                    }
                    // TODO: should we do this logic inside ProofStatus when a completed proof is
                    // returned?
                }
                WorkerRegistryCommand::ProofStatus {
                    target_proof_id,
                    resp_sender,
                } => {
                    // get worker assigned to this proof, forward proof_status request to them
                    match self
                        .workers
                        .iter_mut()
                        .find(|(_, worker_state)| match worker_state.status {
                            WorkerStatus::Idle => false,
                            WorkerStatus::Busy { proof_id } => proof_id == target_proof_id,
                        }) {
                        Some((worker_addr, worker_state)) => {
                            let worker_response = self
                                .reqwest_client
                                .get(format!("{}/status/{}", worker_addr, target_proof_id))
                                .send()
                                .await;

                            match worker_response {
                                Ok(response) => {
                                    // match so we can get the error code
                                    if response.status().is_success() {
                                        let proof_status: ProofStatus =
                                            match response.json().await {
                                            Ok(proof_status) => proof_status,
                                            Err(err) => {
                                                worker_state.add_strike();
                                                error!("Error deserializing response from {}/status{}.Error: {}", worker_addr, target_proof_id, err);
                                                // TODO: return lost proof status or re-try this
                                                // request??
                                                return;
                                            }
                                        };
                                        debug!(
                                            "ProofStatus of {} from worker {}: {}",
                                            target_proof_id, worker_addr, proof_status
                                        );

                                        // TODO: handle result
                                        resp_sender.send(proof_status);
                                    } else {
                                        // TODO: could make a StrikeWorker command then make handling
                                        // reqwest responses more async by moving them to a tokio task
                                        worker_state.add_strike();
                                        error!(
                                            "Failed to get response from {}/status/{}. Status code {}: {:?}",
                                            worker_addr, 
                                            target_proof_id,
                                            response.status().as_u16(),
                                            response.status().canonical_reason()
                                        );

                                        // TODO: return lost proof status or re-try this
                                        // request??
                                    }
                                }
                                Err(err) => {
                                    // TODO: could make a StrikeWorker command then make handling
                                    // reqwest responses more async by moving them to a tokio task
                                    worker_state.add_strike();
                                        error!(
                                        "Failed to send request {}/status/{}. Error: {}",
                                        worker_addr, target_proof_id,  err
                                    );

                                    // TODO: return lost proof status or re-try this
                                    // request??
                                }
                            }
                        }
                        None => {
                            error!("No worker is working on proof {}", target_proof_id);
                            // TODO: return lost proof status or re-try this
                            // TODO: missing proof resolution
                        }
                    }
                }
            }
        }
    }

    // TODO: where should we call this?
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

                    // TODO: If we're calling this inside `assign_proof` handler, then we need to
                    // make sure it assigns this re-started proof before it assigns the `new`
                    // proof.

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
    },
    ProofStatus {
        target_proof_id: B256,
        // returns the proof_status to the calling thread
        resp_sender: oneshot::Sender<ProofStatus>,
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

    fn assigned_proof(&mut self, proof_id: B256) {
        self.status = WorkerStatus::Busy { proof_id };
        // This worker has been good.  Reset their strikes
        self.strikes = 0;
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
