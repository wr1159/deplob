//! Generate withdrawal test data for Foundry E2E tests
//!
//! Outputs JSON to stdout that can be parsed by Foundry's FFI
//!
//! This creates a complete withdrawal scenario:
//! 1. Generates a deposit (nullifier_note, secret, commitment)
//! 2. Creates a mock Merkle proof (commitment at index 0 in empty tree)
//! 3. Derives the nullifier
//! 4. Returns all data needed for the withdrawal
//!
//! Usage:
//!   cargo run --release --bin generate_withdraw_test_data -- <token_address> <amount> <recipient>

use deplob_core::{CommitmentPreimage, MerkleProof, TREE_DEPTH};
use sp1_sdk::{HashableKey, Prover, ProverClient, ProvingKey, SP1Stdin};
use std::env;

const ELF_BYTES: &[u8] = include_bytes!("../../../program/elf/riscv32im-succinct-zkvm-elf");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Parse arguments or use defaults
    let token_hex = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("abababababababababababababababababababab");
    let amount: u128 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000_000_000_000_000);
    let recipient_hex = args
        .get(3)
        .map(|s| s.as_str())
        .unwrap_or("1234567890123456789012345678901234567890");

    // Parse addresses
    let token_bytes: [u8; 20] = hex::decode(token_hex.trim_start_matches("0x"))
        .expect("Invalid token hex")
        .try_into()
        .expect("Token must be 20 bytes");

    let recipient_bytes: [u8; 20] = hex::decode(recipient_hex.trim_start_matches("0x"))
        .expect("Invalid recipient hex")
        .try_into()
        .expect("Recipient must be 20 bytes");

    // Generate random secrets (simulate a deposit)
    let nullifier_note: [u8; 32] = rand::random();
    let secret: [u8; 32] = rand::random();

    // Compute commitment and nullifier
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
    let commitment = preimage.commitment();
    let nullifier = preimage.nullifier();

    // Create a mock Merkle proof (commitment at index 0 in empty tree)
    let mut current = commitment;
    let mut siblings: [[u8; 32]; TREE_DEPTH] = [[0u8; 32]; TREE_DEPTH];
    let mut path_indices: [u8; TREE_DEPTH] = [0u8; TREE_DEPTH];

    // Build proof: commitment at leaf 0, all siblings are zero hashes
    for i in 0..TREE_DEPTH {
        let zero_hash = deplob_core::zero_hashes()[i];
        siblings[i] = zero_hash;
        path_indices[i] = 0; // Always go left (index 0)
        current = deplob_core::hash_pair(&current, &zero_hash);
    }

    let root = current;
    let leaf_index: u32 = 0;

    // Verify locally
    let merkle_proof = MerkleProof {
        siblings,
        path_indices,
    };
    let computed_root = merkle_proof.compute_root(&commitment);
    assert_eq!(
        root, computed_root,
        "Local Merkle proof verification failed"
    );

    // Initialize SP1 and execute to verify
    let client = ProverClient::from_env().await;

    let mut stdin = SP1Stdin::new();

    // Private inputs
    stdin.write(&nullifier_note);
    stdin.write(&secret);
    stdin.write(&siblings);
    stdin.write(&path_indices);
    stdin.write(&leaf_index);

    // Public inputs
    stdin.write(&root);
    stdin.write(&recipient_bytes);
    stdin.write(&token_bytes);
    stdin.write(&amount);

    // Execute to verify correctness
    let (mut output, _report) = client.execute(ELF_BYTES.into(), stdin.clone()).await?;

    // Read and verify outputs
    let output_nullifier: [u8; 32] = output.read::<[u8; 32]>();
    let output_root: [u8; 32] = output.read::<[u8; 32]>();
    let output_recipient: [u8; 20] = output.read::<[u8; 20]>();
    let output_token: [u8; 20] = output.read::<[u8; 20]>();
    let output_amount: u128 = output.read::<u128>();

    assert_eq!(nullifier, output_nullifier, "Nullifier mismatch");
    assert_eq!(root, output_root, "Root mismatch");
    assert_eq!(recipient_bytes, output_recipient, "Recipient mismatch");
    assert_eq!(token_bytes, output_token, "Token mismatch");
    assert_eq!(amount, output_amount, "Amount mismatch");

    // Get verification key
    let pk = client.setup(ELF_BYTES.into()).await?;
    let vk = pk.verifying_key();

    // Format siblings for JSON output
    let siblings_hex: Vec<String> = siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(s)))
        .collect();

    // Output JSON for Foundry FFI
    let output = serde_json::json!({
        // Deposit data (for creating the initial deposit)
        "commitment": format!("0x{}", hex::encode(commitment)),
        "nullifier_note": format!("0x{}", hex::encode(nullifier_note)),
        "secret": format!("0x{}", hex::encode(secret)),

        // Withdrawal public inputs
        "nullifier": format!("0x{}", hex::encode(nullifier)),
        "root": format!("0x{}", hex::encode(root)),
        "recipient": format!("0x{}", hex::encode(recipient_bytes)),
        "token": format!("0x{}", hex::encode(token_bytes)),
        "amount": amount.to_string(),

        // Merkle proof data
        "leaf_index": leaf_index,
        "siblings": siblings_hex,
        "path_indices": path_indices.to_vec(),

        // Verification
        "vkey": vk.bytes32(),
        "proof": "0x"  // Empty proof for SP1MockVerifier
    });

    // Print to stdout for FFI capture
    println!("{}", serde_json::to_string(&output)?);

    Ok(())
}
