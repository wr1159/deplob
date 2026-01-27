# Step 8: Order Matching System (TEE)

## Overview

The TEE-based matching engine:

1. Receives encrypted orders from contract events
2. Decrypts and validates orders
3. Maintains private order book
4. Matches orders using price-time priority
5. Generates settlement proofs
6. Submits settlements to the contract

## 8.1 TEE Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     TEE Enclave                              │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   Decryptor  │  │  Order Book  │  │   Matcher    │       │
│  │              │  │              │  │              │       │
│  │  - AES-GCM   │  │  - Bids      │  │  - Price-    │       │
│  │  - Validate  │  │  - Asks      │  │    Time      │       │
│  │              │  │  - Index     │  │  - Partial   │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│         │                 │                 │                │
│         v                 v                 v                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                   Settlement Engine                   │   │
│  │                                                       │   │
│  │  - Generate new commitments                          │   │
│  │  - Create settlement proof                           │   │
│  │  - Sign with TEE attestation                         │   │
│  └──────────────────────────────────────────────────────┘   │
│                            │                                 │
└────────────────────────────┼─────────────────────────────────┘
                             │
                             v
                    ┌────────────────┐
                    │ Smart Contract │
                    │  settleMatch() │
                    └────────────────┘
```

## 8.2 Project Setup

`tee/Cargo.toml`:

```toml
[package]
name = "deplob-tee"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core
deplob-core = { path = "../sp1-programs/lib/deplob-core" }
tokio = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }

# Ethereum
alloy-primitives = { workspace = true }
alloy-sol-types = { workspace = true }
alloy-provider = "0.3"
alloy-contract = "0.3"
alloy-rpc-types = "0.3"

# Crypto
aes-gcm = { workspace = true }
rand = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = "1.0"
bincode = { workspace = true }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# TEE (Intel SGX via Gramine)
# For production, add SGX-specific dependencies
```

## 8.3 Order Book Implementation

`tee/src/orderbook/mod.rs`:

```rust
//! Order Book implementation with price-time priority

use deplob_core::order::{Order, OrderSide};
use std::collections::BTreeMap;
use std::cmp::Ordering;

/// Order with metadata
#[derive(Debug, Clone)]
pub struct OrderEntry {
    pub order: Order,
    pub commitment: [u8; 32],
    pub deposit_nullifier: [u8; 32],
    pub timestamp: u64,
    pub remaining_quantity: u128,
}

/// Price level with orders at that price
#[derive(Debug, Default)]
pub struct PriceLevel {
    pub orders: Vec<OrderEntry>,
}

/// Order book for a single trading pair
#[derive(Debug)]
pub struct OrderBook {
    /// Token pair (base/quote)
    pub base_token: [u8; 20],
    pub quote_token: [u8; 20],

    /// Bids: price -> orders (sorted descending by price)
    pub bids: BTreeMap<u128, PriceLevel>,

    /// Asks: price -> orders (sorted ascending by price)
    pub asks: BTreeMap<u128, PriceLevel>,

    /// Index: commitment -> (side, price)
    pub index: std::collections::HashMap<[u8; 32], (OrderSide, u128)>,
}

impl OrderBook {
    pub fn new(base_token: [u8; 20], quote_token: [u8; 20]) -> Self {
        Self {
            base_token,
            quote_token,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            index: std::collections::HashMap::new(),
        }
    }

    /// Add an order to the book
    pub fn add_order(&mut self, entry: OrderEntry) {
        let price = entry.order.price;
        let commitment = entry.commitment;
        let side = entry.order.side;

        // Add to index
        self.index.insert(commitment, (side, price));

        // Add to appropriate side
        match side {
            OrderSide::Buy => {
                self.bids
                    .entry(price)
                    .or_default()
                    .orders
                    .push(entry);
            }
            OrderSide::Sell => {
                self.asks
                    .entry(price)
                    .or_default()
                    .orders
                    .push(entry);
            }
        }
    }

    /// Remove an order by commitment
    pub fn remove_order(&mut self, commitment: &[u8; 32]) -> Option<OrderEntry> {
        let (side, price) = self.index.remove(commitment)?;

        let book = match side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };

        if let Some(level) = book.get_mut(&price) {
            if let Some(pos) = level.orders.iter().position(|o| &o.commitment == commitment) {
                let entry = level.orders.remove(pos);
                if level.orders.is_empty() {
                    book.remove(&price);
                }
                return Some(entry);
            }
        }

        None
    }

    /// Get best bid price
    pub fn best_bid(&self) -> Option<u128> {
        self.bids.keys().next_back().copied()
    }

    /// Get best ask price
    pub fn best_ask(&self) -> Option<u128> {
        self.asks.keys().next().copied()
    }

    /// Check if there are crossing orders
    pub fn has_crossing(&self) -> bool {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => bid >= ask,
            _ => false,
        }
    }

    /// Get top bid order
    pub fn top_bid(&self) -> Option<&OrderEntry> {
        self.bids
            .values()
            .next_back()?
            .orders
            .first()
    }

    /// Get top ask order
    pub fn top_ask(&self) -> Option<&OrderEntry> {
        self.asks
            .values()
            .next()?
            .orders
            .first()
    }
}
```

## 8.4 Matching Engine

`tee/src/matching/mod.rs`:

```rust
//! Matching engine with price-time priority

use crate::orderbook::{OrderBook, OrderEntry};
use deplob_core::order::OrderSide;

/// A matched trade
#[derive(Debug, Clone)]
pub struct Trade {
    pub buyer: TradeParty,
    pub seller: TradeParty,
    pub price: u128,
    pub quantity: u128,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct TradeParty {
    pub commitment: [u8; 32],
    pub deposit_nullifier: [u8; 32],
    pub remaining_quantity: u128,
    pub is_fully_filled: bool,
}

/// Matching engine
pub struct MatchingEngine {
    pub books: std::collections::HashMap<([u8; 20], [u8; 20]), OrderBook>,
}

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            books: std::collections::HashMap::new(),
        }
    }

    /// Get or create order book for a pair
    pub fn get_or_create_book(&mut self, base: [u8; 20], quote: [u8; 20]) -> &mut OrderBook {
        self.books
            .entry((base, quote))
            .or_insert_with(|| OrderBook::new(base, quote))
    }

    /// Add order and attempt matching
    pub fn add_order(&mut self, entry: OrderEntry) -> Vec<Trade> {
        let base = entry.order.token_in;
        let quote = entry.order.token_out;

        let book = self.get_or_create_book(base, quote);
        book.add_order(entry);

        // Try to match
        self.match_orders(base, quote)
    }

    /// Match orders in a book
    pub fn match_orders(&mut self, base: [u8; 20], quote: [u8; 20]) -> Vec<Trade> {
        let mut trades = Vec::new();

        let book = match self.books.get_mut(&(base, quote)) {
            Some(b) => b,
            None => return trades,
        };

        while book.has_crossing() {
            // Get best bid and ask
            let best_bid_price = book.best_bid().unwrap();
            let best_ask_price = book.best_ask().unwrap();

            // Get mutable references to orders
            let bid_level = book.bids.get_mut(&best_bid_price).unwrap();
            let ask_level = book.asks.get_mut(&best_ask_price).unwrap();

            let bid_order = &mut bid_level.orders[0];
            let ask_order = &mut ask_level.orders[0];

            // Determine trade quantity
            let trade_qty = bid_order.remaining_quantity.min(ask_order.remaining_quantity);

            // Execution price is the resting order's price (maker price)
            // If bid was first, use bid price; if ask was first, use ask price
            // For simplicity, use ask price (taker buys at ask)
            let execution_price = best_ask_price;

            // Create trade
            let trade = Trade {
                buyer: TradeParty {
                    commitment: bid_order.commitment,
                    deposit_nullifier: bid_order.deposit_nullifier,
                    remaining_quantity: bid_order.remaining_quantity - trade_qty,
                    is_fully_filled: bid_order.remaining_quantity == trade_qty,
                },
                seller: TradeParty {
                    commitment: ask_order.commitment,
                    deposit_nullifier: ask_order.deposit_nullifier,
                    remaining_quantity: ask_order.remaining_quantity - trade_qty,
                    is_fully_filled: ask_order.remaining_quantity == trade_qty,
                },
                price: execution_price,
                quantity: trade_qty,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };

            trades.push(trade.clone());

            // Update quantities
            bid_order.remaining_quantity -= trade_qty;
            ask_order.remaining_quantity -= trade_qty;

            // Remove filled orders
            if trade.buyer.is_fully_filled {
                bid_level.orders.remove(0);
                if bid_level.orders.is_empty() {
                    book.bids.remove(&best_bid_price);
                }
                book.index.remove(&trade.buyer.commitment);
            }

            if trade.seller.is_fully_filled {
                ask_level.orders.remove(0);
                if ask_level.orders.is_empty() {
                    book.asks.remove(&best_ask_price);
                }
                book.index.remove(&trade.seller.commitment);
            }
        }

        trades
    }
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deplob_core::order::Order;

    fn make_order(price: u128, qty: u128, side: OrderSide) -> OrderEntry {
        OrderEntry {
            order: Order {
                price,
                quantity: qty,
                side,
                token_in: [1u8; 20],
                token_out: [2u8; 20],
            },
            commitment: rand::random(),
            deposit_nullifier: rand::random(),
            timestamp: 0,
            remaining_quantity: qty,
        }
    }

    #[test]
    fn test_simple_match() {
        let mut engine = MatchingEngine::new();

        // Add sell order at 100
        let sell = make_order(100, 10, OrderSide::Sell);
        engine.add_order(sell);

        // Add buy order at 100 - should match
        let buy = make_order(100, 10, OrderSide::Buy);
        let trades = engine.add_order(buy);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 100);
        assert_eq!(trades[0].quantity, 10);
    }

    #[test]
    fn test_partial_fill() {
        let mut engine = MatchingEngine::new();

        // Add sell order for 100 units
        let sell = make_order(100, 100, OrderSide::Sell);
        engine.add_order(sell);

        // Add buy order for 30 units
        let buy = make_order(100, 30, OrderSide::Buy);
        let trades = engine.add_order(buy);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 30);
        assert!(trades[0].buyer.is_fully_filled);
        assert!(!trades[0].seller.is_fully_filled);
        assert_eq!(trades[0].seller.remaining_quantity, 70);
    }

    #[test]
    fn test_price_priority() {
        let mut engine = MatchingEngine::new();

        // Add sells at different prices
        engine.add_order(make_order(102, 10, OrderSide::Sell));
        engine.add_order(make_order(100, 10, OrderSide::Sell)); // Best ask
        engine.add_order(make_order(101, 10, OrderSide::Sell));

        // Buy should match with best ask (100)
        let trades = engine.add_order(make_order(105, 10, OrderSide::Buy));

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 100); // Matched at best ask
    }
}
```

## 8.5 Settlement Generation

`tee/src/settlement/mod.rs`:

```rust
//! Settlement generation for matched trades

use crate::matching::Trade;
use deplob_core::poseidon::poseidon_hash;

/// Settlement data to submit on-chain
#[derive(Debug, Clone)]
pub struct Settlement {
    pub buyer_old_nullifier: [u8; 32],
    pub seller_old_nullifier: [u8; 32],
    pub buyer_new_commitment: [u8; 32],
    pub seller_new_commitment: [u8; 32],
    pub attestation: Vec<u8>,
    pub proof: Vec<u8>,
}

/// New commitment data for a party
#[derive(Debug, Clone)]
pub struct NewCommitmentData {
    pub nullifier_note: [u8; 32],
    pub secret: [u8; 32],
    pub token: [u8; 20],
    pub amount: u128,
}

pub struct SettlementEngine {
    /// TEE signing key (for attestation)
    signing_key: [u8; 32],
}

impl SettlementEngine {
    pub fn new(signing_key: [u8; 32]) -> Self {
        Self { signing_key }
    }

    /// Generate settlement for a trade
    pub fn generate_settlement(&self, trade: &Trade) -> Settlement {
        // Generate new secrets for both parties
        let buyer_new = self.generate_new_commitment(
            trade.buyer.commitment, // Derive from original
            &trade,
            true, // is_buyer
        );

        let seller_new = self.generate_new_commitment(
            trade.seller.commitment,
            &trade,
            false, // is_seller
        );

        // Create attestation (TEE signature over settlement data)
        let attestation = self.create_attestation(
            &trade.buyer.deposit_nullifier,
            &trade.seller.deposit_nullifier,
            &buyer_new.commitment,
            &seller_new.commitment,
            trade.price,
            trade.quantity,
        );

        Settlement {
            buyer_old_nullifier: trade.buyer.deposit_nullifier,
            seller_old_nullifier: trade.seller.deposit_nullifier,
            buyer_new_commitment: buyer_new.commitment,
            seller_new_commitment: seller_new.commitment,
            attestation,
            proof: vec![], // SP1 settlement proof (if needed)
        }
    }

    fn generate_new_commitment(
        &self,
        _original_commitment: [u8; 32],
        trade: &Trade,
        is_buyer: bool,
    ) -> NewCommitment {
        // Generate new random values
        let nullifier_note: [u8; 32] = rand::random();
        let secret: [u8; 32] = rand::random();

        // Determine token and amount
        // Buyer receives: token_out (the asset they bought)
        // Seller receives: token_in (the payment)
        let (token, amount) = if is_buyer {
            // Buyer gets the sold asset
            ([2u8; 20], trade.quantity) // token_out, quantity
        } else {
            // Seller gets the payment
            let payment = trade.quantity * trade.price;
            ([1u8; 20], payment) // token_in, payment amount
        };

        // Compute commitment
        let mut token_bytes = [0u8; 32];
        token_bytes[12..].copy_from_slice(&token);

        let mut amount_bytes = [0u8; 32];
        amount_bytes[16..].copy_from_slice(&amount.to_be_bytes());

        let commitment = poseidon_hash(&[
            nullifier_note,
            secret,
            token_bytes,
            amount_bytes,
        ]);

        NewCommitment {
            nullifier_note,
            secret,
            token,
            amount,
            commitment,
        }
    }

    fn create_attestation(
        &self,
        buyer_nullifier: &[u8; 32],
        seller_nullifier: &[u8; 32],
        buyer_new_commitment: &[u8; 32],
        seller_new_commitment: &[u8; 32],
        price: u128,
        quantity: u128,
    ) -> Vec<u8> {
        // Hash all settlement data
        let mut data = Vec::new();
        data.extend_from_slice(buyer_nullifier);
        data.extend_from_slice(seller_nullifier);
        data.extend_from_slice(buyer_new_commitment);
        data.extend_from_slice(seller_new_commitment);
        data.extend_from_slice(&price.to_be_bytes());
        data.extend_from_slice(&quantity.to_be_bytes());

        // Sign with TEE key (simplified - use proper SGX attestation in production)
        // In production, this would be an SGX remote attestation
        let signature = self.sign(&data);

        signature
    }

    fn sign(&self, data: &[u8]) -> Vec<u8> {
        // Simplified signing - use proper ECDSA/Ed25519 in production
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&self.signing_key);
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

#[derive(Debug)]
struct NewCommitment {
    nullifier_note: [u8; 32],
    secret: [u8; 32],
    token: [u8; 20],
    amount: u128,
    commitment: [u8; 32],
}
```

## 8.6 Event Listener

`tee/src/listener/mod.rs`:

```rust
//! Blockchain event listener for order events

use alloy_primitives::{Address, B256};
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum ContractEvent {
    OrderCreated {
        order_commitment: [u8; 32],
        encrypted_order: Vec<u8>,
        timestamp: u64,
    },
    OrderCancelled {
        order_nullifier: [u8; 32],
        timestamp: u64,
    },
}

pub struct EventListener<P: Provider> {
    provider: P,
    contract_address: Address,
    event_tx: mpsc::Sender<ContractEvent>,
}

impl<P: Provider + Clone + 'static> EventListener<P> {
    pub fn new(
        provider: P,
        contract_address: Address,
        event_tx: mpsc::Sender<ContractEvent>,
    ) -> Self {
        Self {
            provider,
            contract_address,
            event_tx,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        // Create filter for OrderCreated and OrderCancelled events
        let filter = Filter::new()
            .address(self.contract_address)
            .from_block(0);

        // Subscribe to logs
        let logs = self.provider.get_logs(&filter).await?;

        for log in logs {
            // Parse log based on topic
            let event = self.parse_log(&log)?;
            self.event_tx.send(event).await?;
        }

        Ok(())
    }

    fn parse_log(&self, log: &alloy_rpc_types::Log) -> anyhow::Result<ContractEvent> {
        // Parse based on event signature
        // OrderCreated(bytes32 indexed orderCommitment, bytes encryptedOrder, uint256 timestamp)
        // OrderCancelled(bytes32 indexed orderNullifier, uint256 timestamp)

        let topic0 = log.topics().first().ok_or_else(|| anyhow::anyhow!("No topic"))?;

        // Simplified parsing - use proper ABI decoding in production
        if topic0.as_slice() == &[/* OrderCreated signature */] {
            Ok(ContractEvent::OrderCreated {
                order_commitment: log.topics().get(1)
                    .map(|t| t.0)
                    .unwrap_or_default(),
                encrypted_order: log.data().data.to_vec(),
                timestamp: 0, // Parse from data
            })
        } else {
            Ok(ContractEvent::OrderCancelled {
                order_nullifier: log.topics().get(1)
                    .map(|t| t.0)
                    .unwrap_or_default(),
                timestamp: 0,
            })
        }
    }
}
```

## 8.7 Main TEE Application

`tee/src/main.rs`:

```rust
//! DePLOB TEE Matching Engine

mod listener;
mod matching;
mod orderbook;
mod settlement;

use matching::MatchingEngine;
use settlement::SettlementEngine;
use tokio::sync::mpsc;
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting DePLOB TEE Matching Engine");

    // Load configuration
    let config = load_config()?;

    // Initialize components
    let mut matching_engine = MatchingEngine::new();
    let settlement_engine = SettlementEngine::new(config.tee_signing_key);

    // Create event channel
    let (event_tx, mut event_rx) = mpsc::channel(1000);

    // Start event listener (in production, connect to actual provider)
    // let listener = EventListener::new(provider, contract_address, event_tx);
    // tokio::spawn(async move { listener.start().await });

    // Main event loop
    while let Some(event) = event_rx.recv().await {
        match event {
            listener::ContractEvent::OrderCreated {
                order_commitment,
                encrypted_order,
                timestamp,
            } => {
                info!("Received new order: {:?}", hex::encode(order_commitment));

                // Decrypt order
                match decrypt_order(&encrypted_order, &config.decryption_key) {
                    Ok(order_entry) => {
                        // Add to matching engine
                        let trades = matching_engine.add_order(order_entry);

                        // Process any matches
                        for trade in trades {
                            info!("Trade matched: {} @ {}", trade.quantity, trade.price);

                            // Generate settlement
                            let settlement = settlement_engine.generate_settlement(&trade);

                            // Submit to contract
                            if let Err(e) = submit_settlement(&settlement, &config).await {
                                error!("Failed to submit settlement: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to decrypt order: {}", e);
                    }
                }
            }

            listener::ContractEvent::OrderCancelled {
                order_nullifier,
                timestamp,
            } => {
                info!("Order cancelled: {:?}", hex::encode(order_nullifier));
                // Find and remove order from book
                // (Would need to track nullifier -> commitment mapping)
            }
        }
    }

    Ok(())
}

fn load_config() -> anyhow::Result<Config> {
    // Load from environment or config file
    Ok(Config {
        tee_signing_key: [0u8; 32], // Load actual key
        decryption_key: [0u8; 32],  // Load actual key
        contract_address: [0u8; 20],
        rpc_url: String::new(),
    })
}

fn decrypt_order(
    encrypted: &[u8],
    key: &[u8; 32],
) -> anyhow::Result<orderbook::OrderEntry> {
    use deplob_core::encryption::decrypt_aes_gcm;

    // Extract nonce (first 12 bytes)
    let nonce: [u8; 12] = encrypted[..12].try_into()?;
    let ciphertext = &encrypted[12..];

    let plaintext = decrypt_aes_gcm(key, ciphertext, &nonce)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

    // Deserialize order
    let order: deplob_core::order::OrderPayload = bincode::deserialize(&plaintext)?;

    Ok(orderbook::OrderEntry {
        order: order.order,
        commitment: order.order_commitment,
        deposit_nullifier: order.deposit_nullifier,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        remaining_quantity: order.order.quantity,
    })
}

async fn submit_settlement(
    settlement: &settlement::Settlement,
    config: &Config,
) -> anyhow::Result<()> {
    // Submit settlement transaction to contract
    // In production, use alloy to send transaction
    info!("Submitting settlement to contract");
    Ok(())
}

struct Config {
    tee_signing_key: [u8; 32],
    decryption_key: [u8; 32],
    contract_address: [u8; 20],
    rpc_url: String,
}
```

## 8.8 Checklist

- [ ] Order book implementation correct
- [ ] Price-time priority matching works
- [ ] Partial fills handled correctly
- [ ] Settlement generation works
- [ ] Attestation created correctly
- [ ] Event listener receives orders
- [ ] Order decryption works
- [ ] Settlement submission works
- [ ] Cancelled orders removed from book
