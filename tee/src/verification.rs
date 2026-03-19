use deplob_core::{CommitmentPreimage, MerkleProof, TREE_DEPTH};
use thiserror::Error;

use crate::types::OrderSide;

#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("Invalid Merkle proof: leaf not in root")]
    InvalidMerkleProof,
    #[error("Insufficient deposit: need {needed}, have {available}")]
    InsufficientDeposit { needed: u128, available: u128 },
    #[error("Token mismatch: deposit token does not match order token_in")]
    TokenMismatch,
    #[error("Invalid proof path length: expected {}, got {0}", TREE_DEPTH)]
    InvalidPathLength(usize),
    #[error("Price overflow computing required amount")]
    PriceOverflow,
}

/// Reconstruct the commitment and nullifier from deposit secrets and verify
/// the deposit's Merkle inclusion proof.
///
/// Returns `(commitment, deposit_nullifier)` on success.
pub fn verify_deposit_ownership(
    nullifier_note: [u8; 32],
    secret: [u8; 32],
    token: [u8; 20],
    amount: u128,
    merkle_root: [u8; 32],
    siblings: &[[u8; 32]],
    path_indices: &[u8],
) -> Result<([u8; 32], [u8; 32]), VerifyError> {
    if siblings.len() != TREE_DEPTH || path_indices.len() != TREE_DEPTH {
        return Err(VerifyError::InvalidPathLength(siblings.len()));
    }

    let preimage = CommitmentPreimage::new(nullifier_note, secret, token, amount);
    let commitment = preimage.commitment();
    let nullifier = preimage.nullifier();

    let mut sib_array = [[0u8; 32]; TREE_DEPTH];
    let mut idx_array = [0u8; TREE_DEPTH];
    sib_array.copy_from_slice(siblings);
    idx_array.copy_from_slice(path_indices);

    let proof = MerkleProof {
        siblings: sib_array,
        path_indices: idx_array,
    };

    if !proof.verify(&commitment, &merkle_root) {
        return Err(VerifyError::InvalidMerkleProof);
    }

    Ok((commitment, nullifier))
}

/// Verify that the deposit has sufficient balance to back the order.
pub fn verify_deposit_covers_order(
    deposit_token: [u8; 20],
    deposit_amount: u128,
    side: OrderSide,
    order_token_in: [u8; 20],
    order_quantity: u128,
    order_price: u128,
) -> Result<(), VerifyError> {
    if deposit_token != order_token_in {
        return Err(VerifyError::TokenMismatch);
    }

    let required = match side {
        // Sell: deposit must cover the quantity being offered
        OrderSide::Sell => order_quantity,
        // Buy: deposit must cover quantity * price (cost of purchase)
        OrderSide::Buy => order_quantity
            .checked_mul(order_price)
            .ok_or(VerifyError::PriceOverflow)?,
    };

    if deposit_amount < required {
        return Err(VerifyError::InsufficientDeposit {
            needed: required,
            available: deposit_amount,
        });
    }

    Ok(())
}
