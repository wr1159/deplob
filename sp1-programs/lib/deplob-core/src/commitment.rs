//! Commitment scheme for DePLOB
//!
//! A commitment hides the details of a deposit while allowing the user
//! to prove ownership later. The scheme is based on Tornado Cash:
//!
//! commitment = hash(nullifier_note || secret || token || amount)
//! nullifier = hash(nullifier_note)
//!
//! The nullifier is revealed when spending to prevent double-spending,
//! but it cannot be linked back to the original commitment.

use crate::keccak::{keccak256_concat, address_to_bytes32, u128_to_bytes32};
use serde::{Deserialize, Serialize};

/// Raw commitment preimage (private data the user must store)
///
/// This contains all the secret values needed to spend the commitment later.
/// Users must securely backup this data - if lost, funds are unrecoverable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentPreimage {
    /// Random 32-byte value used to derive the nullifier
    pub nullifier_note: [u8; 32],

    /// Random 32-byte secret for commitment hiding
    pub secret: [u8; 32],

    /// Token contract address (20 bytes, stored as 32 for hashing)
    pub token: [u8; 32],

    /// Deposit amount (stored as 32 bytes for hashing)
    pub amount: [u8; 32],
}

impl CommitmentPreimage {
    /// Create a new commitment preimage
    ///
    /// # Arguments
    /// * `nullifier_note` - Random 32 bytes (user generates)
    /// * `secret` - Random 32 bytes (user generates)
    /// * `token` - Token contract address (20 bytes)
    /// * `amount` - Deposit amount
    pub fn new(
        nullifier_note: [u8; 32],
        secret: [u8; 32],
        token: [u8; 20],
        amount: u128,
    ) -> Self {
        Self {
            nullifier_note,
            secret,
            token: address_to_bytes32(&token),
            amount: u128_to_bytes32(amount),
        }
    }

    /// Compute the commitment hash
    ///
    /// commitment = keccak256(nullifier_note || secret || token || amount)
    pub fn commitment(&self) -> [u8; 32] {
        keccak256_concat(&[
            self.nullifier_note,
            self.secret,
            self.token,
            self.amount,
        ])
    }

    /// Compute the nullifier for spending
    ///
    /// nullifier = keccak256(nullifier_note)
    ///
    /// The nullifier is revealed when withdrawing. It prevents double-spending
    /// because the same nullifier_note always produces the same nullifier.
    pub fn nullifier(&self) -> [u8; 32] {
        keccak256_concat(&[self.nullifier_note])
    }

    /// Get the token address as 20 bytes
    pub fn token_address(&self) -> [u8; 20] {
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&self.token[12..]);
        addr
    }

    /// Get the amount as u128
    pub fn amount_value(&self) -> u128 {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.amount[16..]);
        u128::from_be_bytes(bytes)
    }
}

/// Public commitment hash (stored on-chain in Merkle tree)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Commitment(pub [u8; 32]);

impl Commitment {
    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Commitment(bytes)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<&CommitmentPreimage> for Commitment {
    fn from(preimage: &CommitmentPreimage) -> Self {
        Commitment(preimage.commitment())
    }
}

impl From<[u8; 32]> for Commitment {
    fn from(bytes: [u8; 32]) -> Self {
        Commitment(bytes)
    }
}

/// Nullifier hash (revealed when spending)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Nullifier(pub [u8; 32]);

impl Nullifier {
    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Nullifier(bytes)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<&CommitmentPreimage> for Nullifier {
    fn from(preimage: &CommitmentPreimage) -> Self {
        Nullifier(preimage.nullifier())
    }
}

impl From<[u8; 32]> for Nullifier {
    fn from(bytes: [u8; 32]) -> Self {
        Nullifier(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_preimage_creation() {
        let nullifier_note = [1u8; 32];
        let secret = [2u8; 32];
        let token = [0xABu8; 20];
        let amount: u128 = 1_000_000_000_000_000_000; // 1 ETH in wei

        let preimage = CommitmentPreimage::new(nullifier_note, secret, token, amount);

        // Check token address extraction
        assert_eq!(preimage.token_address(), token);

        // Check amount extraction
        assert_eq!(preimage.amount_value(), amount);
    }

    #[test]
    fn test_commitment_deterministic() {
        let nullifier_note = [1u8; 32];
        let secret = [2u8; 32];
        let token = [0xABu8; 20];
        let amount: u128 = 1_000_000_000_000_000_000;

        let preimage1 = CommitmentPreimage::new(nullifier_note, secret, token, amount);
        let preimage2 = CommitmentPreimage::new(nullifier_note, secret, token, amount);

        // Same inputs = same commitment
        assert_eq!(preimage1.commitment(), preimage2.commitment());

        // Same inputs = same nullifier
        assert_eq!(preimage1.nullifier(), preimage2.nullifier());
    }

    #[test]
    fn test_commitment_different_from_nullifier() {
        let preimage = CommitmentPreimage::new(
            [1u8; 32],
            [2u8; 32],
            [0xABu8; 20],
            1_000_000_000_000_000_000,
        );

        // Commitment and nullifier should be different
        assert_ne!(preimage.commitment(), preimage.nullifier());
    }

    #[test]
    fn test_different_inputs_different_outputs() {
        let preimage1 = CommitmentPreimage::new(
            [1u8; 32],
            [2u8; 32],
            [0xABu8; 20],
            1_000_000_000_000_000_000,
        );

        let preimage2 = CommitmentPreimage::new(
            [3u8; 32], // Different nullifier_note
            [2u8; 32],
            [0xABu8; 20],
            1_000_000_000_000_000_000,
        );

        // Different inputs = different commitment
        assert_ne!(preimage1.commitment(), preimage2.commitment());

        // Different nullifier_note = different nullifier
        assert_ne!(preimage1.nullifier(), preimage2.nullifier());
    }

    #[test]
    fn test_commitment_type_conversions() {
        let preimage = CommitmentPreimage::new(
            [1u8; 32],
            [2u8; 32],
            [0xABu8; 20],
            1_000_000_000_000_000_000,
        );

        let commitment: Commitment = (&preimage).into();
        let nullifier: Nullifier = (&preimage).into();

        assert_eq!(commitment.as_bytes(), &preimage.commitment());
        assert_eq!(nullifier.as_bytes(), &preimage.nullifier());
    }
}
