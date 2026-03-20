//! Merkle tree indexer — fetches LeafInserted events from the DePLOB contract
//! and rebuilds the Incremental Merkle Tree locally. Used by withdraw and order
//! commands to generate Merkle proofs.

use std::collections::HashMap;

use alloy::{primitives::Address, providers::ProviderBuilder, sol};
use anyhow::{Context, Result};
use deplob_core::merkle::IncrementalMerkleTree;

// Only need the LeafInserted event from MerkleTreeWithHistory
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IMerkleTree {
        event LeafInserted(bytes32 indexed leaf, uint256 indexed leafIndex, bytes32 newRoot);
    }
}

/// Merkle tree rebuilt from on-chain events.
pub struct MerkleIndexer {
    tree: IncrementalMerkleTree,
    /// commitment → leaf_index
    leaf_map: HashMap<[u8; 32], u32>,
}

impl MerkleIndexer {
    /// Fetch all `LeafInserted` events from the DePLOB contract and rebuild
    /// the Merkle tree.
    pub async fn sync(rpc_url: &str, contract_addr: &str) -> Result<Self> {
        let url = rpc_url.parse().context("invalid RPC URL")?;
        let provider = ProviderBuilder::new().connect_http(url);
        let address: Address = contract_addr.parse().context("invalid contract address")?;

        let contract = IMerkleTree::IMerkleTreeInstance::new(address, &provider);

        // Fetch all LeafInserted events from block 0 to latest
        let events = contract
            .LeafInserted_filter()
            .from_block(0)
            .query()
            .await
            .context("failed to fetch LeafInserted events")?;

        // Sort by leafIndex to ensure correct insertion order
        let mut indexed: Vec<(u64, [u8; 32])> = events
            .iter()
            .map(|(event, _log)| {
                let idx: u64 = event.leafIndex.try_into().expect("leafIndex too large");
                (idx, event.leaf.0)
            })
            .collect();
        indexed.sort_by_key(|(idx, _)| *idx);

        // Rebuild tree
        let mut tree = IncrementalMerkleTree::new();
        let mut leaf_map = HashMap::new();

        for (expected_idx, leaf) in &indexed {
            let actual_idx = tree.insert(*leaf);
            assert_eq!(
                actual_idx as u64, *expected_idx,
                "leaf index mismatch: expected {expected_idx}, got {actual_idx}"
            );
            leaf_map.insert(*leaf, actual_idx);
        }

        tracing::info!("Merkle indexer synced: {} leaves", tree.len());

        Ok(Self { tree, leaf_map })
    }

    /// Generate a Merkle proof for a commitment.
    ///
    /// Returns (siblings, path_indices, root).
    pub fn proof_for(&self, commitment: &[u8; 32]) -> Result<(Vec<[u8; 32]>, Vec<u8>, [u8; 32])> {
        let &leaf_index = self
            .leaf_map
            .get(commitment)
            .context("commitment not found in Merkle tree — was it deposited?")?;

        let proof = self.tree.proof(leaf_index);
        let root = self.tree.get_root();

        Ok((proof.siblings.to_vec(), proof.path_indices.to_vec(), root))
    }

    /// Get the leaf index for a commitment, if it exists.
    pub fn _leaf_index(&self, commitment: &[u8; 32]) -> Option<u32> {
        self.leaf_map.get(commitment).copied()
    }

    /// Get the current Merkle root.
    pub fn _root(&self) -> [u8; 32] {
        self.tree.get_root()
    }
}
