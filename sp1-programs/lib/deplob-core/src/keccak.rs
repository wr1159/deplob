//! Keccak256 implementation matching Solidity's keccak256
//!
//! This provides EVM-compatible hashing for Merkle trees and commitments.

use tiny_keccak::{Hasher, Keccak};

/// Keccak256 hash of arbitrary data
///
/// Matches Solidity's `keccak256(abi.encodePacked(data))`
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    let mut output = [0u8; 32];
    hasher.update(data);
    hasher.finalize(&mut output);
    output
}

/// Hash two 32-byte values together (for Merkle tree nodes)
///
/// Matches Solidity's `keccak256(abi.encodePacked(left, right))`
pub fn keccak256_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(left);
    combined[32..].copy_from_slice(right);
    keccak256(&combined)
}

/// Hash multiple 32-byte values together
///
/// Used for commitment creation: `keccak256(abi.encodePacked(a, b, c, ...))`
pub fn keccak256_concat(inputs: &[[u8; 32]]) -> [u8; 32] {
    let mut data = Vec::with_capacity(inputs.len() * 32);
    for input in inputs {
        data.extend_from_slice(input);
    }
    keccak256(&data)
}

/// Convert a u128 to 32-byte big-endian representation
///
/// The value is right-aligned (padded with zeros on the left)
pub fn u128_to_bytes32(value: u128) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[16..].copy_from_slice(&value.to_be_bytes());
    bytes
}

/// Convert a 20-byte address to 32-byte representation
///
/// The address is right-aligned (padded with zeros on the left)
pub fn address_to_bytes32(address: &[u8; 20]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(address);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keccak256_empty() {
        // keccak256("") = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
        let hash = keccak256(&[]);
        let expected = hex::decode("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470").unwrap();
        assert_eq!(hash, expected.as_slice());
    }

    #[test]
    fn test_keccak256_hello() {
        // keccak256("hello") - known test vector
        let hash = keccak256(b"hello");
        let expected = hex::decode("1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8").unwrap();
        assert_eq!(hash, expected.as_slice());
    }

    #[test]
    fn test_keccak256_pair() {
        let left = [1u8; 32];
        let right = [2u8; 32];

        let hash1 = keccak256_pair(&left, &right);
        let hash2 = keccak256_pair(&left, &right);

        // Deterministic
        assert_eq!(hash1, hash2);

        // Order matters
        let hash3 = keccak256_pair(&right, &left);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_u128_to_bytes32() {
        let value: u128 = 1;
        let bytes = u128_to_bytes32(value);

        // Should be right-aligned
        assert_eq!(bytes[31], 1);
        assert_eq!(bytes[30], 0);
        assert_eq!(bytes[0], 0);
    }

    #[test]
    fn test_address_to_bytes32() {
        let address = [0xABu8; 20];
        let bytes = address_to_bytes32(&address);

        // First 12 bytes should be zero
        assert_eq!(&bytes[..12], &[0u8; 12]);
        // Last 20 bytes should be the address
        assert_eq!(&bytes[12..], &address);
    }
}
