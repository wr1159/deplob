use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::{config::ChainArgs, indexer::MerkleIndexer};

#[derive(Args)]
pub struct OrderArgs {
    /// Path to the deposit note JSON file
    #[arg(long)]
    pub note: String,

    /// Order side: "buy" or "sell"
    #[arg(long)]
    pub side: String,

    /// Price (decimal string)
    #[arg(long)]
    pub price: String,

    /// Quantity (decimal string)
    #[arg(long)]
    pub quantity: String,

    /// Token in address (the token being offered, hex)
    #[arg(long)]
    pub token_in: String,

    /// Token out address (the token being requested, hex)
    #[arg(long)]
    pub token_out: String,

    /// TEE matching engine URL (e.g. http://localhost:3000)
    #[arg(long, env = "TEE_URL")]
    pub tee_url: String,

    #[command(flatten)]
    pub chain: ChainArgs,
}

// Request/response types matching TEE API
#[derive(Serialize)]
struct OrderRequest {
    deposit_nullifier_note: String,
    deposit_secret: String,
    deposit_token: String,
    deposit_amount: String,
    merkle_root: String,
    merkle_siblings: Vec<String>,
    merkle_path_indices: Vec<u8>,
    order: OrderJson,
}

#[derive(Serialize)]
struct OrderJson {
    price: String,
    quantity: String,
    side: String,
    token_in: String,
    token_out: String,
}

#[derive(Deserialize)]
struct OrderResponse {
    order_id: String,
    status: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

pub async fn run(args: OrderArgs) -> Result<()> {
    // Validate side
    let side = match args.side.to_lowercase().as_str() {
        "buy" => "buy",
        "sell" => "sell",
        _ => anyhow::bail!("side must be 'buy' or 'sell'"),
    };

    // Load deposit note
    let note_str = std::fs::read_to_string(&args.note)
        .with_context(|| format!("failed to read note file: {}", args.note))?;
    let note: serde_json::Value = serde_json::from_str(&note_str)?;

    let nullifier_note = note["nullifier_note"].as_str().context("missing nullifier_note")?;
    let secret = note["secret"].as_str().context("missing secret")?;
    let token = note["token"].as_str().context("missing token")?;
    let amount = note["amount"].as_str().context("missing amount")?;
    let commitment_hex = note["commitment"].as_str().context("missing commitment")?;

    // Parse commitment for Merkle proof lookup
    let commitment = parse_hex32(commitment_hex)?;

    // Sync Merkle tree and get proof
    println!("Syncing Merkle tree from on-chain events...");
    let indexer = MerkleIndexer::sync(&args.chain.rpc_url, &args.chain.contract).await?;
    let (siblings, path_indices, root, _leaf_index) = indexer.proof_for(&commitment)?;

    println!("Merkle root: 0x{}", hex::encode(root));

    // Build request
    let req = OrderRequest {
        deposit_nullifier_note: nullifier_note.to_string(),
        deposit_secret: secret.to_string(),
        deposit_token: token.to_string(),
        deposit_amount: amount.to_string(),
        merkle_root: format!("0x{}", hex::encode(root)),
        merkle_siblings: siblings.iter().map(|s| format!("0x{}", hex::encode(s))).collect(),
        merkle_path_indices: path_indices,
        order: OrderJson {
            price: args.price.clone(),
            quantity: args.quantity.clone(),
            side: side.to_string(),
            token_in: args.token_in.clone(),
            token_out: args.token_out.clone(),
        },
    };

    // POST to TEE
    let url = format!("{}/v1/orders", args.tee_url.trim_end_matches('/'));
    println!("Submitting order to TEE at {}...", url);

    let client = reqwest::Client::new();
    let resp = client.post(&url).json(&req).send().await.context("failed to reach TEE")?;

    if resp.status().is_success() {
        let order_resp: OrderResponse = resp.json().await?;
        println!("Order accepted!");
        println!("  order_id: {}", order_resp.order_id);
        println!("  status:   {}", order_resp.status);

        // Save order info for later cancel/settlement
        let order_info = serde_json::json!({
            "order_id": order_resp.order_id,
            "status": order_resp.status,
            "note_file": args.note,
        });
        let order_file = format!("order_{}.json", &order_resp.order_id[2..10]);
        std::fs::write(&order_file, serde_json::to_string_pretty(&order_info)?)?;
        println!("Order info saved to {}", order_file);
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
