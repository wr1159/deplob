# 08 — Order Matching System (TEE)

## Overview

The TEE-based matching engine runs as an HTTP server. Users submit orders via the
REST API (see doc 06). The engine maintains a private in-memory order book, matches
orders with price-time priority, and submits `settleMatch()` to the contract when
a trade occurs. The event-listener model (listening for on-chain `OrderCreated` events)
has been replaced by direct HTTP ingestion.

---

## 8.1 TEE Architecture

```
User
  │
  └── POST /v1/orders  ──────────────────────────────────┐
                                                          │
┌─────────────────────────────────────────────────────── ▼ ──────┐
│                        TEE Enclave                              │
│                                                                 │
│  ┌─────────────────┐   ┌──────────────┐   ┌───────────────┐    │
│  │   HTTP Server   │   │  Order Book  │   │    Matcher    │    │
│  │  (axum 0.7)     │   │              │   │               │    │
│  │  POST /v1/orders│──►│  - Bids      │──►│  - Price-time │    │
│  │  DELETE /v1/...  │   │  - Asks      │   │  - Partial    │    │
│  │  GET /v1/health │   │  - Index     │   │    fills      │    │
│  └─────────────────┘   └──────────────┘   └───────────────┘    │
│           │                                       │             │
│           │  (verification)                       │ (trades)    │
│           ▼                                       ▼             │
│  ┌─────────────────┐   ┌──────────────────────────────────┐    │
│  │  Verification   │   │         Settlement Engine        │    │
│  │                 │   │                                  │    │
│  │  - Merkle proof │   │  - Generate new commitments      │    │
│  │  - Chain checks │   │  - chain.settle_match()          │    │
│  └─────────────────┘   └──────────────────────────────────┘    │
│                                        │                        │
└────────────────────────────────────────┼────────────────────────┘
                                         │
                                         ▼
                                ┌────────────────┐
                                │ Smart Contract │
                                │  settleMatch() │
                                └────────────────┘
```

---

## 8.2 Cargo.toml

`tee/Cargo.toml`:

```toml
[package]
name = "deplob-tee"
version.workspace = true
edition.workspace = true

[[bin]]
name = "deplob-tee"
path = "src/main.rs"

[dependencies]
deplob-core = { path = "../sp1-programs/lib/deplob-core" }
tokio = { workspace = true }
serde = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
rand = { workspace = true }
axum = "0.7"
tower-http = { version = "0.5", features = ["trace", "cors"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
hex = { workspace = true }
```

---

## 8.3 Order Book

`tee/src/orderbook/mod.rs` — price-time priority using `BTreeMap<u128, PriceLevel>`.

```
bids: BTreeMap<u128, PriceLevel>   iterate in reverse → best bid = highest price
asks: BTreeMap<u128, PriceLevel>   iterate forward   → best ask = lowest price
index: HashMap<[u8;32], (OrderSide, u128)>  order_id → (side, price) for O(log n) removal
```

Key methods:

| Method | Description |
|--------|-------------|
| `add_order(entry)` | Insert into bids or asks, update index |
| `pop_best_bid()` | Remove and return highest-price oldest-time order |
| `pop_best_ask()` | Remove and return lowest-price oldest-time order |
| `has_crossing()` | `best_bid >= best_ask` |
| `remove_order(&id)` | Cancel by `order_id` |

---

## 8.4 Matching Engine

`tee/src/matching/mod.rs`

```rust
/// Add an order, then run the matching loop.
/// Returns all trades generated (empty if no crossing).
pub fn add_and_match(book: &mut OrderBook, new_entry: OrderEntry) -> Vec<Trade>

/// Run matching until no more crossings.
pub fn run_matching(book: &mut OrderBook) -> Vec<Trade>
```

Algorithm:
1. Insert the new order into the book
2. While `has_crossing()`:
   - `execution_quantity = min(best_bid.quantity, best_ask.quantity)`
   - `execution_price = best_ask.price` (taker buys at ask)
   - Pop both orders, emit `Trade`
   - Reinsert partially filled remainders
3. Return trades

---

## 8.5 Settlement Generation

`tee/src/settlement/mod.rs`

```rust
pub fn generate_settlement(trade: &Trade, rng: &mut impl RngCore) -> SettlementData
```

For each trade, the TEE:
1. Generates fresh `nullifier_note` and `secret` for buyer and seller
2. Computes new commitments: `CommitmentPreimage::new(nullifier_note, secret, token, amount)`
3. Builds `SettlementData` containing old nullifiers + new commitments
4. Calls `chain.settle_match(&data)` → on-chain `settleMatch()`

Buyer receives `token_out` in amount `execution_quantity`.
Seller receives `token_out` (their token_out) in amount `execution_quantity * execution_price`.

The new `CommitmentPreimage` for each party is included in `SettlementData` and
must be securely delivered back to users (TODO: encrypt under user's public key).

---

## 8.6 Chain Client

`tee/src/chain.rs` — async trait abstraction over on-chain queries and settlement submission.

```rust
#[async_trait]
pub trait ChainClient: Send + Sync {
    async fn is_commitment_known(&self, commitment: [u8; 32]) -> anyhow::Result<bool>;
    async fn is_nullifier_spent(&self, nullifier: [u8; 32]) -> anyhow::Result<bool>;
    async fn is_known_root(&self, root: [u8; 32]) -> anyhow::Result<bool>;
    async fn settle_match(&self, data: &SettlementData) -> anyhow::Result<()>;
}
```

### MockChainClient

`MockChainClient` is used for tests and local development. It stores `known_commitments`,
`spent_nullifiers`, and `known_roots` in in-memory `HashSet`s. With the default mock
(empty sets), all order submissions are rejected with "commitment not found on-chain".

### AlloyChainClient (Sepolia)

`AlloyChainClient` makes real `eth_call` / `eth_sendTransaction` calls to the deployed
DePLOB contract using the [alloy](https://github.com/alloy-rs/alloy) provider.

Contract bindings are generated via the `sol!` macro for four functions:
`commitments()`, `nullifierHashes()`, `isKnownRoot()`, and `settleMatch()`.

For read-only queries (the three checks), a plain HTTP provider is created per call.
For `settleMatch`, a provider with `EthereumWallet` filler is used so the transaction
is signed by the TEE operator key.

**Environment variables** (all three required to activate `AlloyChainClient`):

| Variable | Example | Purpose |
| --- | --- | --- |
| `ETH_RPC_URL` | `https://sepolia.infura.io/v3/<key>` | Sepolia JSON-RPC endpoint |
| `DEPLOB_ADDRESS` | `0x1234...abcd` | Deployed DePLOB contract address |
| `TEE_PRIVATE_KEY` | `0xabcd...1234` | Private key of the TEE operator wallet |

The TEE operator address (derived from `TEE_PRIVATE_KEY`) must match the `teeOperator`
set in the DePLOB contract constructor.

If any of the three env vars is missing, the server falls back to `MockChainClient`
with a warning log.

---

## 8.7 Main TEE Application

`tee/src/main.rs` — axum HTTP server with shared state:

```rust
#[tokio::main]
async fn main() {
    let chain: Arc<dyn ChainClient> = match (
        env::var("ETH_RPC_URL"),
        env::var("DEPLOB_ADDRESS"),
        env::var("TEE_PRIVATE_KEY"),
    ) {
        (Ok(rpc), Ok(addr), Ok(key)) => {
            Arc::new(AlloyChainClient::from_env(&rpc, &addr, &key).await.unwrap())
        }
        _ => Arc::new(MockChainClient::new()),
    };

    let shared = new_shared(chain);          // Arc<RwLock<TeeState>>
    let app = routes::router(shared);        // registers /v1/orders, /v1/health

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

All handlers share `SharedState = Arc<RwLock<TeeState>>`. On-chain checks are performed
outside the write lock (using a cloned `Arc<dyn ChainClient>`) to avoid blocking other
requests during RPC calls. The write lock is acquired only for state mutation (deposit
locking, order insertion, matching, and settlement bookkeeping).

---

## 8.8 State Structure

`tee/src/state.rs`:

```rust
pub struct TeeState {
    pub book: OrderBook,
    /// deposit_nullifier → order_id (double-spend protection)
    pub locked_deposits: HashMap<[u8; 32], [u8; 32]>,
    /// order_id → deposit_nullifier (cancellation lookup)
    pub order_to_deposit: HashMap<[u8; 32], [u8; 32]>,
    /// order_id → OrderEntry (full order details)
    pub order_details: HashMap<[u8; 32], OrderEntry>,
    pub chain: Arc<dyn ChainClient>,
}
```

---

## 8.9 Checklist

- [ ] Order book: price-time priority correct (highest bid, lowest ask)
- [ ] Matching: exact fill, partial fill (bid larger), partial fill (ask larger)
- [ ] Matching: price priority — lowest ask matched first
- [ ] Settlement: new commitments generated with random secrets
- [ ] Settlement: `chain.settle_match()` called for each trade
- [ ] After settlement, `locked_deposits` entries are cleared
- [ ] Cancellation removes order from book and frees deposit
- [ ] HTTP server starts and accepts connections on port 3000
