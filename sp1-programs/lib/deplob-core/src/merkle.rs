//! Incremental Merkle Tree implementation
//!
//! This implementation matches the Solidity contract for on-chain verification.
//! It uses keccak256 for hashing, same as Tornado Cash.
//!
//! Key properties:
//! - Fixed depth (20 levels = ~1M leaves)
//! - Efficient incremental insertions
//! - Compact Merkle proofs (O(log n) size)

use crate::keccak::keccak256_pair;
use serde::{Deserialize, Serialize};

/// Tree depth (supports 2^20 = 1,048,576 leaves)
pub const TREE_DEPTH: usize = 20;

/// Maximum number of leaves
pub const MAX_LEAVES: u32 = 1 << TREE_DEPTH; // 2^20

/// Zero value for empty leaves
pub const ZERO_VALUE: [u8; 32] = [0u8; 32];

/// Precomputed zero hashes for each level of the tree
///
/// zeros[0] = ZERO_VALUE
/// zeros[i] = keccak256(zeros[i-1] || zeros[i-1])
///
/// These are computed at compile time for efficiency.
pub fn zero_hashes() -> [[u8; 32]; TREE_DEPTH] {
    let mut zeros = [[0u8; 32]; TREE_DEPTH];
    zeros[0] = ZERO_VALUE;

    for i in 1..TREE_DEPTH {
        zeros[i] = keccak256_pair(&zeros[i - 1], &zeros[i - 1]);
    }

    zeros
}

/// Get zero hash for a specific level (cached computation)
pub fn zero_hash(level: usize) -> [u8; 32] {
    // For SP1 efficiency, we compute inline rather than storing
    if level == 0 {
        return ZERO_VALUE;
    }

    let mut current = ZERO_VALUE;
    for _ in 0..level {
        current = keccak256_pair(&current, &current);
    }
    current
}

/// Merkle proof for verifying leaf inclusion
///
/// A proof consists of sibling hashes along the path from leaf to root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Sibling hashes from leaf (level 0) to root (level TREE_DEPTH-1)
    pub siblings: [[u8; 32]; TREE_DEPTH],

    /// Path indices indicating if the node is a left (0) or right (1) child
    pub path_indices: [u8; TREE_DEPTH],
}

impl MerkleProof {
    /// Verify that a leaf is included in the tree with the given root
    ///
    /// # Arguments
    /// * `leaf` - The leaf hash to verify
    /// * `root` - The expected Merkle root
    ///
    /// # Returns
    /// True if the proof is valid
    pub fn verify(&self, leaf: &[u8; 32], root: &[u8; 32]) -> bool {
        let computed_root = self.compute_root(leaf);
        &computed_root == root
    }

    /// Compute the Merkle root from a leaf using this proof
    ///
    /// # Arguments
    /// * `leaf` - The leaf hash
    ///
    /// # Returns
    /// The computed Merkle root
    pub fn compute_root(&self, leaf: &[u8; 32]) -> [u8; 32] {
        let mut current = *leaf;

        for i in 0..TREE_DEPTH {
            if self.path_indices[i] == 0 {
                // Current node is left child
                current = keccak256_pair(&current, &self.siblings[i]);
            } else {
                // Current node is right child
                current = keccak256_pair(&self.siblings[i], &current);
            }
        }

        current
    }

    /// Create an empty proof (all zeros)
    pub fn empty() -> Self {
        MerkleProof {
            siblings: [[0u8; 32]; TREE_DEPTH],
            path_indices: [0u8; TREE_DEPTH],
        }
    }
}

/// Incremental Merkle Tree
///
/// This tree supports efficient append-only operations, matching the
/// on-chain implementation. It maintains cached subtree roots for
/// O(log n) insertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalMerkleTree {
    /// All inserted leaves (for generating proofs)
    pub leaves: Vec<[u8; 32]>,

    /// Index of the next leaf to be inserted
    pub next_index: u32,

    /// Cached filled subtree roots at each level
    /// filled_subtrees[i] is the root of the rightmost filled subtree at level i
    pub filled_subtrees: [[u8; 32]; TREE_DEPTH],

    /// Current Merkle root
    pub root: [u8; 32],
}

impl IncrementalMerkleTree {
    /// Create a new empty Merkle tree
    pub fn new() -> Self {
        let zeros = zero_hashes();

        // Compute initial root (all zeros)
        let mut root = zeros[TREE_DEPTH - 1];
        root = keccak256_pair(&root, &root);

        Self {
            leaves: Vec::new(),
            next_index: 0,
            filled_subtrees: zeros,
            root,
        }
    }

    /// Insert a new leaf into the tree
    ///
    /// # Arguments
    /// * `leaf` - The leaf hash to insert
    ///
    /// # Returns
    /// The index where the leaf was inserted
    ///
    /// # Panics
    /// If the tree is full (>= 2^TREE_DEPTH leaves)
    pub fn insert(&mut self, leaf: [u8; 32]) -> u32 {
        assert!(self.next_index < MAX_LEAVES, "Merkle tree is full");

        let index = self.next_index;
        self.leaves.push(leaf);

        let mut current_hash = leaf;
        let mut current_index = index;
        let zeros = zero_hashes();

        for i in 0..TREE_DEPTH {
            if current_index % 2 == 0 {
                // Left child: update filled subtree, pair with zero
                self.filled_subtrees[i] = current_hash;
                current_hash = keccak256_pair(&current_hash, &zeros[i]);
            } else {
                // Right child: pair with filled subtree
                current_hash = keccak256_pair(&self.filled_subtrees[i], &current_hash);
            }
            current_index /= 2;
        }

        self.root = current_hash;
        self.next_index += 1;

        index
    }

    /// Generate a Merkle proof for a leaf at the given index
    ///
    /// # Arguments
    /// * `index` - The leaf index
    ///
    /// # Returns
    /// A MerkleProof that can verify the leaf's inclusion
    ///
    /// # Panics
    /// If the index is out of bounds
    pub fn proof(&self, index: u32) -> MerkleProof {
        assert!((index as usize) < self.leaves.len(), "Index out of bounds");

        let mut siblings = [[0u8; 32]; TREE_DEPTH];
        let mut path_indices = [0u8; TREE_DEPTH];
        let mut current_index = index;
        let zeros = zero_hashes();

        // Build all nodes level by level for efficiency
        let mut current_level: Vec<[u8; 32]> = self.leaves.clone();

        for level in 0..TREE_DEPTH {
            path_indices[level] = (current_index % 2) as u8;

            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            // Get sibling from current level
            siblings[level] = if (sibling_index as usize) < current_level.len() {
                current_level[sibling_index as usize]
            } else {
                zeros[level]
            };

            // Compute next level
            let mut next_level = Vec::with_capacity((current_level.len() + 1) / 2);
            let mut i = 0;
            while i < current_level.len() {
                let left = current_level[i];
                let right = if i + 1 < current_level.len() {
                    current_level[i + 1]
                } else {
                    zeros[level]
                };
                next_level.push(keccak256_pair(&left, &right));
                i += 2;
            }
            current_level = next_level;
            current_index /= 2;
        }

        MerkleProof {
            siblings,
            path_indices,
        }
    }

    /// Get the current root
    pub fn get_root(&self) -> [u8; 32] {
        self.root
    }

    /// Get the number of leaves in the tree
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }
}

impl Default for IncrementalMerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_hashes_consistency() {
        let zeros1 = zero_hashes();
        let zeros2 = zero_hashes();

        // Should be consistent across calls
        assert_eq!(zeros1, zeros2);

        // Level 0 should be all zeros
        assert_eq!(zeros1[0], ZERO_VALUE);

        // Each level should be hash of previous level with itself
        for i in 1..TREE_DEPTH {
            let expected = keccak256_pair(&zeros1[i - 1], &zeros1[i - 1]);
            assert_eq!(zeros1[i], expected);
        }
    }

    #[test]
    fn test_empty_tree() {
        let tree = IncrementalMerkleTree::new();

        assert_eq!(tree.next_index, 0);
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn test_single_insert() {
        let mut tree = IncrementalMerkleTree::new();
        let leaf = [1u8; 32];

        let index = tree.insert(leaf);

        assert_eq!(index, 0);
        assert_eq!(tree.len(), 1);
        assert!(!tree.is_empty());
    }

    #[test]
    fn test_insert_changes_root() {
        let mut tree = IncrementalMerkleTree::new();

        let root_before = tree.get_root();
        tree.insert([1u8; 32]);
        let root_after = tree.get_root();

        assert_ne!(root_before, root_after);
    }

    #[test]
    fn test_proof_verification() {
        let mut tree = IncrementalMerkleTree::new();

        let leaf1 = [1u8; 32];
        let leaf2 = [2u8; 32];
        let leaf3 = [3u8; 32];

        tree.insert(leaf1);
        tree.insert(leaf2);
        tree.insert(leaf3);

        let root = tree.get_root();

        // Verify all proofs
        let proof0 = tree.proof(0);
        let proof1 = tree.proof(1);
        let proof2 = tree.proof(2);

        assert!(proof0.verify(&leaf1, &root), "Proof for leaf 0 failed");
        assert!(proof1.verify(&leaf2, &root), "Proof for leaf 1 failed");
        assert!(proof2.verify(&leaf3, &root), "Proof for leaf 2 failed");
    }

    #[test]
    fn test_proof_fails_for_wrong_leaf() {
        let mut tree = IncrementalMerkleTree::new();

        let leaf1 = [1u8; 32];
        let leaf2 = [2u8; 32];

        tree.insert(leaf1);

        let root = tree.get_root();
        let proof = tree.proof(0);

        // Should fail with wrong leaf
        assert!(!proof.verify(&leaf2, &root));
    }

    #[test]
    fn test_proof_fails_for_wrong_root() {
        let mut tree = IncrementalMerkleTree::new();

        let leaf = [1u8; 32];
        tree.insert(leaf);

        let proof = tree.proof(0);
        let wrong_root = [99u8; 32];

        // Should fail with wrong root
        assert!(!proof.verify(&leaf, &wrong_root));
    }

    #[test]
    fn test_multiple_inserts() {
        let mut tree = IncrementalMerkleTree::new();

        // Insert 10 leaves
        for i in 0u8..10 {
            let mut leaf = [0u8; 32];
            leaf[0] = i;
            let index = tree.insert(leaf);
            assert_eq!(index, i as u32);
        }

        assert_eq!(tree.len(), 10);

        // Verify all proofs
        let root = tree.get_root();
        for i in 0u8..10 {
            let mut leaf = [0u8; 32];
            leaf[0] = i;
            let proof = tree.proof(i as u32);
            assert!(proof.verify(&leaf, &root), "Proof for leaf {} failed", i);
        }
    }

    #[test]
    fn test_deterministic_root() {
        let mut tree1 = IncrementalMerkleTree::new();
        let mut tree2 = IncrementalMerkleTree::new();

        let leaves = [[1u8; 32], [2u8; 32], [3u8; 32]];

        for leaf in leaves.iter() {
            tree1.insert(*leaf);
            tree2.insert(*leaf);
        }

        // Same insertions should produce same root
        assert_eq!(tree1.get_root(), tree2.get_root());
    }
}
