//! Merkle tree indexer — fetches LeafInserted events from the DePLOB contract
//! and rebuilds the Incremental Merkle Tree locally. Used by withdraw and order
//! commands to generate Merkle proofs.

use std::collections::HashMap;

use alloy::{
    primitives::Address,
    providers::{Provider, ProviderBuilder},
    sol,
};
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

        // Fetch LeafInserted events in chunks to stay within RPC block range limits
        let latest_block = provider
            .get_block_number()
            .await
            .context("failed to get latest block number")?;

        const CHUNK_SIZE: u64 = 10_000;
        // On public RPCs, scanning from block 0 is too slow. Default to last 50k blocks
        // which covers any recent deployment. Set DEPLOY_BLOCK to override.
        let start_block = std::env::var("DEPLOY_BLOCK")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_else(|| latest_block.saturating_sub(50_000));

        let mut indexed: Vec<(u64, [u8; 32])> = Vec::new();
        let mut from = start_block;

        while from <= latest_block {
            let to = (from + CHUNK_SIZE - 1).min(latest_block);
            let events = contract
                .LeafInserted_filter()
                .from_block(from)
                .to_block(to)
                .query()
                .await
                .with_context(|| {
                    format!("failed to fetch LeafInserted events for blocks {from}..{to}")
                })?;

            for (event, _log) in &events {
                let idx: u64 = event.leafIndex.try_into().expect("leafIndex too large");
                indexed.push((idx, event.leaf.0));
            }

            from = to + 1;
        }
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
    /// Returns (siblings, path_indices, root, leaf_index).
    pub fn proof_for(
        &self,
        commitment: &[u8; 32],
    ) -> Result<(Vec<[u8; 32]>, Vec<u8>, [u8; 32], u32)> {
        let &leaf_index = self
            .leaf_map
            .get(commitment)
            .context("commitment not found in Merkle tree — was it deposited?")?;

        let proof = self.tree.proof(leaf_index);
        let root = self.tree.get_root();

        Ok((
            proof.siblings.to_vec(),
            proof.path_indices.to_vec(),
            root,
            leaf_index,
        ))
    }
}
