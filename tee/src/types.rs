use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Price in base units (token_out per token_in)
    pub price: u128,
    /// Quantity of token_in being offered
    pub quantity: u128,
    pub side: OrderSide,
    pub token_in: [u8; 20],
    pub token_out: [u8; 20],
}

#[derive(Debug, Clone)]
pub struct OrderEntry {
    /// Unique order id = keccak256(deposit_nullifier || price || quantity || side || token_in || token_out)
    pub order_id: [u8; 32],
    /// Nullifier of the backing deposit (used to prevent double-spend and as cancel key)
    pub deposit_nullifier: [u8; 32],
    pub order: Order,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub buy_entry: OrderEntry,
    pub sell_entry: OrderEntry,
    pub execution_price: u128,
    pub execution_quantity: u128,
}
