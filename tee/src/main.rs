mod attestation;
mod chain;
mod matching;
mod orderbook;
mod routes;
mod settlement;
mod state;
mod types;
mod verification;

use std::{env, sync::Arc};

use attestation::{AttestationProvider, EcdsaAttestationProvider, MockAttestationProvider};
use chain::{AlloyChainClient, ChainClient, MockChainClient};
use state::new_shared;

#[tokio::main]
async fn main() {
    // Load .env file if present (won't override existing env vars)
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "deplob_tee=info".into()),
        )
        .init();

    // If all three env vars are set, connect to the real chain.
    // Otherwise fall back to the in-memory mock (useful for local dev).
    let chain: Arc<dyn ChainClient> = match (
        env::var("ETH_RPC_URL"),
        env::var("DEPLOB_ADDRESS"),
        env::var("TEE_PRIVATE_KEY"),
    ) {
        (Ok(rpc), Ok(addr), Ok(key)) => {
            tracing::info!("Connecting to chain — RPC: {rpc}, contract: {addr}");
            let client = AlloyChainClient::from_env(&rpc, &addr, &key)
                .await
                .expect("failed to initialise AlloyChainClient");
            Arc::new(client)
        }
        _ => {
            tracing::warn!(
                "ETH_RPC_URL / DEPLOB_ADDRESS / TEE_PRIVATE_KEY not set \
                 — using MockChainClient (all on-chain checks will fail for real deposits)"
            );
            Arc::new(MockChainClient::new())
        }
    };

    // Build attestation provider:
    //   1. TEE_ATTESTATION_KEY env var → Phase A (ECDSA from env)
    //   2. /sealed/attestation.key exists or can be created → Phase B (SGX sealed key)
    //   3. Neither → mock (empty attestation bytes)
    let attestation: Arc<dyn AttestationProvider> = match env::var("TEE_ATTESTATION_KEY") {
        Ok(key) if !key.is_empty() => {
            let provider = EcdsaAttestationProvider::new(&key)
                .expect("failed to initialise EcdsaAttestationProvider");
            tracing::info!(
                "Attestation signing address (env): {}",
                provider.signing_address().unwrap()
            );
            Arc::new(provider)
        }
        _ if std::path::Path::new("/sealed").exists() => {
            let provider = EcdsaAttestationProvider::from_sealed("/sealed/attestation.key")
                .expect("failed to initialise sealed EcdsaAttestationProvider");
            tracing::info!(
                "Attestation signing address (sealed): {}",
                provider.signing_address().unwrap()
            );
            Arc::new(provider)
        }
        _ => {
            tracing::warn!(
                "TEE_ATTESTATION_KEY not set and /sealed not found — using mock attestation (empty bytes)"
            );
            Arc::new(MockAttestationProvider)
        }
    };

    let shared = new_shared(chain, attestation);
    let app = routes::router(shared);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    tracing::info!("DePLOB TEE listening on {addr}");
    axum::serve(listener, app).await.expect("server error");
}
