use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use deplob_core::keccak256_concat;

use crate::{
    matching::add_and_match,
    settlement::generate_settlement,
    state::SharedState,
    types::{Order, OrderEntry, OrderSide},
    verification::{verify_deposit_covers_order, verify_deposit_ownership},
};

// ============ Request / Response types ============

#[derive(Deserialize)]
pub struct OrderRequest {
    /// Hex-encoded 32-byte deposit nullifier_note
    pub deposit_nullifier_note: String,
    /// Hex-encoded 32-byte deposit secret
    pub deposit_secret: String,
    /// Hex-encoded 20-byte deposit token address
    pub deposit_token: String,
    /// Deposit amount (string to avoid JS number precision issues)
    pub deposit_amount: String,
    /// Hex-encoded 32-byte Merkle root
    pub merkle_root: String,
    /// 20 hex-encoded 32-byte sibling hashes
    pub merkle_siblings: Vec<String>,
    /// 20 path indices (0 = left child, 1 = right child)
    pub merkle_path_indices: Vec<u8>,
    pub order: OrderJson,
}

#[derive(Deserialize)]
pub struct OrderJson {
    /// Price as decimal string
    pub price: String,
    /// Quantity as decimal string
    pub quantity: String,
    pub side: OrderSide,
    /// Hex-encoded 20-byte address
    pub token_in: String,
    /// Hex-encoded 20-byte address
    pub token_out: String,
}

#[derive(Serialize)]
pub struct OrderResponse {
    /// Hex-encoded order_id (= deposit_nullifier for this design)
    pub order_id: String,
    pub status: &'static str,
}

#[derive(Deserialize)]
pub struct CancelRequest {
    /// Hex-encoded 32-byte deposit nullifier_note (proves ownership)
    pub deposit_nullifier_note: String,
}

#[derive(Serialize)]
pub struct CancelResponse {
    pub order_id: String,
    pub status: &'static str,
    pub deposit_nullifier: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn err(code: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (code, Json(ErrorResponse { error: msg.into() }))
}

// ============ Helpers ============

fn parse_hex32(s: &str) -> Result<[u8; 32], String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|e| e.to_string())?;
    bytes
        .try_into()
        .map_err(|_| "expected 32 bytes".to_string())
}

fn parse_hex20(s: &str) -> Result<[u8; 20], String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|e| e.to_string())?;
    bytes
        .try_into()
        .map_err(|_| "expected 20 bytes".to_string())
}

fn to_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ============ Handlers ============

/// POST /v1/orders
///
/// User submits their deposit secrets and order parameters directly to the TEE.
/// The TEE verifies deposit ownership, locks the deposit, and adds the order to
/// the matching engine. If a match occurs, settlement is triggered immediately.
pub async fn submit_order(
    State(shared): State<SharedState>,
    Json(req): Json<OrderRequest>,
) -> impl IntoResponse {
    // --- Parse inputs ---
    let nullifier_note = match parse_hex32(&req.deposit_nullifier_note) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("deposit_nullifier_note: {e}")).into_response(),
    };
    let secret = match parse_hex32(&req.deposit_secret) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("deposit_secret: {e}")).into_response(),
    };
    let token = match parse_hex20(&req.deposit_token) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("deposit_token: {e}")).into_response(),
    };
    let amount: u128 = match req.deposit_amount.parse() {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("deposit_amount: {e}")).into_response(),
    };
    let merkle_root = match parse_hex32(&req.merkle_root) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("merkle_root: {e}")).into_response(),
    };
    let siblings: Result<Vec<[u8; 32]>, _> = req.merkle_siblings.iter().map(|s| parse_hex32(s)).collect();
    let siblings = match siblings {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("merkle_siblings: {e}")).into_response(),
    };
    let price: u128 = match req.order.price.parse() {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("order.price: {e}")).into_response(),
    };
    let quantity: u128 = match req.order.quantity.parse() {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("order.quantity: {e}")).into_response(),
    };
    let token_in = match parse_hex20(&req.order.token_in) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("order.token_in: {e}")).into_response(),
    };
    let token_out = match parse_hex20(&req.order.token_out) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("order.token_out: {e}")).into_response(),
    };

    if price == 0 || quantity == 0 {
        return err(StatusCode::BAD_REQUEST, "price and quantity must be > 0").into_response();
    }

    // --- Verify deposit ownership via Merkle proof ---
    let (commitment, deposit_nullifier) = match verify_deposit_ownership(
        nullifier_note,
        secret,
        token,
        amount,
        merkle_root,
        &siblings,
        &req.merkle_path_indices,
    ) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    // --- Verify deposit covers the order ---
    if let Err(e) = verify_deposit_covers_order(token, amount, req.order.side, token_in, quantity, price) {
        return err(StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // --- On-chain checks (no write lock held — avoids blocking other requests) ---
    let chain = shared.read().await.chain.clone();

    // Check on-chain: commitment must exist
    match chain.is_commitment_known(commitment).await {
        Ok(true) => {}
        Ok(false) => return err(StatusCode::BAD_REQUEST, "commitment not found on-chain").into_response(),
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    // Check on-chain: nullifier must not be spent
    match chain.is_nullifier_spent(deposit_nullifier).await {
        Ok(false) => {}
        Ok(true) => return err(StatusCode::BAD_REQUEST, "deposit nullifier already spent").into_response(),
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    // Check on-chain: Merkle root must be known
    match chain.is_known_root(merkle_root).await {
        Ok(true) => {}
        Ok(false) => return err(StatusCode::BAD_REQUEST, "unknown Merkle root").into_response(),
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    // --- Acquire write lock for state mutation ---
    let mut state = shared.write().await;

    // Check deposit not already locked in another order (re-check under write lock)
    if state.locked_deposits.contains_key(&deposit_nullifier) {
        return err(StatusCode::CONFLICT, "deposit already used for an open order").into_response();
    }

    // --- Compute order_id ---
    // order_id = keccak256(deposit_nullifier || price_le16 || quantity_le16 || side_byte || token_in || token_out)
    let mut price_bytes = [0u8; 32];
    let mut qty_bytes = [0u8; 32];
    price_bytes[16..].copy_from_slice(&price.to_be_bytes());
    qty_bytes[16..].copy_from_slice(&quantity.to_be_bytes());
    let side_byte = [match req.order.side { OrderSide::Buy => 0u8, OrderSide::Sell => 1u8 }; 32];
    let mut token_in_padded = [0u8; 32];
    let mut token_out_padded = [0u8; 32];
    token_in_padded[12..].copy_from_slice(&token_in);
    token_out_padded[12..].copy_from_slice(&token_out);

    let order_id = keccak256_concat(&[
        deposit_nullifier,
        price_bytes,
        qty_bytes,
        side_byte,
        token_in_padded,
        token_out_padded,
    ]);

    let entry = OrderEntry {
        order_id,
        deposit_nullifier,
        order: Order {
            price,
            quantity,
            side: req.order.side,
            token_in,
            token_out,
        },
        timestamp: now_secs(),
    };

    // --- Lock deposit and register order ---
    state.locked_deposits.insert(deposit_nullifier, order_id);
    state.order_to_deposit.insert(order_id, deposit_nullifier);
    state.order_details.insert(order_id, entry.clone());

    // --- Run matching ---
    let trades = add_and_match(&mut state.book, entry);

    // --- Collect settlements and release write lock ---
    let settlements = {
        let mut rng = thread_rng();
        let mut out = Vec::new();
        for trade in &trades {
            // Remove settled orders from tracking maps
            state.locked_deposits.remove(&trade.buy_entry.deposit_nullifier);
            state.locked_deposits.remove(&trade.sell_entry.deposit_nullifier);
            state.order_to_deposit.remove(&trade.buy_entry.order_id);
            state.order_to_deposit.remove(&trade.sell_entry.order_id);
            state.order_details.remove(&trade.buy_entry.order_id);
            state.order_details.remove(&trade.sell_entry.order_id);

            out.push(generate_settlement(trade, &mut rng));
        }
        out
    }; // rng dropped here

    drop(state);

    // --- Submit settlements to chain (outside write lock) ---
    for settlement in &settlements {
        if let Err(e) = chain.settle_match(settlement).await {
            tracing::error!("settle_match failed: {e}");
        }
    }

    (
        StatusCode::OK,
        Json(OrderResponse {
            order_id: to_hex(&order_id),
            status: "accepted",
        }),
    )
        .into_response()
}

/// DELETE /v1/orders/:order_id
///
/// Cancel an open order. The user proves ownership by providing their
/// deposit_nullifier_note, from which the TEE derives the deposit_nullifier
/// and looks up the associated order.
pub async fn cancel_order(
    State(shared): State<SharedState>,
    Path(order_id_hex): Path<String>,
    Json(req): Json<CancelRequest>,
) -> impl IntoResponse {
    let order_id = match parse_hex32(&order_id_hex) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("order_id: {e}")).into_response(),
    };
    let nullifier_note = match parse_hex32(&req.deposit_nullifier_note) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("deposit_nullifier_note: {e}")).into_response(),
    };

    // Derive deposit_nullifier from nullifier_note
    let deposit_nullifier = deplob_core::keccak256_concat(&[nullifier_note]);

    let mut state = shared.write().await;

    // Verify the deposit_nullifier owns this order_id
    match state.locked_deposits.get(&deposit_nullifier) {
        Some(&stored_order_id) if stored_order_id == order_id => {}
        Some(_) => return err(StatusCode::FORBIDDEN, "deposit_nullifier_note does not match order_id").into_response(),
        None => return err(StatusCode::NOT_FOUND, "no open order found for this deposit").into_response(),
    }

    // Remove from order book
    state.book.remove_order(&order_id);

    // Clean up tracking maps
    state.locked_deposits.remove(&deposit_nullifier);
    state.order_to_deposit.remove(&order_id);
    state.order_details.remove(&order_id);

    drop(state);

    (
        StatusCode::OK,
        Json(CancelResponse {
            order_id: to_hex(&order_id),
            status: "cancelled",
            deposit_nullifier: to_hex(&deposit_nullifier),
        }),
    )
        .into_response()
}
