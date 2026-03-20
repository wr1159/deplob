use deplob_core::CommitmentPreimage;
use rand::RngCore;
use serde::Serialize;

use crate::types::Trade;

/// New deposit note stored for a user after a trade settlement.
/// Keyed by the user's old deposit_nullifier.
#[derive(Debug, Clone, Serialize)]
pub struct StoredSettlement {
    pub commitment: String,
    pub nullifier_note: String,
    pub secret: String,
    pub nullifier: String,
    pub token: String,
    pub amount: String,
}

impl StoredSettlement {
    pub fn from_preimage(preimage: &CommitmentPreimage) -> Self {
        Self {
            commitment: format!("0x{}", hex::encode(preimage.commitment())),
            nullifier_note: format!("0x{}", hex::encode(preimage.nullifier_note)),
            secret: format!("0x{}", hex::encode(preimage.secret)),
            nullifier: format!("0x{}", hex::encode(preimage.nullifier())),
            token: format!("0x{}", hex::encode(preimage.token_address())),
            amount: preimage.amount_value().to_string(),
        }
    }
}

/// Data needed to call `settleMatch()` on the smart contract.
#[derive(Debug, Clone)]
pub struct SettlementData {
    pub buyer_old_nullifier: [u8; 32],
    pub seller_old_nullifier: [u8; 32],
    pub buyer_new_commitment: [u8; 32],
    pub seller_new_commitment: [u8; 32],
    /// New deposit note for buyer — must be securely delivered back to the buyer.
    /// TODO: encrypt under buyer's public key before returning.
    pub buyer_new_preimage: CommitmentPreimage,
    /// New deposit note for seller.
    pub seller_new_preimage: CommitmentPreimage,
}

/// Generate settlement data for a matched trade.
///
/// The TEE creates new deposit notes for both parties reflecting their
/// post-trade balances. These new commitments are inserted on-chain via
/// `settleMatch()`, and the new preimages must be delivered back to users.
pub fn generate_settlement(trade: &Trade, rng: &mut impl RngCore) -> SettlementData {
    let buy_entry = &trade.buy_entry;
    let sell_entry = &trade.sell_entry;

    // Buyer receives token_out (what they were buying)
    let buyer_token_out = buy_entry.order.token_out;
    let buyer_quantity_out = trade.execution_quantity;

    // Seller receives token_in value (price * quantity)
    let seller_token_out = sell_entry.order.token_out;
    let seller_quantity_out = trade
        .execution_quantity
        .saturating_mul(trade.execution_price);

    let buyer_new_preimage = random_preimage(rng, buyer_token_out, buyer_quantity_out);
    let seller_new_preimage = random_preimage(rng, seller_token_out, seller_quantity_out);

    SettlementData {
        buyer_old_nullifier: buy_entry.deposit_nullifier,
        seller_old_nullifier: sell_entry.deposit_nullifier,
        buyer_new_commitment: buyer_new_preimage.commitment(),
        seller_new_commitment: seller_new_preimage.commitment(),
        buyer_new_preimage,
        seller_new_preimage,
    }
}

fn random_preimage(rng: &mut impl RngCore, token: [u8; 20], amount: u128) -> CommitmentPreimage {
    let mut nullifier_note = [0u8; 32];
    let mut secret = [0u8; 32];
    rng.fill_bytes(&mut nullifier_note);
    rng.fill_bytes(&mut secret);
    CommitmentPreimage::new(nullifier_note, secret, token, amount)
}
