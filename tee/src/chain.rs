use std::collections::HashSet;

use anyhow::Context;
use async_trait::async_trait;

use crate::settlement::SettlementData;

// ============ Contract ABI bindings ============

alloy::sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IDePLOB {
        function commitments(bytes32 commitment) external view returns (bool);
        function nullifierHashes(bytes32 nullifier) external view returns (bool);
        function isKnownRoot(bytes32 root) external view returns (bool);
        function settleMatch(
            bytes32 buyerOldNullifier,
            bytes32 sellerOldNullifier,
            bytes32 buyerNewCommitment,
            bytes32 sellerNewCommitment,
            bytes calldata attestation,
            bytes calldata proof
        ) external;
    }
}

// ============ ChainClient trait ============

/// Abstraction over on-chain state queries and settlement submission.
///
/// The real implementation (`AlloyChainClient`) uses an alloy provider to call
/// the deployed DePLOB contract on Sepolia. The mock implementation is used for
/// tests and local development (no chain required).
#[async_trait]
pub trait ChainClient: Send + Sync {
    /// Returns true if the commitment was previously deposited on-chain.
    async fn is_commitment_known(&self, commitment: [u8; 32]) -> anyhow::Result<bool>;
    /// Returns true if the nullifier has already been spent (withdrawn or settled).
    async fn is_nullifier_spent(&self, nullifier: [u8; 32]) -> anyhow::Result<bool>;
    /// Returns true if the given root exists in the Merkle root history.
    async fn is_known_root(&self, root: [u8; 32]) -> anyhow::Result<bool>;
    /// Submit a settled trade to the smart contract (`settleMatch`).
    async fn settle_match(&self, data: &SettlementData) -> anyhow::Result<()>;
}

// ============ MockChainClient ============

/// In-memory mock used in tests and local development.
#[derive(Debug, Default)]
pub struct MockChainClient {
    pub known_commitments: HashSet<[u8; 32]>,
    pub spent_nullifiers: HashSet<[u8; 32]>,
    pub known_roots: HashSet<[u8; 32]>,
    /// Settled trades recorded for assertions in tests.
    pub settlements: std::sync::Mutex<Vec<SettlementData>>,
}

impl MockChainClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populate with a known commitment (simulates a successful deposit).
    pub fn add_commitment(&mut self, commitment: [u8; 32]) {
        self.known_commitments.insert(commitment);
    }

    pub fn add_known_root(&mut self, root: [u8; 32]) {
        self.known_roots.insert(root);
    }
}

#[async_trait]
impl ChainClient for MockChainClient {
    async fn is_commitment_known(&self, commitment: [u8; 32]) -> anyhow::Result<bool> {
        Ok(self.known_commitments.contains(&commitment))
    }

    async fn is_nullifier_spent(&self, nullifier: [u8; 32]) -> anyhow::Result<bool> {
        Ok(self.spent_nullifiers.contains(&nullifier))
    }

    async fn is_known_root(&self, root: [u8; 32]) -> anyhow::Result<bool> {
        Ok(self.known_roots.contains(&root))
    }

    async fn settle_match(&self, data: &SettlementData) -> anyhow::Result<()> {
        self.settlements
            .lock()
            .expect("settlements lock")
            .push(data.clone());
        Ok(())
    }
}

// ============ AlloyChainClient ============

use alloy::{
    network::EthereumWallet,
    primitives::{Address, Bytes, FixedBytes},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};

/// Production chain client that makes `eth_call` / `eth_sendTransaction` calls
/// to the deployed DePLOB contract.
///
/// Requires three environment variables when starting the TEE:
/// - `ETH_RPC_URL`     — e.g. `https://sepolia.infura.io/v3/<key>`
/// - `DEPLOB_ADDRESS`  — deployed DePLOB contract address
/// - `TEE_PRIVATE_KEY` — hex private key of the TEE operator wallet
///
/// The TEE operator address (derived from `TEE_PRIVATE_KEY`) must match the
/// `teeOperator` set in the DePLOB contract constructor.
pub struct AlloyChainClient {
    rpc_url: String,
    address: Address,
    signer: PrivateKeySigner,
}

impl AlloyChainClient {
    pub fn new(rpc_url: String, address: Address, signer: PrivateKeySigner) -> Self {
        Self {
            rpc_url,
            address,
            signer,
        }
    }

    pub async fn from_env(rpc_url: &str, address: &str, private_key: &str) -> anyhow::Result<Self> {
        let address: Address = address.parse().context("invalid DEPLOB_ADDRESS")?;
        let signer: PrivateKeySigner = private_key.parse().context("invalid TEE_PRIVATE_KEY")?;
        Ok(Self::new(rpc_url.to_string(), address, signer))
    }
}

#[async_trait]
impl ChainClient for AlloyChainClient {
    async fn is_commitment_known(&self, commitment: [u8; 32]) -> anyhow::Result<bool> {
        let url = self.rpc_url.parse().context("invalid ETH_RPC_URL")?;
        let provider = ProviderBuilder::new().connect_http(url);
        let contract = IDePLOB::IDePLOBInstance::new(self.address, provider);
        let result = contract
            .commitments(FixedBytes(commitment))
            .call()
            .await
            .context("commitments() call failed")?;
        Ok(result)
    }

    async fn is_nullifier_spent(&self, nullifier: [u8; 32]) -> anyhow::Result<bool> {
        let url = self.rpc_url.parse().context("invalid ETH_RPC_URL")?;
        let provider = ProviderBuilder::new().connect_http(url);
        let contract = IDePLOB::IDePLOBInstance::new(self.address, provider);
        let result = contract
            .nullifierHashes(FixedBytes(nullifier))
            .call()
            .await
            .context("nullifierHashes() call failed")?;
        Ok(result)
    }

    async fn is_known_root(&self, root: [u8; 32]) -> anyhow::Result<bool> {
        let url = self.rpc_url.parse().context("invalid ETH_RPC_URL")?;
        let provider = ProviderBuilder::new().connect_http(url);
        let contract = IDePLOB::IDePLOBInstance::new(self.address, provider);
        let result = contract
            .isKnownRoot(FixedBytes(root))
            .call()
            .await
            .context("isKnownRoot() call failed")?;
        Ok(result)
    }

    async fn settle_match(&self, data: &SettlementData) -> anyhow::Result<()> {
        let url = self.rpc_url.parse().context("invalid ETH_RPC_URL")?;
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);
        let contract = IDePLOB::IDePLOBInstance::new(self.address, provider);
        contract
            .settleMatch(
                FixedBytes(data.buyer_old_nullifier),
                FixedBytes(data.seller_old_nullifier),
                FixedBytes(data.buyer_new_commitment),
                FixedBytes(data.seller_new_commitment),
                Bytes::new(), // attestation — TODO: real TEE attestation
                Bytes::new(), // proof — TODO: settlement proof
            )
            .send()
            .await
            .context("settleMatch send failed")?
            .watch()
            .await
            .context("settleMatch watch failed")?;
        Ok(())
    }
}
