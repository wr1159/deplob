use crate::orderbook::OrderBook;
use crate::types::{OrderEntry, Trade};

/// Add an order to the book and immediately run the matching loop.
/// Returns all trades generated (may be empty or multiple for partial fills).
pub fn add_and_match(book: &mut OrderBook, new_entry: OrderEntry) -> Vec<Trade> {
    book.add_order(new_entry);
    run_matching(book)
}

/// Run the matching loop until no more crossings exist.
pub fn run_matching(book: &mut OrderBook) -> Vec<Trade> {
    let mut trades = Vec::new();

    while book.has_crossing() {
        let bid_qty = match book.peek_best_bid() {
            Some(e) => e.order.quantity,
            None => break,
        };
        let ask_qty = match book.peek_best_ask() {
            Some(e) => e.order.quantity,
            None => break,
        };

        let execution_quantity = bid_qty.min(ask_qty);
        let mut bid = book.pop_best_bid().expect("bid exists after peek");
        let mut ask = book.pop_best_ask().expect("ask exists after peek");

        // Execution price = ask price (resting order convention)
        let execution_price = ask.order.price;

        trades.push(Trade {
            buy_entry: bid.clone(),
            sell_entry: ask.clone(),
            execution_price,
            execution_quantity,
        });

        // Reinsert partially filled remainders
        if bid_qty > execution_quantity {
            bid.order.quantity -= execution_quantity;
            book.add_order(bid);
        }
        if ask_qty > execution_quantity {
            ask.order.quantity -= execution_quantity;
            book.add_order(ask);
        }
    }

    trades
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
    fn test_exact_match() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 100, 10));
        let trades = add_and_match(&mut book, make_entry(2, OrderSide::Sell, 100, 10));
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].execution_quantity, 10);
        assert_eq!(trades[0].execution_price, 100);
        assert!(book.bids.is_empty());
        assert!(book.asks.is_empty());
    }

    #[test]
    fn test_no_match_price_gap() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 90, 10));
        let trades = add_and_match(&mut book, make_entry(2, OrderSide::Sell, 100, 10));
        assert!(trades.is_empty());
    }

    #[test]
    fn test_partial_fill_bid_larger() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 100, 15));
        let trades = add_and_match(&mut book, make_entry(2, OrderSide::Sell, 100, 10));
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].execution_quantity, 10);
        // Remaining 5 on bid side
        assert_eq!(book.best_bid_price(), Some(100));
        assert_eq!(book.peek_best_bid().unwrap().order.quantity, 5);
    }

    #[test]
    fn test_partial_fill_ask_larger() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Buy, 100, 10));
        let trades = add_and_match(&mut book, make_entry(2, OrderSide::Sell, 100, 15));
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].execution_quantity, 10);
        // Remaining 5 on ask side
        assert_eq!(book.best_ask_price(), Some(100));
        assert_eq!(book.peek_best_ask().unwrap().order.quantity, 5);
    }

    #[test]
    fn test_price_priority_asks() {
        // Lower ask should be matched first
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Sell, 90, 10));
        book.add_order(make_entry(2, OrderSide::Sell, 80, 10));
        let trades = add_and_match(&mut book, make_entry(3, OrderSide::Buy, 100, 10));
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].execution_price, 80); // lowest ask matched
        assert_eq!(trades[0].sell_entry.order_id, [2u8; 32]);
    }

    #[test]
    fn test_multiple_trades_one_bid_many_asks() {
        let mut book = OrderBook::new();
        book.add_order(make_entry(1, OrderSide::Sell, 100, 5));
        book.add_order(make_entry(2, OrderSide::Sell, 100, 5));
        // Buy order of 10 should consume both asks
        let trades = add_and_match(&mut book, make_entry(3, OrderSide::Buy, 100, 10));
        assert_eq!(trades.len(), 2);
        assert!(book.asks.is_empty());
        assert!(book.bids.is_empty());
    }
}
