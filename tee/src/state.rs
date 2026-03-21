use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::attestation::AttestationProvider;
use crate::chain::ChainClient;
use crate::orderbook::OrderBook;
use crate::settlement::StoredSettlement;
use crate::types::OrderEntry;

pub struct TeeState {
    pub book: OrderBook,
    /// deposit_nullifier -> order_id: prevents the same deposit backing two orders
    pub locked_deposits: HashMap<[u8; 32], [u8; 32]>,
    /// order_id -> deposit_nullifier: used for cancellation lookup
    pub order_to_deposit: HashMap<[u8; 32], [u8; 32]>,
    /// order_id -> OrderEntry: full order details
    pub order_details: HashMap<[u8; 32], OrderEntry>,
    /// deposit_nullifier -> StoredSettlement: new deposit notes after a trade
    pub settlements: HashMap<[u8; 32], StoredSettlement>,
    pub chain: Arc<dyn ChainClient>,
    pub attestation: Arc<dyn AttestationProvider>,
}

impl TeeState {
    pub fn new(chain: Arc<dyn ChainClient>, attestation: Arc<dyn AttestationProvider>) -> Self {
        Self {
            book: OrderBook::new(),
            locked_deposits: HashMap::new(),
            order_to_deposit: HashMap::new(),
            order_details: HashMap::new(),
            settlements: HashMap::new(),
            chain,
            attestation,
        }
    }
}

pub type SharedState = Arc<RwLock<TeeState>>;

pub fn new_shared(
    chain: Arc<dyn ChainClient>,
    attestation: Arc<dyn AttestationProvider>,
) -> SharedState {
    Arc::new(RwLock::new(TeeState::new(chain, attestation)))
}
