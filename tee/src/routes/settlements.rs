use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::SharedState;

#[derive(Deserialize)]
pub struct SettlementQuery {
    /// Hex-encoded 32-byte nullifier_note (proves ownership of the deposit)
    pub nullifier_note: String,
}

#[derive(Serialize)]
pub struct SettlementResponse {
    pub status: String,
    pub new_deposit_note: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn err(code: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (code, Json(ErrorResponse { error: msg.into() }))
}

fn parse_hex32(s: &str) -> Result<[u8; 32], String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|e| e.to_string())?;
    bytes
        .try_into()
        .map_err(|_| "expected 32 bytes".to_string())
}

/// GET /v1/settlements/:deposit_nullifier?nullifier_note=0x...
///
/// Retrieves the new deposit note for a user after their order was matched.
/// The user proves ownership by providing the nullifier_note preimage,
/// which the TEE hashes to verify it matches the deposit_nullifier path param.
pub async fn get_settlement(
    State(shared): State<SharedState>,
    Path(deposit_nullifier_hex): Path<String>,
    Query(query): Query<SettlementQuery>,
) -> impl IntoResponse {
    let deposit_nullifier = match parse_hex32(&deposit_nullifier_hex) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("deposit_nullifier: {e}")).into_response(),
    };
    let nullifier_note = match parse_hex32(&query.nullifier_note) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("nullifier_note: {e}")).into_response(),
    };

    // Verify ownership: hash(nullifier_note) must equal the path param
    let derived = deplob_core::keccak256_concat(&[nullifier_note]);
    if derived != deposit_nullifier {
        return err(StatusCode::FORBIDDEN, "nullifier_note does not match deposit_nullifier").into_response();
    }

    let state = shared.read().await;

    match state.settlements.get(&deposit_nullifier) {
        Some(settlement) => {
            let note_json = serde_json::to_value(settlement).unwrap();
            (
                StatusCode::OK,
                Json(SettlementResponse {
                    status: "settled".to_string(),
                    new_deposit_note: Some(note_json),
                }),
            )
                .into_response()
        }
        None => {
            (
                StatusCode::OK,
                Json(SettlementResponse {
                    status: "pending".to_string(),
                    new_deposit_note: None,
                }),
            )
                .into_response()
        }
    }
}
