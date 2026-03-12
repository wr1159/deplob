//! Withdrawal Program
//!
//! Proves:
//! 1. Knowledge of (nullifier_note, secret) such that commitment exists in tree
//! 2. Nullifier is correctly derived from nullifier_note
//! 3. Commitment matches the claimed token and amount
//!
//! Public Inputs:
//!   - nullifier: bytes32
//!   - root: bytes32
//!   - recipient: address (bytes20)
//!   - token: address (bytes20)
//!   - amount: uint128
//!
//! Private Inputs:
//!   - nullifier_note: bytes32
//!   - secret: bytes32
//!   - merkle_proof: (siblings, path_indices)
//!   - leaf_index: u32

#![no_main]
sp1_zkvm::entrypoint!(main);

use deplob_core::{CommitmentPreimage, MerkleProof, TREE_DEPTH};

pub fn main() {
    // ============ Read Private Inputs ============
    let nullifier_note: [u8; 32] = sp1_zkvm::io::read();
    let secret: [u8; 32] = sp1_zkvm::io::read();
    let merkle_siblings: [[u8; 32]; TREE_DEPTH] = sp1_zkvm::io::read();
    let merkle_path_indices: [u8; TREE_DEPTH] = sp1_zkvm::io::read();
    let _leaf_index: u32 = sp1_zkvm::io::read();

    // ============ Read Public Inputs ============
    let expected_root: [u8; 32] = sp1_zkvm::io::read();
    let recipient: [u8; 20] = sp1_zkvm::io::read();
    let token: [u8; 20] = sp1_zkvm::io::read();
    let amount: u128 = sp1_zkvm::io::read();

    // ============ Reconstruct Commitment ============
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token, amount);

    let commitment = preimage.commitment();
    let nullifier = preimage.nullifier();

    // ============ Verify Merkle Proof ============
    let merkle_proof = MerkleProof {
        siblings: merkle_siblings,
        path_indices: merkle_path_indices,
    };

    let computed_root = merkle_proof.compute_root(&commitment);

    // Assert the computed root matches the expected root
    assert_eq!(
        computed_root, expected_root,
        "Merkle proof verification failed"
    );

    // ============ Commit Public Outputs ============
    // These are verified by the smart contract
    sp1_zkvm::io::commit(&nullifier);
    sp1_zkvm::io::commit(&expected_root);
    sp1_zkvm::io::commit(&recipient);
    sp1_zkvm::io::commit(&token);
    sp1_zkvm::io::commit(&amount);
}
