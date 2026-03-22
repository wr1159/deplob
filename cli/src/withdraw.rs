use anyhow::{Context, Result};
use clap::Args;

use alloy::{
    network::EthereumWallet,
    primitives::{Address, Bytes, FixedBytes, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol,
};
use deplob_core::{CommitmentPreimage, TREE_DEPTH};
use sp1_sdk::{HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Stdin};

use crate::{config::ChainArgs, indexer::MerkleIndexer};

/// Withdraw program ELF — built by `cargo-prove prove build` in sp1-programs/withdraw/program.
const ELF_BYTES: &[u8] =
    include_bytes!("../../sp1-programs/withdraw/program/elf/riscv32im-succinct-zkvm-elf");

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

    /// Proof source: "generate" to create an SP1 proof inline via the network,
    /// a file path to load a pre-generated proof, or omit for empty proof (mock verifier).
    #[arg(long)]
    pub proof: Option<String>,

    /// Proof system when using --proof generate (groth16 or plonk)
    #[arg(long, default_value = "groth16")]
    pub proof_type: String,

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
    let (siblings, path_indices, root, leaf_index) = indexer.proof_for(&commitment)?;

    println!("Merkle root: 0x{}", hex::encode(root));

    let recipient_bytes_20: [u8; 20] = {
        let addr: Address = args.recipient.parse().context("invalid recipient address")?;
        *addr.0
    };

    let proof = match args.proof.as_deref() {
        Some("generate") => {
            generate_sp1_proof(
                &nullifier_note,
                &secret,
                &siblings,
                &path_indices,
                leaf_index,
                &root,
                &recipient_bytes_20,
                &token_bytes,
                amount,
                &args.proof_type,
            )
            .await?
        }
        Some(proof_path) => {
            let proof_bytes = std::fs::read(proof_path)
                .with_context(|| format!("failed to read proof file: {proof_path}"))?;
            println!(
                "Loaded proof from {} ({} bytes)",
                proof_path,
                proof_bytes.len()
            );
            Bytes::from(proof_bytes)
        }
        None => {
            println!("No --proof provided, using empty proof (mock verifier mode)");
            Bytes::new()
        }
    };

    // Parse addresses
    let recipient_addr = Address::from(recipient_bytes_20);
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

/// Generate an SP1 ZK proof (Groth16 or Plonk) for the withdrawal circuit.
///
/// Uses `ProverClient::from_env()` which reads `SP1_PROVER` (network/local)
/// and `SP1_PRIVATE_KEY` for network authentication.
async fn generate_sp1_proof(
    nullifier_note: &[u8; 32],
    secret: &[u8; 32],
    siblings: &[[u8; 32]],
    path_indices: &[u8],
    leaf_index: u32,
    root: &[u8; 32],
    recipient: &[u8; 20],
    token: &[u8; 20],
    amount: u128,
    proof_type: &str,
) -> Result<Bytes> {
    // Convert to fixed-size arrays expected by SP1 stdin
    let merkle_siblings: [[u8; 32]; TREE_DEPTH] = siblings
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected {TREE_DEPTH} siblings"))?;
    let merkle_path_indices: [u8; TREE_DEPTH] = path_indices
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected {TREE_DEPTH} path indices"))?;

    // Build SP1 stdin — must match the exact write order in withdraw/program/src/main.rs
    let mut stdin = SP1Stdin::new();

    // Private inputs
    stdin.write(nullifier_note);
    stdin.write(secret);
    stdin.write(&merkle_siblings);
    stdin.write(&merkle_path_indices);
    stdin.write(&leaf_index);

    // Public inputs
    stdin.write(root);
    stdin.write(recipient);
    stdin.write(token);
    stdin.write(&amount);

    println!("Initializing SP1 prover...");
    let client = ProverClient::from_env().await;

    let pk = client
        .setup(ELF_BYTES.into())
        .await
        .context("SP1 setup failed")?;
    let vk = pk.verifying_key().clone();
    println!("Verification key: {}", vk.bytes32());

    let proof_bytes = match proof_type {
        "plonk" => {
            println!("Generating Plonk proof via SP1 network...");
            let proof = client
                .prove(&pk, stdin)
                .plonk()
                .await
                .context("Plonk proof generation failed")?;
            println!("Plonk proof generated successfully!");
            proof.bytes()
        }
        _ => {
            println!("Generating Groth16 proof via SP1 network...");
            let proof = client
                .prove(&pk, stdin)
                .groth16()
                .await
                .context("Groth16 proof generation failed")?;
            println!("Groth16 proof generated successfully!");
            proof.bytes()
        }
    };

    println!("Proof size: {} bytes", proof_bytes.len());
    Ok(Bytes::from(proof_bytes))
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
