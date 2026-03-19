pub mod health;
pub mod orders;

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
        .with_state(state)
}
