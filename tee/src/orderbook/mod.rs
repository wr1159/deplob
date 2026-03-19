use std::collections::{BTreeMap, HashMap, VecDeque};

use crate::types::{OrderEntry, OrderSide};

#[derive(Debug, Default)]
pub struct PriceLevel {
    pub orders: VecDeque<OrderEntry>,
}

impl PriceLevel {
    pub fn push(&mut self, entry: OrderEntry) {
        self.orders.push_back(entry);
    }

    pub fn pop_front(&mut self) -> Option<OrderEntry> {
        self.orders.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    pub fn peek(&self) -> Option<&OrderEntry> {
        self.orders.front()
    }
}

#[derive(Debug, Default)]
pub struct OrderBook {
    /// Bids: price -> queue of orders (iterate in reverse for best bid = highest price)
    pub bids: BTreeMap<u128, PriceLevel>,
    /// Asks: price -> queue of orders (first entry = best ask = lowest price)
    pub asks: BTreeMap<u128, PriceLevel>,
    /// Index: order_id -> (side, price) for O(log n) removal by id
    pub index: HashMap<[u8; 32], (OrderSide, u128)>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_order(&mut self, entry: OrderEntry) {
        let side = entry.order.side;
        let price = entry.order.price;
        let order_id = entry.order_id;
        self.index.insert(order_id, (side, price));
        match side {
            OrderSide::Buy => self.bids.entry(price).or_default().push(entry),
            OrderSide::Sell => self.asks.entry(price).or_default().push(entry),
        }
    }

    pub fn best_bid_price(&self) -> Option<u128> {
        self.bids.keys().next_back().copied()
    }

    pub fn best_ask_price(&self) -> Option<u128> {
        self.asks.keys().next().copied()
    }

    pub fn has_crossing(&self) -> bool {
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) => bid >= ask,
            _ => false,
        }
    }

    pub fn pop_best_bid(&mut self) -> Option<OrderEntry> {
        let price = self.best_bid_price()?;
        let level = self.bids.get_mut(&price)?;
        let entry = level.pop_front()?;
        if level.is_empty() {
            self.bids.remove(&price);
        }
        self.index.remove(&entry.order_id);
        Some(entry)
    }

    pub fn pop_best_ask(&mut self) -> Option<OrderEntry> {
        let price = self.best_ask_price()?;
        let level = self.asks.get_mut(&price)?;
        let entry = level.pop_front()?;
        if level.is_empty() {
            self.asks.remove(&price);
        }
        self.index.remove(&entry.order_id);
        Some(entry)
    }

    pub fn peek_best_bid(&self) -> Option<&OrderEntry> {
        self.bids.get(&self.best_bid_price()?)?.peek()
    }

    pub fn peek_best_ask(&self) -> Option<&OrderEntry> {
        self.asks.get(&self.best_ask_price()?)?.peek()
    }

    /// Remove an order by id. Returns the entry if found.
    pub fn remove_order(&mut self, order_id: &[u8; 32]) -> Option<OrderEntry> {
        let (side, price) = self.index.remove(order_id)?;
        let book = match side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };
        let level = book.get_mut(&price)?;
        let pos = level.orders.iter().position(|e| &e.order_id == order_id)?;
        let entry = level.orders.remove(pos)?;
        if level.is_empty() {
            book.remove(&price);
        }
        Some(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Order, OrderSide};

    fn make_entry(order_id: u8, side: OrderSide, price: u128, quantity: u128) -> OrderEntry {
        OrderEntry {
            order_id: [order_id; 32],
            deposit_nullifier: [order_id + 100; 32],
            order: Order {
                price,
                quantity,
                side,
                token_in: [1u8; 20],
                token_out: [2u8; 20],
            },
            timestamp: 0,
        }
    }

    #[test]
    fn test_best_bid_is_highest_price() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 100, 10));
        book.add_order(make_entry(2, OrderSide::Buy, 200, 10));
        book.add_order(make_entry(3, OrderSide::Buy, 150, 10));
        assert_eq!(book.best_bid_price(), Some(200));
    }

    #[test]
    fn test_best_ask_is_lowest_price() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Sell, 300, 10));
        book.add_order(make_entry(2, OrderSide::Sell, 100, 10));
        book.add_order(make_entry(3, OrderSide::Sell, 200, 10));
        assert_eq!(book.best_ask_price(), Some(100));
    }

    #[test]
    fn test_crossing_detected() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 200, 10));
        book.add_order(make_entry(2, OrderSide::Sell, 150, 10));
        assert!(book.has_crossing());
    }

    #[test]
    fn test_no_crossing() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 100, 10));
        book.add_order(make_entry(2, OrderSide::Sell, 200, 10));
        assert!(!book.has_crossing());
    }

    #[test]
    fn test_remove_order_by_id() {
        let mut book = OrderBook::new();
        let entry = make_entry(1, OrderSide::Buy, 100, 10);
        let id = entry.order_id;
        book.add_order(entry);
        assert!(book.remove_order(&id).is_some());
        assert!(book.bids.is_empty());
        assert!(!book.index.contains_key(&id));
    }

    #[test]
    fn test_time_priority_fifo() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 100, 5));
        book.add_order(make_entry(2, OrderSide::Buy, 100, 7));
        let first = book.pop_best_bid().unwrap();
        assert_eq!(first.order_id, [1u8; 32]); // earlier order comes first
    }
}
