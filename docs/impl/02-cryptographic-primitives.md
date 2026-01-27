# Step 2: Cryptographic Primitives

## Overview

DePLOB requires these cryptographic primitives:

| Primitive | Purpose | Implementation |
|-----------|---------|----------------|
| Poseidon Hash | Commitments, nullifiers (ZK-friendly) | Rust crate |
| Keccak256 | Merkle tree (EVM compatible) | Rust + Solidity |
| AES-GCM | Order encryption for TEE | Rust crate |
| ECDH | Key exchange with TEE | Rust crate |

## 2.1 Poseidon Hash (ZK-Friendly)

Poseidon is optimized for ZK circuits with fewer constraints than Keccak256.

### Add Dependencies

Update `sp1-programs/lib/deplob-core/Cargo.toml`:

```toml
[package]
name = "deplob-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
# Poseidon hash implementation
poseidon-rs = "0.0.10"
ff = { version = "0.13", features = ["derive"] }
```

### Implement Poseidon in Rust

`sp1-programs/lib/deplob-core/src/poseidon.rs`:

```rust
//! Poseidon hash implementation for DePLOB
//!
//! Uses BN254 field for Ethereum compatibility

use poseidon_rs::{Fr, Poseidon};

/// Poseidon hash of two field elements
pub fn poseidon_hash2(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let poseidon = Poseidon::new();

    let left_fr = bytes_to_fr(left);
    let right_fr = bytes_to_fr(right);

    let hash = poseidon.hash(vec![left_fr, right_fr]).expect("Poseidon hash failed");

    fr_to_bytes(&hash)
}

/// Poseidon hash of variable inputs
pub fn poseidon_hash(inputs: &[[u8; 32]]) -> [u8; 32] {
    let poseidon = Poseidon::new();

    let fr_inputs: Vec<Fr> = inputs.iter().map(|x| bytes_to_fr(x)).collect();

    let hash = poseidon.hash(fr_inputs).expect("Poseidon hash failed");

    fr_to_bytes(&hash)
}

/// Convert bytes to field element
fn bytes_to_fr(bytes: &[u8; 32]) -> Fr {
    Fr::from_bytes(bytes).expect("Invalid field element")
}

/// Convert field element to bytes
fn fr_to_bytes(fr: &Fr) -> [u8; 32] {
    fr.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poseidon_hash2() {
        let left = [1u8; 32];
        let right = [2u8; 32];

        let hash1 = poseidon_hash2(&left, &right);
        let hash2 = poseidon_hash2(&left, &right);

        // Deterministic
        assert_eq!(hash1, hash2);

        // Different inputs = different output
        let hash3 = poseidon_hash2(&right, &left);
        assert_ne!(hash1, hash3);
    }
}
```

## 2.2 Keccak256 (EVM Compatible)

For Merkle tree to match Solidity's `keccak256`.

`sp1-programs/lib/deplob-core/src/keccak.rs`:

```rust
//! Keccak256 implementation matching Solidity

use tiny_keccak::{Hasher, Keccak};

/// Keccak256 hash matching Solidity's keccak256
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    let mut output = [0u8; 32];
    hasher.update(data);
    hasher.finalize(&mut output);
    output
}

/// Hash two 32-byte values (for Merkle tree)
pub fn keccak256_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(left);
    combined[32..].copy_from_slice(right);
    keccak256(&combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keccak256() {
        // Test vector: keccak256("hello")
        let input = b"hello";
        let hash = keccak256(input);

        // Known hash value
        let expected = hex::decode(
            "1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8"
        ).unwrap();

        assert_eq!(hash, expected.as_slice());
    }
}
```

Add `tiny-keccak` and `hex` to workspace dependencies.

## 2.3 Commitment Scheme

The core commitment structure for deposits and orders.

`sp1-programs/lib/deplob-core/src/commitment.rs`:

```rust
//! Commitment scheme for DePLOB
//!
//! commitment = Poseidon(nullifier_note, secret, token, amount)

use crate::poseidon::poseidon_hash;
use serde::{Deserialize, Serialize};

/// Raw commitment data (private)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentPreimage {
    /// Random value for nullifier derivation
    pub nullifier_note: [u8; 32],
    /// Random secret for hiding
    pub secret: [u8; 32],
    /// Token address (as bytes32, padded)
    pub token: [u8; 32],
    /// Amount (as bytes32)
    pub amount: [u8; 32],
}

impl CommitmentPreimage {
    /// Create new commitment preimage
    pub fn new(
        nullifier_note: [u8; 32],
        secret: [u8; 32],
        token: [u8; 20],  // Ethereum address
        amount: u128,
    ) -> Self {
        let mut token_padded = [0u8; 32];
        token_padded[12..].copy_from_slice(&token);

        let mut amount_bytes = [0u8; 32];
        amount_bytes[16..].copy_from_slice(&amount.to_be_bytes());

        Self {
            nullifier_note,
            secret,
            token: token_padded,
            amount: amount_bytes,
        }
    }

    /// Compute commitment hash
    pub fn commitment(&self) -> [u8; 32] {
        poseidon_hash(&[
            self.nullifier_note,
            self.secret,
            self.token,
            self.amount,
        ])
    }

    /// Compute nullifier for spending
    pub fn nullifier(&self) -> [u8; 32] {
        poseidon_hash(&[self.nullifier_note])
    }
}

/// Public commitment (stored on-chain)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Commitment(pub [u8; 32]);

impl From<&CommitmentPreimage> for Commitment {
    fn from(preimage: &CommitmentPreimage) -> Self {
        Commitment(preimage.commitment())
    }
}

/// Nullifier (reveals to prevent double-spend)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Nullifier(pub [u8; 32]);

impl From<&CommitmentPreimage> for Nullifier {
    fn from(preimage: &CommitmentPreimage) -> Self {
        Nullifier(preimage.nullifier())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment() {
        let preimage = CommitmentPreimage::new(
            [1u8; 32],  // nullifier_note
            [2u8; 32],  // secret
            [0xABu8; 20],  // token address
            1000000000000000000u128,  // 1 ETH in wei
        );

        let commitment = preimage.commitment();
        let nullifier = preimage.nullifier();

        // Commitment and nullifier should be different
        assert_ne!(commitment, nullifier);

        // Same preimage = same outputs
        let commitment2 = preimage.commitment();
        assert_eq!(commitment, commitment2);
    }
}
```

## 2.4 Merkle Tree

Incremental Merkle tree for commitment storage.

`sp1-programs/lib/deplob-core/src/merkle.rs`:

```rust
//! Incremental Merkle Tree implementation
//!
//! Matches the Solidity implementation for on-chain verification

use crate::keccak::keccak256_pair;
use serde::{Deserialize, Serialize};

/// Merkle tree depth (supports 2^20 = 1M leaves)
pub const TREE_DEPTH: usize = 20;

/// Zero value for empty leaves
pub const ZERO_VALUE: [u8; 32] = [0u8; 32];

/// Precomputed zero hashes for each level
pub fn zero_hashes() -> [[u8; 32]; TREE_DEPTH] {
    let mut zeros = [[0u8; 32]; TREE_DEPTH];
    zeros[0] = ZERO_VALUE;

    for i in 1..TREE_DEPTH {
        zeros[i] = keccak256_pair(&zeros[i - 1], &zeros[i - 1]);
    }

    zeros
}

/// Merkle proof for inclusion verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Sibling hashes from leaf to root
    pub siblings: [[u8; 32]; TREE_DEPTH],
    /// Path indices (0 = left, 1 = right)
    pub path_indices: [u8; TREE_DEPTH],
}

impl MerkleProof {
    /// Verify that a leaf is in the tree with given root
    pub fn verify(&self, leaf: &[u8; 32], root: &[u8; 32]) -> bool {
        let computed_root = self.compute_root(leaf);
        &computed_root == root
    }

    /// Compute root from leaf using proof
    pub fn compute_root(&self, leaf: &[u8; 32]) -> [u8; 32] {
        let mut current = *leaf;

        for i in 0..TREE_DEPTH {
            if self.path_indices[i] == 0 {
                // Current node is on the left
                current = keccak256_pair(&current, &self.siblings[i]);
            } else {
                // Current node is on the right
                current = keccak256_pair(&self.siblings[i], &current);
            }
        }

        current
    }
}

/// Incremental Merkle Tree state (for building proofs off-chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalMerkleTree {
    /// Current leaves (sparse representation)
    pub leaves: Vec<[u8; 32]>,
    /// Number of inserted leaves
    pub next_index: u32,
    /// Cached subtree roots for efficient updates
    pub filled_subtrees: [[u8; 32]; TREE_DEPTH],
    /// Current root
    pub root: [u8; 32],
}

impl IncrementalMerkleTree {
    /// Create new empty tree
    pub fn new() -> Self {
        let zeros = zero_hashes();

        // Initial root is hash of all zeros
        let mut root = zeros[TREE_DEPTH - 1];
        root = keccak256_pair(&root, &root);

        Self {
            leaves: Vec::new(),
            next_index: 0,
            filled_subtrees: zeros,
            root,
        }
    }

    /// Insert a new leaf
    pub fn insert(&mut self, leaf: [u8; 32]) -> u32 {
        let index = self.next_index;
        self.leaves.push(leaf);

        let mut current = leaf;
        let mut current_index = index;

        for i in 0..TREE_DEPTH {
            if current_index % 2 == 0 {
                // Left child: sibling is zero or previous subtree
                self.filled_subtrees[i] = current;
                current = keccak256_pair(&current, &zero_hashes()[i]);
            } else {
                // Right child: sibling is filled subtree
                current = keccak256_pair(&self.filled_subtrees[i], &current);
            }
            current_index /= 2;
        }

        self.root = current;
        self.next_index += 1;

        index
    }

    /// Generate Merkle proof for leaf at index
    pub fn proof(&self, index: u32) -> MerkleProof {
        let zeros = zero_hashes();
        let mut siblings = [[0u8; 32]; TREE_DEPTH];
        let mut path_indices = [0u8; TREE_DEPTH];
        let mut current_index = index;

        for i in 0..TREE_DEPTH {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            path_indices[i] = (current_index % 2) as u8;

            // Get sibling value
            let level_size = 1u32 << i;
            if sibling_index < self.next_index.min(level_size * 2) {
                // Sibling exists in tree
                siblings[i] = self.compute_node(i, sibling_index);
            } else {
                // Sibling is zero hash
                siblings[i] = zeros[i];
            }

            current_index /= 2;
        }

        MerkleProof {
            siblings,
            path_indices,
        }
    }

    /// Compute node at given level and index
    fn compute_node(&self, level: usize, index: u32) -> [u8; 32] {
        if level == 0 {
            return self.leaves.get(index as usize).copied().unwrap_or(ZERO_VALUE);
        }

        let left = self.compute_node(level - 1, index * 2);
        let right = self.compute_node(level - 1, index * 2 + 1);
        keccak256_pair(&left, &right)
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
    fn test_merkle_tree_insert_and_verify() {
        let mut tree = IncrementalMerkleTree::new();

        // Insert some leaves
        let leaf1 = [1u8; 32];
        let leaf2 = [2u8; 32];
        let leaf3 = [3u8; 32];

        let idx1 = tree.insert(leaf1);
        let idx2 = tree.insert(leaf2);
        let idx3 = tree.insert(leaf3);

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(idx3, 2);

        // Generate and verify proofs
        let proof1 = tree.proof(0);
        let proof2 = tree.proof(1);
        let proof3 = tree.proof(2);

        assert!(proof1.verify(&leaf1, &tree.root));
        assert!(proof2.verify(&leaf2, &tree.root));
        assert!(proof3.verify(&leaf3, &tree.root));

        // Wrong leaf should fail
        assert!(!proof1.verify(&leaf2, &tree.root));
    }
}
```

## 2.5 Encryption for TEE

AES-GCM encryption for order data sent to TEE.

`sp1-programs/lib/deplob-core/src/encryption.rs`:

```rust
//! Encryption utilities for TEE communication
//!
//! Uses AES-256-GCM for symmetric encryption
//! Uses ECDH for key exchange

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use serde::{Deserialize, Serialize};

/// Encrypted data with nonce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Ciphertext
    pub ciphertext: Vec<u8>,
    /// Nonce (12 bytes for AES-GCM)
    pub nonce: [u8; 12],
}

/// Encrypt data with AES-256-GCM
pub fn encrypt_aes_gcm(
    key: &[u8; 32],
    plaintext: &[u8],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, &'static str> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "Invalid key")?;
    let nonce = Nonce::from_slice(nonce);

    cipher.encrypt(nonce, plaintext).map_err(|_| "Encryption failed")
}

/// Decrypt data with AES-256-GCM
pub fn decrypt_aes_gcm(
    key: &[u8; 32],
    ciphertext: &[u8],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, &'static str> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "Invalid key")?;
    let nonce = Nonce::from_slice(nonce);

    cipher.decrypt(nonce, ciphertext).map_err(|_| "Decryption failed")
}

/// Generate random nonce
pub fn generate_nonce() -> [u8; 12] {
    use rand::RngCore;
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

/// Generate random 32-byte key
pub fn generate_key() -> [u8; 32] {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = generate_key();
        let nonce = generate_nonce();
        let plaintext = b"Hello, DePLOB!";

        let ciphertext = encrypt_aes_gcm(&key, plaintext, &nonce).unwrap();
        let decrypted = decrypt_aes_gcm(&key, &ciphertext, &nonce).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }
}
```

Add to `Cargo.toml` workspace dependencies:
```toml
aes-gcm = "0.10"
rand = "0.8"
```

## 2.6 Update Core Library

`sp1-programs/lib/deplob-core/src/lib.rs`:

```rust
//! DePLOB Core Library
//!
//! Shared cryptographic primitives and types for all SP1 programs

pub mod commitment;
pub mod encryption;
pub mod keccak;
pub mod merkle;
pub mod poseidon;

// Re-exports
pub use commitment::{Commitment, CommitmentPreimage, Nullifier};
pub use merkle::{IncrementalMerkleTree, MerkleProof, TREE_DEPTH};
```

## 2.7 Solidity Compatibility

Ensure Rust implementations match Solidity. Create test vectors.

`sp1-programs/lib/deplob-core/src/test_vectors.rs`:

```rust
//! Test vectors for Solidity compatibility

#[cfg(test)]
mod tests {
    use crate::keccak::keccak256;

    #[test]
    fn test_keccak_solidity_compatibility() {
        // This should match: keccak256(abi.encodePacked(uint256(1)))
        let mut input = [0u8; 32];
        input[31] = 1;  // uint256(1) in big-endian

        let hash = keccak256(&input);

        // Verify this matches Solidity output
        // (Run in Foundry to get expected value)
        println!("keccak256(1) = 0x{}", hex::encode(hash));
    }
}
```

## 2.8 Checklist

- [ ] Poseidon hash compiles and tests pass
- [ ] Keccak256 matches Solidity output
- [ ] Commitment scheme works correctly
- [ ] Merkle tree insert/proof/verify works
- [ ] AES-GCM encryption/decryption works
- [ ] All test vectors verified against Solidity
