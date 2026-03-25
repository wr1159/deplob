pub mod attestation;
pub mod health;
pub mod orders;
pub mod settlements;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::state::SharedState;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/v1/health", get(health::health))
        .route("/v1/orders", post(orders::submit_order))
        .route("/v1/orders/:order_id", delete(orders::cancel_order))
        .route(
            "/v1/settlements/:deposit_nullifier",
            get(settlements::get_settlement),
        )
        .route("/v1/attestation/quote", get(attestation::get_quote))
        .with_state(state)
}
