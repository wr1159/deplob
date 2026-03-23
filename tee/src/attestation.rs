//! Settlement attestation — signs settlement data with a secp256k1 key.
//!
//! Three construction modes:
//! - `MockAttestationProvider` — returns empty bytes (Tier 1–3 and tests)
//! - `EcdsaAttestationProvider::new(key)` — from env var `TEE_ATTESTATION_KEY` (Phase A)
//! - `EcdsaAttestationProvider::from_sealed(path)` — load or generate key sealed to
//!   `/sealed/attestation.key` (Phase B — SGX enclave with Gramine)

use std::path::Path;

use anyhow::Context;

use alloy::{
    primitives::B256,
    signers::{local::PrivateKeySigner, SignerSync},
};
use deplob_core::keccak256_concat;

use crate::settlement::SettlementData;

// ============ Trait ============

/// Abstraction over attestation signing for settlement data.
pub trait AttestationProvider: Send + Sync {
    /// Sign a settlement, producing attestation bytes for on-chain verification.
    /// Returns empty bytes if attestation is not configured (mock mode).
    fn sign_settlement(&self, data: &SettlementData) -> anyhow::Result<Vec<u8>>;

    /// The address that corresponds to the signing key, if any.
    fn signing_address(&self) -> Option<String>;
}

// ============ Mock ============

/// Returns empty attestation bytes. Used in Tier 1–3 and unit tests.
pub struct MockAttestationProvider;

impl AttestationProvider for MockAttestationProvider {
    fn sign_settlement(&self, _data: &SettlementData) -> anyhow::Result<Vec<u8>> {
        Ok(vec![])
    }

    fn signing_address(&self) -> Option<String> {
        None
    }
}

// ============ ECDSA ============

/// Signs settlement data with a secp256k1 key for on-chain verification via `ecrecover`.
pub struct EcdsaAttestationProvider {
    signer: PrivateKeySigner,
}

const SEALED_KEY_PATH: &str = "/sealed/attestation.key";

impl EcdsaAttestationProvider {
    /// Construct from a hex-encoded private key (Phase A — env var).
    pub fn new(private_key: &str) -> anyhow::Result<Self> {
        let signer: PrivateKeySigner = private_key
            .parse()
            .context("invalid TEE_ATTESTATION_KEY")?;
        Ok(Self { signer })
    }

    /// Load or generate a sealed attestation key (Phase B — SGX enclave).
    ///
    /// If the file exists, reads and parses the hex key.
    /// If not, generates a new random key and writes it to the sealed path.
    /// Inside a Gramine SGX enclave, `/sealed/` is encrypted with the
    /// MRENCLAVE-derived key — the file can only be read by the same enclave.
    pub fn from_sealed(path: &str) -> anyhow::Result<Self> {
        let sealed = Path::new(path);

        let signer = if sealed.exists() {
            let hex_key = std::fs::read_to_string(sealed)
                .context("failed to read sealed attestation key")?;
            let hex_key = hex_key.trim();
            hex_key
                .parse::<PrivateKeySigner>()
                .context("failed to parse sealed attestation key")?
        } else {
            // Generate a new key and persist it
            let signer = PrivateKeySigner::random();
            let key_hex = format!("0x{}", hex::encode(signer.credential().to_bytes()));
            // Ensure parent directory exists
            if let Some(parent) = sealed.parent() {
                std::fs::create_dir_all(parent)
                    .context("failed to create sealed key directory")?;
            }
            std::fs::write(sealed, &key_hex)
                .context("failed to write sealed attestation key")?;
            tracing::info!("Generated new attestation key, sealed to {path}");
            signer
        };

        Ok(Self { signer })
    }
}

impl AttestationProvider for EcdsaAttestationProvider {
    fn sign_settlement(&self, data: &SettlementData) -> anyhow::Result<Vec<u8>> {
        let hash = settlement_hash(data);

        // sign_message applies EIP-191 prefix internally:
        //   keccak256("\x19Ethereum Signed Message:\n32" || hash)
        // This matches Solidity's ecrecover with the same prefix.
        let sig = self
            .signer
            .sign_message_sync(&hash)
            .context("attestation signing failed")?;

        Ok(sig.as_bytes().to_vec())
    }

    fn signing_address(&self) -> Option<String> {
        Some(format!("0x{}", hex::encode(self.signer.address().as_slice())))
    }
}

/// Compute the settlement hash that gets signed.
///
/// `keccak256(buyerOldNullifier || sellerOldNullifier || buyerNewCommitment || sellerNewCommitment)`
///
/// Must match the Solidity-side `keccak256(abi.encodePacked(...))`.
pub fn settlement_hash(data: &SettlementData) -> [u8; 32] {
    keccak256_concat(&[
        data.buyer_old_nullifier,
        data.seller_old_nullifier,
        data.buyer_new_commitment,
        data.seller_new_commitment,
    ])
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
    use deplob_core::CommitmentPreimage;

    fn mock_settlement() -> SettlementData {
        let buyer_preimage = CommitmentPreimage::new([1u8; 32], [2u8; 32], [0xAA; 20], 1000);
        let seller_preimage = CommitmentPreimage::new([3u8; 32], [4u8; 32], [0xBB; 20], 2000);
        SettlementData {
            buyer_old_nullifier: [0x10; 32],
            seller_old_nullifier: [0x20; 32],
            buyer_new_commitment: buyer_preimage.commitment(),
            seller_new_commitment: seller_preimage.commitment(),
            buyer_new_preimage: buyer_preimage,
            seller_new_preimage: seller_preimage,
        }
    }

    #[test]
    fn test_mock_returns_empty() {
        let provider = MockAttestationProvider;
        let data = mock_settlement();
        let result = provider.sign_settlement(&data).unwrap();
        assert!(result.is_empty());
        assert!(provider.signing_address().is_none());
    }

    #[test]
    fn test_ecdsa_produces_65_bytes() {
        // Anvil account 0 private key
        let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let provider = EcdsaAttestationProvider::new(key).unwrap();
        let data = mock_settlement();
        let sig = provider.sign_settlement(&data).unwrap();
        assert_eq!(sig.len(), 65, "ECDSA signature must be 65 bytes (r||s||v)");
    }

    #[test]
    fn test_ecdsa_recovers_correct_address() {
        let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let provider = EcdsaAttestationProvider::new(key).unwrap();
        let data = mock_settlement();
        let sig_bytes = provider.sign_settlement(&data).unwrap();

        // The signing address should be the Anvil account 0 address
        let expected_addr = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        let addr_str = provider.signing_address().unwrap();
        let expected_str = format!("0x{}", hex::encode(expected_addr.as_slice()));
        assert_eq!(addr_str.to_lowercase(), expected_str.to_lowercase());

        // Verify signature can be recovered to the correct address
        let hash = settlement_hash(&data);
        let sig = alloy::primitives::Signature::try_from(sig_bytes.as_slice()).unwrap();
        let hash_b256 = B256::from(hash);

        // EIP-191: keccak256("\x19Ethereum Signed Message:\n32" || hash)
        let prefixed = alloy::primitives::eip191_hash_message(hash_b256);
        let recovered = sig
            .recover_address_from_prehash(&prefixed)
            .expect("recovery should succeed");
        assert_eq!(recovered, expected_addr);
    }

    #[test]
    fn test_settlement_hash_deterministic() {
        let data = mock_settlement();
        let h1 = settlement_hash(&data);
        let h2 = settlement_hash(&data);
        assert_eq!(h1, h2, "settlement hash must be deterministic");
        assert_ne!(h1, [0u8; 32], "hash should not be zero");
    }
}
