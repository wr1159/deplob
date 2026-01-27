//! Deposit Proof Generation Script
//!
//! This script:
//! 1. Generates random secrets (nullifier_note, secret)
//! 2. Computes the commitment
//! 3. Executes the SP1 program to verify correctness
//! 4. Optionally generates a real ZK proof

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolType};
use deplob_core::CommitmentPreimage;
use sp1_sdk::{HashableKey, ProverClient, SP1Stdin};
use std::env;

const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

// Solidity ABI encoding for public values
sol! {
    struct DepositPublicValues {
        bytes32 commitment;
        address token;
        uint256 amount;
    }
}

fn main() -> anyhow::Result<()> {
    // Initialize SP1 prover
    sp1_sdk::utils::setup_logger();
    let client = ProverClient::new();

    // ============ Prepare Inputs ============

    // Private inputs (user-generated secrets)
    let nullifier_note: [u8; 32] = rand::random();
    let secret: [u8; 32] = rand::random();

    // Public inputs
    let token_bytes: [u8; 20] = [0xAB; 20]; // Example token address
    let amount: u128 = 1_000_000_000_000_000_000; // 1 token (18 decimals)

    // Compute expected commitment (for verification)
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
    let expected_commitment = preimage.commitment();

    println!("=== Deposit Proof Generation ===\n");
    println!("Private Inputs:");
    println!("  nullifier_note: 0x{}", hex::encode(nullifier_note));
    println!("  secret: 0x{}", hex::encode(secret));
    println!("\nPublic Inputs:");
    println!("  token: 0x{}", hex::encode(token_bytes));
    println!("  amount: {} wei", amount);
    println!("\nExpected Commitment: 0x{}", hex::encode(expected_commitment));

    // Create stdin for SP1 program
    let mut stdin = SP1Stdin::new();
    stdin.write(&nullifier_note);
    stdin.write(&secret);
    stdin.write(&token_bytes);
    stdin.write(&amount);

    // ============ Execute (for testing) ============

    println!("\n--- Executing SP1 Program ---");
    let (mut output, report) = client.execute(ELF, stdin.clone()).run()?;
    println!(
        "Execution successful! Cycles: {}",
        report.total_instruction_count()
    );

    // Read outputs from SP1 program
    let commitment: [u8; 32] = output.read();
    let token_out: [u8; 20] = output.read();
    let amount_out: u128 = output.read();

    println!("\nSP1 Program Outputs:");
    println!("  commitment: 0x{}", hex::encode(commitment));
    println!("  token: 0x{}", hex::encode(token_out));
    println!("  amount: {}", amount_out);

    // Verify outputs match expectations
    assert_eq!(
        commitment, expected_commitment,
        "Commitment mismatch!"
    );
    assert_eq!(token_out, token_bytes, "Token mismatch!");
    assert_eq!(amount_out, amount, "Amount mismatch!");
    println!("\nAll outputs verified!");

    // ============ Generate Proof (Optional) ============
    //
    // Proof types:
    //   GENERATE_PROOF=groth16  - Groth16 proof (on-chain verifiable, requires trusted setup)
    //   GENERATE_PROOF=plonk    - Plonk proof (on-chain verifiable, no trusted setup)
    //   GENERATE_PROOF=true     - Same as groth16
    //
    // Note: On-chain verification requires the corresponding verifier contract:
    //   - Groth16: @sp1-contracts/v5.0.0/SP1VerifierGroth16.sol
    //   - Plonk: @sp1-contracts/v5.0.0/SP1VerifierPlonk.sol

    let proof_type = env::var("GENERATE_PROOF").unwrap_or_default();

    if !proof_type.is_empty() && proof_type != "false" {
        let (pk, vk) = client.setup(ELF);
        println!("\n--- Generating SP1 Proof ---");
        println!("Verification key: {}", vk.bytes32());

        let proof_bytes = match proof_type.as_str() {
            "plonk" => {
                println!("Generating Plonk proof (on-chain verifiable)...");
                println!("This may take 10-30 minutes...");
                let proof = client.prove(&pk, stdin).plonk().run()?;
                println!("Plonk proof generated successfully!");
                proof.bytes()
            }
            "groth16" | "true" | _ => {
                println!("Generating Groth16 proof (on-chain verifiable)...");
                println!("This may take 10-30 minutes...");
                let proof = client.prove(&pk, stdin).groth16().run()?;
                println!("Groth16 proof generated successfully!");
                proof.bytes()
            }
        };

        // Encode public values for Solidity
        let token_address = Address::from_slice(&token_bytes);
        let public_values = DepositPublicValues {
            commitment: commitment.into(),
            token: token_address,
            amount: U256::from(amount),
        };

        let encoded_public_values = DepositPublicValues::abi_encode(&public_values);

        // Save proof artifacts
        std::fs::write("deposit_proof.bin", &proof_bytes)?;
        std::fs::write("deposit_public_values.bin", &encoded_public_values)?;
        std::fs::write("deposit_vkey.txt", vk.bytes32())?;

        println!("\nProof artifacts saved:");
        println!("  - deposit_proof.bin ({} bytes)", proof_bytes.len());
        println!("  - deposit_public_values.bin");
        println!("  - deposit_vkey.txt");
    }

    // ============ Save User Note ============

    let user_data = serde_json::json!({
        "nullifier_note": hex::encode(nullifier_note),
        "secret": hex::encode(secret),
        "commitment": hex::encode(commitment),
        "token": hex::encode(token_bytes),
        "amount": amount.to_string(),
    });

    std::fs::write(
        "deposit_note.json",
        serde_json::to_string_pretty(&user_data)?,
    )?;

    println!("\n=== IMPORTANT ===");
    println!("Save 'deposit_note.json' securely!");
    println!("You will need it to withdraw your funds later.");
    println!("If lost, your funds will be unrecoverable!");

    Ok(())
}
