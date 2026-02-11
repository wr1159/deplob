//! DePLOB Core Library
//!
//! Shared cryptographic primitives and types for all SP1 programs.
//!
//! # Modules
//!
//! - [`keccak`]: Keccak256 hash function (EVM compatible)
//! - [`commitment`]: Commitment scheme for hiding deposit details
//! - [`merkle`]: Incremental Merkle tree for on-chain verification
//!
//! # Example
//!
//! ```
//! use deplob_core::{CommitmentPreimage, IncrementalMerkleTree, Commitment};
//!
//! // Create a deposit commitment
//! let preimage = CommitmentPreimage::new(
//!     [1u8; 32],  // nullifier_note
//!     [2u8; 32],  // secret
//!     [0xAB; 20], // token address
//!     1_000_000_000_000_000_000, // 1 ETH
//! );
//!
//! let commitment = preimage.commitment();
//! let nullifier = preimage.nullifier();
//!
//! // Add to Merkle tree
//! let mut tree = IncrementalMerkleTree::new();
//! let index = tree.insert(commitment);
//!
//! // Generate proof
//! let proof = tree.proof(index);
//! assert!(proof.verify(&commitment, &tree.get_root()));
//! ```

pub mod commitment;
pub mod keccak;
pub mod merkle;

// Re-export commonly used types
pub use commitment::{Commitment, CommitmentPreimage, Nullifier};
pub use keccak::{
    address_to_bytes32, keccak256, keccak256_concat, keccak256_pair, u128_to_bytes32,
};
pub use merkle::{zero_hashes, IncrementalMerkleTree, MerkleProof, TREE_DEPTH, ZERO_VALUE};

/// Alias for keccak256_pair for convenience
pub fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    keccak256_pair(left, right)
}
