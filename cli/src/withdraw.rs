use anyhow::{Context, Result};
use clap::Args;

use alloy::{
    network::EthereumWallet,
    primitives::{Address, Bytes, FixedBytes, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol,
};
use deplob_core::CommitmentPreimage;

use crate::{config::ChainArgs, indexer::MerkleIndexer};

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IDePLOBWithdraw {
        function withdraw(
            bytes32 nullifierHash,
            address recipient,
            address token,
            uint256 amount,
            bytes32 root,
            bytes calldata proof
        ) external;
    }
}

#[derive(Args)]
pub struct WithdrawArgs {
    /// Path to the deposit note JSON file
    #[arg(long)]
    pub note: String,

    /// Recipient address (hex) — can be a fresh wallet for anonymity
    #[arg(long)]
    pub recipient: String,

    /// Path to a proof file (e.g. withdraw_proof.bin from withdraw-script).
    /// If omitted, sends empty proof bytes (only works with MockSP1Verifier).
    #[arg(long)]
    pub proof: Option<String>,

    #[command(flatten)]
    pub chain: ChainArgs,
}

pub async fn run(args: WithdrawArgs) -> Result<()> {
    // Load deposit note
    let note_str = std::fs::read_to_string(&args.note)
        .with_context(|| format!("failed to read note file: {}", args.note))?;
    let note: serde_json::Value = serde_json::from_str(&note_str)?;

    let nullifier_note = parse_hex32(
        note["nullifier_note"]
            .as_str()
            .context("missing nullifier_note")?,
    )?;
    let secret = parse_hex32(note["secret"].as_str().context("missing secret")?)?;
    let token_hex = note["token"].as_str().context("missing token")?;
    let token_bytes = parse_hex20(token_hex)?;
    let amount: u128 = note["amount"].as_str().context("missing amount")?.parse()?;

    // Recompute commitment and nullifier
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
    let commitment = preimage.commitment();
    let nullifier = preimage.nullifier();

    println!("Commitment: 0x{}", hex::encode(commitment));
    println!("Nullifier:  0x{}", hex::encode(nullifier));

    // Sync Merkle tree indexer and get proof
    println!("Syncing Merkle tree from on-chain events...");
    let indexer = MerkleIndexer::sync(&args.chain.rpc_url, &args.chain.contract).await?;
    let (_siblings, _path_indices, root) = indexer.proof_for(&commitment)?;

    println!("Merkle root: 0x{}", hex::encode(root));

    let proof = if let Some(proof_path) = &args.proof {
        let proof_bytes = std::fs::read(proof_path)
            .with_context(|| format!("failed to read proof file: {proof_path}"))?;
        println!(
            "Loaded proof from {} ({} bytes)",
            proof_path,
            proof_bytes.len()
        );
        Bytes::from(proof_bytes)
    } else {
        println!("No --proof file provided — using empty proof (mock verifier mode)");
        Bytes::new()
    };

    // Parse addresses
    let recipient_addr: Address = args
        .recipient
        .parse()
        .context("invalid recipient address")?;
    let token_addr: Address = token_hex.parse().context("invalid token address")?;
    let contract_addr: Address = args
        .chain
        .contract
        .parse()
        .context("invalid contract address")?;

    // Build provider with wallet
    let signer: PrivateKeySigner = args
        .chain
        .private_key
        .parse()
        .context("invalid private key")?;
    let wallet = EthereumWallet::from(signer);
    let url = args.chain.rpc_url.parse().context("invalid RPC URL")?;
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);

    // Submit withdrawal
    println!("Submitting withdrawal...");
    let deplob = IDePLOBWithdraw::IDePLOBWithdrawInstance::new(contract_addr, &provider);
    let tx = deplob
        .withdraw(
            FixedBytes(nullifier),
            recipient_addr,
            token_addr,
            U256::from(amount),
            FixedBytes(root),
            proof,
        )
        .send()
        .await
        .context("withdraw send failed")?;
    let receipt = tx.watch().await.context("withdraw watch failed")?;
    println!("Withdraw tx: 0x{}", hex::encode(receipt));
    println!("Tokens sent to {}", args.recipient);

    Ok(())
}

fn parse_hex32(s: &str) -> Result<[u8; 32]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("invalid hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 32 bytes"))
}

fn parse_hex20(s: &str) -> Result<[u8; 20]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("invalid hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 20 bytes"))
}
