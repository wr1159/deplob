//! Withdrawal Proof Generation Script
//!
//! This script:
//! 1. Reads a deposit note (nullifier_note, secret, commitment, etc.)
//! 2. Accepts a Merkle proof from the contract
//! 3. Executes the SP1 program to verify correctness
//! 4. Optionally generates a real ZK proof for on-chain verification
//!
//! Usage:
//!   cargo run --release --bin withdraw-script -- <deposit_note.json> <merkle_proof.json> <recipient>
//!
//! Or with real proof generation:
//!   GENERATE_PROOF=groth16 cargo run --release --bin withdraw-script -- <deposit_note.json> <merkle_proof.json> <recipient>

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolType};
use deplob_core::{CommitmentPreimage, MerkleProof, TREE_DEPTH};
use sp1_sdk::{HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Stdin};
use std::env;

const ELF_BYTES: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

// Solidity ABI encoding for public values
sol! {
    struct WithdrawPublicValues {
        bytes32 nullifier;
        bytes32 root;
        address recipient;
        address token;
        uint256 amount;
    }
}

/// Merkle proof data loaded from JSON
#[derive(serde::Deserialize)]
struct MerkleProofData {
    siblings: Vec<String>,
    path_indices: Vec<u8>,
    root: String,
    leaf_index: u32,
}

/// Deposit note data loaded from JSON
#[derive(serde::Deserialize)]
struct DepositNote {
    nullifier_note: String,
    secret: String,
    #[allow(dead_code)]
    commitment: String,
    token: String,
    amount: String,
}

fn parse_bytes32(hex_str: &str) -> [u8; 32] {
    let clean = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(clean).expect("Invalid hex string");
    bytes.try_into().expect("Expected 32 bytes")
}

fn parse_bytes20(hex_str: &str) -> [u8; 20] {
    let clean = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(clean).expect("Invalid hex string");
    bytes.try_into().expect("Expected 20 bytes")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize SP1 prover
    sp1_sdk::utils::setup_logger();
    let client = ProverClient::from_env().await;

    let args: Vec<String> = env::args().collect();

    // ============ Load Inputs ============

    let (deposit_note, merkle_proof_data, recipient_bytes) = if args.len() >= 4 {
        // Load from files
        let deposit_note_path = &args[1];
        let merkle_proof_path = &args[2];
        let recipient_hex = &args[3];

        let deposit_note: DepositNote =
            serde_json::from_str(&std::fs::read_to_string(deposit_note_path)?)?;
        let merkle_proof_data: MerkleProofData =
            serde_json::from_str(&std::fs::read_to_string(merkle_proof_path)?)?;
        let recipient_bytes = parse_bytes20(recipient_hex);

        (deposit_note, merkle_proof_data, recipient_bytes)
    } else {
        // Demo mode with mock data
        println!("=== Demo Mode (no files provided) ===\n");
        println!("Usage: cargo run --release --bin withdraw-script -- <deposit_note.json> <merkle_proof.json> <recipient>\n");

        // Generate fresh deposit
        let nullifier_note: [u8; 32] = rand::random();
        let secret: [u8; 32] = rand::random();
        let token_bytes: [u8; 20] = [0xAB; 20];
        let amount: u128 = 1_000_000_000_000_000_000;

        let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
        let commitment = preimage.commitment();

        // Create a mock Merkle tree with just this commitment
        let mut current = commitment;
        let mut siblings = Vec::new();
        let mut path_indices = Vec::new();

        // Build a simple proof (commitment at index 0)
        for i in 0..TREE_DEPTH {
            // Zero hash at this level (sibling is empty)
            let zero_hash = deplob_core::zero_hashes()[i];
            siblings.push(format!("0x{}", hex::encode(zero_hash)));
            path_indices.push(0); // Always go left
            current = deplob_core::hash_pair(&current, &zero_hash);
        }

        let root = current;

        let deposit_note = DepositNote {
            nullifier_note: hex::encode(nullifier_note),
            secret: hex::encode(secret),
            commitment: hex::encode(commitment),
            token: hex::encode(token_bytes),
            amount: amount.to_string(),
        };

        let merkle_proof_data = MerkleProofData {
            siblings,
            path_indices,
            root: format!("0x{}", hex::encode(root)),
            leaf_index: 0,
        };

        let recipient_bytes: [u8; 20] = rand::random();

        (deposit_note, merkle_proof_data, recipient_bytes)
    };

    // Parse deposit note
    let nullifier_note = parse_bytes32(&deposit_note.nullifier_note);
    let secret = parse_bytes32(&deposit_note.secret);
    let token_bytes = parse_bytes20(&deposit_note.token);
    let amount: u128 = deposit_note.amount.parse()?;

    // Parse Merkle proof
    let merkle_siblings: [[u8; 32]; TREE_DEPTH] = merkle_proof_data
        .siblings
        .iter()
        .map(|s| parse_bytes32(s))
        .collect::<Vec<_>>()
        .try_into()
        .expect("Expected TREE_DEPTH siblings");

    let merkle_path_indices: [u8; TREE_DEPTH] = merkle_proof_data
        .path_indices
        .try_into()
        .expect("Expected TREE_DEPTH path indices");

    let expected_root = parse_bytes32(&merkle_proof_data.root);
    let leaf_index = merkle_proof_data.leaf_index;

    // Compute expected values
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
    let commitment = preimage.commitment();
    let expected_nullifier = preimage.nullifier();

    // Verify Merkle proof locally first
    let merkle_proof = MerkleProof {
        siblings: merkle_siblings,
        path_indices: merkle_path_indices,
    };
    let computed_root = merkle_proof.compute_root(&commitment);
    assert_eq!(
        computed_root, expected_root,
        "Local Merkle proof verification failed"
    );

    println!("=== Withdrawal Proof Generation ===\n");
    println!("Private Inputs:");
    println!("  nullifier_note: 0x{}", hex::encode(nullifier_note));
    println!("  secret: 0x{}", hex::encode(secret));
    println!("  leaf_index: {}", leaf_index);
    println!("\nPublic Inputs:");
    println!("  root: 0x{}", hex::encode(expected_root));
    println!("  recipient: 0x{}", hex::encode(recipient_bytes));
    println!("  token: 0x{}", hex::encode(token_bytes));
    println!("  amount: {} wei", amount);
    println!("\nDerived Values:");
    println!("  commitment: 0x{}", hex::encode(commitment));
    println!("  nullifier: 0x{}", hex::encode(expected_nullifier));

    // ============ Create SP1 Stdin ============

    let mut stdin = SP1Stdin::new();

    // Private inputs
    stdin.write(&nullifier_note);
    stdin.write(&secret);
    stdin.write(&merkle_siblings);
    stdin.write(&merkle_path_indices);
    stdin.write(&leaf_index);

    // Public inputs
    stdin.write(&expected_root);
    stdin.write(&recipient_bytes);
    stdin.write(&token_bytes);
    stdin.write(&amount);

    // ============ Execute (for testing) ============

    println!("\n--- Executing SP1 Program ---");
    let elf = ELF_BYTES.into();
    let (mut output, report) = client.execute(elf, stdin.clone()).await?;
    println!(
        "Execution successful! Cycles: {}",
        report.total_instruction_count()
    );

    // Read outputs from SP1 program
    let nullifier: [u8; 32] = output.read::<[u8; 32]>();
    let root_out: [u8; 32] = output.read::<[u8; 32]>();
    let recipient_out: [u8; 20] = output.read::<[u8; 20]>();
    let token_out: [u8; 20] = output.read::<[u8; 20]>();
    let amount_out: u128 = output.read::<u128>();

    println!("\nSP1 Program Outputs:");
    println!("  nullifier: 0x{}", hex::encode(nullifier));
    println!("  root: 0x{}", hex::encode(root_out));
    println!("  recipient: 0x{}", hex::encode(recipient_out));
    println!("  token: 0x{}", hex::encode(token_out));
    println!("  amount: {}", amount_out);

    // Verify outputs match expectations
    assert_eq!(nullifier, expected_nullifier, "Nullifier mismatch!");
    assert_eq!(root_out, expected_root, "Root mismatch!");
    assert_eq!(recipient_out, recipient_bytes, "Recipient mismatch!");
    assert_eq!(token_out, token_bytes, "Token mismatch!");
    assert_eq!(amount_out, amount, "Amount mismatch!");
    println!("\nAll outputs verified!");

    // ============ Generate Proof (Optional) ============

    let proof_type = env::var("GENERATE_PROOF").unwrap_or_default();

    if !proof_type.is_empty() && proof_type != "false" {
        let pk = client.setup(ELF_BYTES.into()).await?;
        let vk = pk.verifying_key().clone();
        println!("\n--- Generating SP1 Proof ---");
        println!("Verification key: {}", vk.bytes32());

        let proof_bytes = match proof_type.as_str() {
            "plonk" => {
                println!("Generating Plonk proof (on-chain verifiable)...");
                println!("This may take 10-30 minutes...");
                let proof = client.prove(&pk, stdin.clone()).plonk().await?;
                println!("Plonk proof generated successfully!");
                proof.bytes()
            }
            "groth16" | "true" | _ => {
                println!("Generating Groth16 proof (on-chain verifiable)...");
                println!("This may take 10-30 minutes...");
                let proof = client.prove(&pk, stdin.clone()).groth16().await?;
                println!("Groth16 proof generated successfully!");
                proof.bytes()
            }
        };

        // Encode public values for Solidity
        let token_address = Address::from_slice(&token_bytes);
        let recipient_address = Address::from_slice(&recipient_bytes);
        let public_values = WithdrawPublicValues {
            nullifier: nullifier.into(),
            root: expected_root.into(),
            recipient: recipient_address,
            token: token_address,
            amount: U256::from(amount),
        };

        let encoded_public_values = WithdrawPublicValues::abi_encode(&public_values);

        // Save proof artifacts
        std::fs::write("withdraw_proof.bin", &proof_bytes)?;
        std::fs::write("withdraw_public_values.bin", &encoded_public_values)?;
        std::fs::write("withdraw_vkey.txt", vk.bytes32())?;

        println!("\nProof artifacts saved:");
        println!("  - withdraw_proof.bin ({} bytes)", proof_bytes.len());
        println!("  - withdraw_public_values.bin");
        println!("  - withdraw_vkey.txt");
    }

    // ============ Save Withdrawal Data ============

    let withdrawal_data = serde_json::json!({
        "nullifier": format!("0x{}", hex::encode(nullifier)),
        "root": format!("0x{}", hex::encode(expected_root)),
        "recipient": format!("0x{}", hex::encode(recipient_bytes)),
        "token": format!("0x{}", hex::encode(token_bytes)),
        "amount": amount.to_string(),
    });

    std::fs::write(
        "withdrawal_data.json",
        serde_json::to_string_pretty(&withdrawal_data)?,
    )?;

    println!("\n=== Withdrawal Ready ===");
    println!("Withdrawal data saved to 'withdrawal_data.json'");
    println!("\nTo submit on-chain:");
    println!("  1. Generate proof with: GENERATE_PROOF=groth16 cargo run --release ...");
    println!("  2. Submit to contract with proof and public values");

    Ok(())
}
