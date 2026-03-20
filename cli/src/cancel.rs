use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

#[derive(Args)]
pub struct CancelArgs {
    /// Order ID to cancel (hex)
    #[arg(long)]
    pub order_id: String,

    /// Path to the deposit note JSON file (proves ownership)
    #[arg(long)]
    pub note: String,

    /// TEE matching engine URL (e.g. http://localhost:3000)
    #[arg(long, env = "TEE_URL")]
    pub tee_url: String,
}

#[derive(Serialize)]
struct CancelRequest {
    deposit_nullifier_note: String,
}

#[derive(Deserialize)]
struct CancelResponse {
    order_id: String,
    status: String,
    deposit_nullifier: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

pub async fn run(args: CancelArgs) -> Result<()> {
    // Load deposit note to get nullifier_note
    let note_str = std::fs::read_to_string(&args.note)
        .with_context(|| format!("failed to read note file: {}", args.note))?;
    let note: serde_json::Value = serde_json::from_str(&note_str)?;
    let nullifier_note = note["nullifier_note"]
        .as_str()
        .context("missing nullifier_note in deposit note")?;

    let order_id = args.order_id.strip_prefix("0x").unwrap_or(&args.order_id);
    let url = format!(
        "{}/v1/orders/0x{}",
        args.tee_url.trim_end_matches('/'),
        order_id
    );

    println!("Cancelling order 0x{}...", order_id);

    let req = CancelRequest {
        deposit_nullifier_note: nullifier_note.to_string(),
    };

    let client = reqwest::Client::new();
    let resp = client
        .delete(&url)
        .json(&req)
        .send()
        .await
        .context("failed to reach TEE")?;

    if resp.status().is_success() {
        let cancel_resp: CancelResponse = resp.json().await?;
        println!("Order cancelled!");
        println!("  order_id:          {}", cancel_resp.order_id);
        println!("  status:            {}", cancel_resp.status);
        println!("  deposit_nullifier: {}", cancel_resp.deposit_nullifier);
        println!("\nDeposit is now unlocked — you can withdraw or create a new order.");
    } else {
        let status = resp.status();
        let err_resp: ErrorResponse = resp.json().await.unwrap_or(ErrorResponse {
            error: "unknown error".to_string(),
        });
        anyhow::bail!("TEE returned {}: {}", status, err_resp.error);
    }

    Ok(())
}
