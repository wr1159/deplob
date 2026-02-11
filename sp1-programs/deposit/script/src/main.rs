//! Deposit Note Generator
//!
//! Generates a deposit note (commitment + secrets) for the DePLOB shielded pool.
//! No ZK proof is required for deposit — the user just submits the commitment
//! along with their tokens and the contract inserts it into the Merkle tree.
//!
//! Usage:
//!   cargo run --release --bin deposit-script -- [token_address] [amount_wei]

use deplob_core::CommitmentPreimage;
use std::env;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    let token_hex = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("abababababababababababababababababababab");
    let amount: u128 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000_000_000_000_000);

    let token_bytes: [u8; 20] = hex::decode(token_hex.trim_start_matches("0x"))
        .expect("Invalid token hex")
        .try_into()
        .expect("Token must be 20 bytes");

    // Generate random secrets
    let nullifier_note: [u8; 32] = rand::random();
    let secret: [u8; 32] = rand::random();

    // Compute commitment and nullifier
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
    let commitment = preimage.commitment();
    let nullifier = preimage.nullifier();

    println!("=== DePLOB Deposit Note ===\n");
    println!("Commitment (submit to contract):");
    println!("  0x{}", hex::encode(commitment));
    println!("\nPublic info:");
    println!("  token:  0x{}", hex::encode(token_bytes));
    println!("  amount: {} wei", amount);
    println!("\n=== KEEP SECRET — needed for withdrawal ===");
    println!("  nullifier_note: 0x{}", hex::encode(nullifier_note));
    println!("  secret:         0x{}", hex::encode(secret));
    println!("  nullifier:      0x{}", hex::encode(nullifier));

    // Save the deposit note
    let note = serde_json::json!({
        "commitment": format!("0x{}", hex::encode(commitment)),
        "nullifier_note": format!("0x{}", hex::encode(nullifier_note)),
        "secret": format!("0x{}", hex::encode(secret)),
        "nullifier": format!("0x{}", hex::encode(nullifier)),
        "token": format!("0x{}", hex::encode(token_bytes)),
        "amount": amount.to_string(),
    });

    std::fs::write("deposit_note.json", serde_json::to_string_pretty(&note)?)?;

    println!("\nSaved to deposit_note.json");
    println!("\n*** WARNING: Keep deposit_note.json secret and secure. ***");
    println!("*** If lost, your funds will be unrecoverable!          ***");

    Ok(())
}
