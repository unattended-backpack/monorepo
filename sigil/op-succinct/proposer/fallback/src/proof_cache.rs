use crate::SpanProofRequest;
use anyhow::{Context, Result};
use log::info;

use alloy_primitives::B256;
use std::{collections::HashMap, fs, path::Path};

const PROOF_CACHE_DIR: &str = "proofs";

// If the proving server goes offline and it has completed some span proofs, when it comes back up
// it will have to re-run those exact same spans.  Spans can take hours so we want some degree of
// persistence for when the server binary is stopped & started.  At the very least for easier
// debugging.

// This cache writes span proofs to disk, using a LRU cache (more appropriately, Least Recently
// Completed Proof).
pub struct ProofCache {
    // max cache size.  Setting it to 0 disables the cache
    cache_size: usize,
    // goes up to cache_size and loops.  Keeps track of the next proof to replace
    // increments each time we write a proof to disk
    current_cache_index: usize,
    // when we get a new proof, replace the proof at current_index and also look it up in
    // proof_requests and evict the address that we just replaced
    // (proof_id, proof_file_path_name)
    cache_list: Vec<(B256, String)>,
    proof_request_lookup: HashMap<B256, SpanProofRequest>,
}

impl ProofCache {
    pub fn new(cache_size: usize) -> Result<Self> {
        // Create `proofs/` directory if it doesn't already exist
        let path = Path::new(PROOF_CACHE_DIR);
        if !path.exists() {
            info!("`proofs/` directory doesn't exist.  Creating it.");
            fs::create_dir(path).context("Create proofs/ directory")?;
        } else {
            info!("Found `proofs/` directory.");
        }

        let current_cache_index = 0;
        let mut cache_list = Vec::with_capacity(cache_size);
        // fill with default values so we never have to check if current_cache_index is out of
        // bounds
        cache_list.resize_with(cache_size, || (B256::default(), "empty".into()));

        let proof_request_lookup = HashMap::new();

        Ok(Self {
            cache_size,
            current_cache_index,
            cache_list,
            proof_request_lookup,
        })
    }

    // This is needed so in `write_proof()` we can get the proof file name (proof request start & end
    // block) from the proof_id, which is the only argument we get sent in `/get_proof_status`
    pub fn record_proof_request(&mut self, proof_id: &B256, proof_request: &SpanProofRequest) {
        self.proof_request_lookup.insert(*proof_id, *proof_request);
    }

    pub fn lookup_proof_request(&self, proof_id: &B256) -> Option<&SpanProofRequest> {
        self.proof_request_lookup.get(proof_id)
    }

    // If we have a proof locally we can save hours of time by skipping span proof generation.
    // This just returns true if it does indeed exist locally.
    // Careful: Just because it exists doesn't mean it is known by proof_request_lookup.  This is
    // guarded against by returning a proof_id to the proposer and subsequently calling record_proof_request
    // even when we already have the proof locally
    // Called in `request_span_proof`
    pub fn proof_exists(&self, proof_request: &SpanProofRequest) -> bool {
        // if cache is disabled
        if self.cache_size == 0 {
            return false;
        }

        let proof_path_name = proof_request_to_file_path(proof_request);
        let proof_path = Path::new(&proof_path_name);

        proof_path.exists()
    }

    // Retreive a proof that we previously computed.  This can save us hours of proving time.
    // Safe to call even if we're not sure we have a proof.
    // called in get_proof_status()
    pub fn read_proof(&self, proof_id: &B256) -> Result<Option<Vec<u8>>> {
        // if cache is disabled
        if self.cache_size == 0 {
            return Ok(None);
        }

        match self.proof_request_lookup.get(proof_id) {
            Some(proof_request) => {
                let proof_path_name = proof_request_to_file_path(proof_request);
                let proof_path = Path::new(&proof_path_name);
                if proof_path.exists() {
                    info!(
                        "Found cached span proof of request {}, loading from file {}",
                        proof_request, proof_path_name
                    );

                    // load proof from file and return
                    let proof_bytes = fs::read(proof_path)
                        .context(format!("Reading proof from file {}", proof_path_name))?;
                    Ok(Some(proof_bytes))
                } else {
                    // This means the proof is requested & the cache is aware of it (its in proof_request_lookup) but it hasn't
                    // completed yet (didn't get written to disk in write_proof())
                    Ok(None)
                }
            }
            // we haven't received this proof request yet, it's not in the proof_request_lookup
            None => Ok(None),
        }
    }

    // writes the completed proof to file, deleting the Least Recently Completed proof that the
    // cache is aware of.
    // Takes a mutable reference, so only use this if you're sure the proof isn't already on disk
    pub fn write_proof(&mut self, proof_bytes: Vec<u8>, proof_id: &B256) -> Result<()> {
        // if cache is disabled
        if self.cache_size == 0 {
            return Ok(());
        }

        info!(
            "num proof bytes in write proof in cache: {}",
            proof_bytes.len()
        );

        // overwrite the current_cache_index of cache_list
        let elem = self
            .cache_list
            .get_mut(self.current_cache_index)
            .context(format!(
                "Index {} out of bounds of cache_list vector",
                self.current_cache_index,
            ))?;

        // if a proof exists here, evict it by deleting the file
        let old_proof_path = Path::new(&elem.1);
        if old_proof_path.exists() {
            fs::remove_file(old_proof_path)
                .context(format!("Delete old proof file {:?}", old_proof_path))?;
        }

        // get proof request parameters (needed for proof file name) from the proof_id
        let proof_request = self.proof_request_lookup.get(proof_id).context(format!(
            "span proof id {} not found in request hashmap",
            proof_id
        ))?;
        // get proof file name so we can write
        let proof_path_name = proof_request_to_file_path(proof_request);
        let path = Path::new(&proof_path_name);

        // write completed proof to file
        fs::write(path, proof_bytes).context(format!(
            "Write proof id {} to file {}",
            proof_id, proof_path_name
        ))?;

        // overwrite the current_cache_index in the cache_list vector with our newly written proof
        let new_elem = (*proof_id, proof_path_name);
        *elem = new_elem;

        self.increment_current_cache_index();
        Ok(())
    }

    fn increment_current_cache_index(&mut self) {
        if self.cache_size > 0 {
            // increment by 1, looping to the start if its at capacity (cache_size)
            let new_cache_index = (self.current_cache_index + 1) % self.cache_size;
            self.current_cache_index = new_cache_index;
        }
    }
}

fn proof_request_to_file_path(proof_request: &SpanProofRequest) -> String {
    format!(
        "{}/{}-{}",
        PROOF_CACHE_DIR, proof_request.start, proof_request.end
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructor() {
        let proof_cache = ProofCache::new(0).unwrap();
        assert_eq!(proof_cache.cache_list.len(), 0);

        let proof_cache = ProofCache::new(10).unwrap();
        assert_eq!(proof_cache.cache_list.len(), 10);
    }

    #[test]
    fn test_proof_request_to_file_path() {
        let proof_request = SpanProofRequest {
            start: 255,
            end: 256,
        };

        let proof_file_path_name = proof_request_to_file_path(&proof_request);
        let correct_proof_file_path_name = format!("{PROOF_CACHE_DIR}/255-256");

        assert_eq!(proof_file_path_name, correct_proof_file_path_name);
    }

    #[test]
    fn test_increment_current_cache_index_0() {
        let mut proof_cache = ProofCache::new(0).unwrap();
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
    }

    #[test]
    fn test_increment_current_cache_index_1() {
        let mut proof_cache = ProofCache::new(1).unwrap();
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
    }

    #[test]
    fn test_increment_current_cache_index_2() {
        let mut proof_cache = ProofCache::new(2).unwrap();
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 1);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 1);
    }

    #[test]
    fn test_increment_current_cache_index_3() {
        let mut proof_cache = ProofCache::new(3).unwrap();
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 1);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 2);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
    }
}
