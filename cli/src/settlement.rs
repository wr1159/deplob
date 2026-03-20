use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;

#[derive(Args)]
pub struct SettlementArgs {
    /// Path to the original deposit note JSON file
    #[arg(long)]
    pub note: String,

    /// TEE matching engine URL (e.g. http://localhost:3000)
    #[arg(long, env = "TEE_URL")]
    pub tee_url: String,

    /// Output file for the new deposit note
    #[arg(long, default_value = "new_deposit_note.json")]
    pub save: String,
}

#[derive(Deserialize)]
struct SettlementResponse {
    status: String,
    new_deposit_note: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

pub async fn run(args: SettlementArgs) -> Result<()> {
    // Load deposit note to get nullifier_note
    let note_str = std::fs::read_to_string(&args.note)
        .with_context(|| format!("failed to read note file: {}", args.note))?;
    let note: serde_json::Value = serde_json::from_str(&note_str)?;
    let nullifier_note = note["nullifier_note"]
        .as_str()
        .context("missing nullifier_note in deposit note")?;

    // Derive deposit_nullifier = keccak256(nullifier_note)
    let nn_bytes = parse_hex32(nullifier_note)?;
    let deposit_nullifier = deplob_core::keccak256_concat(&[nn_bytes]);
    let deposit_nullifier_hex = format!("0x{}", hex::encode(deposit_nullifier));

    let url = format!(
        "{}/v1/settlements/{}?nullifier_note={}",
        args.tee_url.trim_end_matches('/'),
        deposit_nullifier_hex,
        nullifier_note
    );

    println!("Checking settlement for deposit {}...", deposit_nullifier_hex);

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .context("failed to reach TEE")?;

    if resp.status().is_success() {
        let settlement: SettlementResponse = resp.json().await?;
        println!("Status: {}", settlement.status);

        if let Some(new_note) = settlement.new_deposit_note {
            std::fs::write(&args.save, serde_json::to_string_pretty(&new_note)?)?;
            println!("New deposit note saved to {}", args.save);
            println!("\n*** Keep this file secret — needed for withdrawal ***");
        } else {
            println!("No new deposit note available (order may not have been matched yet).");
        }
    } else {
        let status = resp.status();
        let err_resp: ErrorResponse = resp.json().await.unwrap_or(ErrorResponse {
            error: "unknown error".to_string(),
        });
        anyhow::bail!("TEE returned {}: {}", status, err_resp.error);
    }

    Ok(())
}

fn parse_hex32(s: &str) -> Result<[u8; 32]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("invalid hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 32 bytes"))
}
