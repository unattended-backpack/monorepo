use crate::GenericProofRequest;
use anyhow::Result;

use super::ProofType;
use alloy_primitives::B256;
use std::{collections::HashMap, thread::current};

pub struct ProofCache {
    // max cache size.  Setting it to 0 disables the cache
    cache_size: usize,
    // index of the next proof to insert
    // goes up to cache_size and loops.  Keeps track of the next proof to replace
    current_cache_index: usize,
    // when we get a new proof, replace the proof at current_index and also look it up in
    // proof_requests and evict the address that we just replaced
    proof_ids_in_cache: Vec<B256>,
}

impl ProofCache {
    pub fn new(cache_size: usize) -> Self {
        let current_cache_index = 0;
        let proof_ids_in_cache = Vec::with_capacity(cache_size);
        Self {
            cache_size,
            current_cache_index,
            proof_ids_in_cache,
        }
    }

    pub fn get_proof(&self, proof_id: &B256) -> Option<Vec<u8>> {
        // if cache is disabled
        if self.cache_size == 0 {
            return None;
        }

        // load file with the proof_id
        todo!()
    }

    pub fn proof_completed(&mut self, proof_bytes: Vec<u8>, proof_id: &B256) -> Result<()> {
        // if cache is disabled
        if self.cache_size == 0 {
            return Ok(());
        }

        let current_cache_index = self.current_cache_index;
        // if a proof exists at this index, kick it out (sorry!)
        if let Some(proof_to_evict) = self.proof_ids_in_cache.get(current_cache_index) {
            // delete file

            //
        }
        // can I just overwrite that index safely here?

        // write the new proof to file

        if let Some(elem) = self.proof_ids_in_cache.get_mut(current_cache_index) {
        } else {
            return Err(anyhow!(
                "Index {} out of bounds of cache vector which has length {}",
                current_cache_index,
                self.proof_ids_in_cache.len()
            ));
        }

        self.increment_current_cache_index();
        todo!()
    }

    fn increment_current_cache_index(&mut self) {
        if self.cache_size > 0 {
            // increment by 1, looping to the start if its at capacity (cache_size)
            let new_cache_index = (self.current_cache_index + 1) % self.cache_size;
            self.current_cache_index = new_cache_index;
        }
    }
}

struct ProofMetaData {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_current_cache_index_0() {
        let mut proof_cache = ProofCache::new(0);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
    }

    #[test]
    fn test_increment_current_cache_index_1() {
        let mut proof_cache = ProofCache::new(1);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
    }

    #[test]
    fn test_increment_current_cache_index_2() {
        let mut proof_cache = ProofCache::new(2);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 1);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 1);
    }

    #[test]
    fn test_increment_current_cache_index_3() {
        let mut proof_cache = ProofCache::new(3);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 1);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 2);
        proof_cache.increment_current_cache_index();
        assert_eq!(proof_cache.current_cache_index, 0);
    }
}
