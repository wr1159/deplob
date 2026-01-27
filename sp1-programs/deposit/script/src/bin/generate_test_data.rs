//! Generate test data for Foundry E2E tests
//!
//! Outputs JSON to stdout that can be parsed by Foundry's FFI
//!
//! Usage:
//!   cargo run --release --bin generate_test_data -- <token_address> <amount>

use deplob_core::CommitmentPreimage;
use sp1_sdk::{HashableKey, ProverClient, SP1Stdin};
use std::env;

const ELF: &[u8] = include_bytes!("../../../program/elf/riscv32im-succinct-zkvm-elf");

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Parse arguments or use defaults
    let token_hex = args.get(1).map(|s| s.as_str()).unwrap_or("abababababababababababababababababababab");
    let amount: u128 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000_000_000_000_000);

    // Parse token address
    let token_bytes: [u8; 20] = hex::decode(token_hex.trim_start_matches("0x"))
        .expect("Invalid token hex")
        .try_into()
        .expect("Token must be 20 bytes");

    // Generate random secrets
    let nullifier_note: [u8; 32] = rand::random();
    let secret: [u8; 32] = rand::random();

    // Compute commitment
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
    let commitment = preimage.commitment();

    // Initialize SP1 and execute to verify
    let client = ProverClient::new();

    let mut stdin = SP1Stdin::new();
    stdin.write(&nullifier_note);
    stdin.write(&secret);
    stdin.write(&token_bytes);
    stdin.write(&amount);

    // Execute to verify correctness
    let (mut output, _report) = client.execute(ELF, stdin.clone()).run()?;

    let computed_commitment: [u8; 32] = output.read();
    assert_eq!(commitment, computed_commitment, "Commitment mismatch");

    // Get verification key
    let (_pk, vk) = client.setup(ELF);

    // Output JSON for Foundry FFI
    let output = serde_json::json!({
        "commitment": format!("0x{}", hex::encode(commitment)),
        "token": format!("0x{}", hex::encode(token_bytes)),
        "amount": amount.to_string(),
        "nullifier_note": format!("0x{}", hex::encode(nullifier_note)),
        "secret": format!("0x{}", hex::encode(secret)),
        "vkey": vk.bytes32(),
        "proof": "0x"  // Empty proof for SP1MockVerifier
    });

    // Print to stdout for FFI capture
    println!("{}", serde_json::to_string(&output)?);

    Ok(())
}
