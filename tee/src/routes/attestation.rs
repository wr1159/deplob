use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize)]
struct QuoteResponse {
    quote: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// GET /v1/attestation/quote
///
/// Reads the DCAP quote from Gramine's pseudo-filesystem and returns it
/// hex-encoded. The quote embeds the enclave's signing address in REPORTDATA
/// and can be submitted to `registerEnclave()` on-chain.
///
/// Returns 503 if not running inside a Gramine SGX enclave.
pub async fn get_quote() -> impl IntoResponse {
    let quote_path = std::path::Path::new("/dev/attestation/quote");
    match std::fs::read(quote_path) {
        Ok(bytes) => (
            StatusCode::OK,
            Json(QuoteResponse {
                quote: format!("0x{}", hex::encode(&bytes)),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!("Failed to read attestation quote: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Attestation quote not available (not running in SGX enclave)"
                        .to_string(),
                }),
            )
                .into_response()
        }
    }
}
