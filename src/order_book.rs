use rustc_hash::FxHashMap;

use crate::{
    order::{Order, OrderSide, OrderType},
    order_pool::OrderPool,
    price::Price,
    price_level::{MAX_LEVEL, PriceLevel},
};

pub struct OrderBook {
    pool: OrderPool,
    bids: PriceLevel,
    asks: PriceLevel,

    id_to_index: FxHashMap<u64, usize>,
    id_to_price: FxHashMap<u64, Price>,

    pub best_ask_index: Option<usize>,
    pub best_bid_index: Option<usize>,
}

impl OrderBook {
    pub fn new(capacity: usize) -> Self {
        Self {
            pool: OrderPool::new(capacity),
            bids: PriceLevel::new(),
            asks: PriceLevel::new(),
            id_to_index: FxHashMap::default(),
            id_to_price: FxHashMap::default(),
            best_ask_index: None,
            best_bid_index: None,
        }
    }

    pub fn add_order(&mut self, mut order: Order) {
        match order.r#type {
            OrderType::GTC | OrderType::IOC => {
                let remaining_qty = self.match_order(&mut order);

                if remaining_qty > 0 && matches!(order.r#type, OrderType::GTC) {
                    order.quantity = remaining_qty;
                    self.insert_maker(order);
                }
            }
            OrderType::FOK => {
                if self.check_available(&order) >= order.quantity as i64 {
                    self.match_order(&mut order);
                }
            }
        }
    }

    pub fn cancel_order(&mut self, order_id: u64) {
        let node_index = match self.id_to_index.get(&order_id) {
            Some(&idx) => idx,
            None => return,
        };
        let order_price = *self.id_to_price.get(&order_id).unwrap();

        let node = &self.pool.nodes[node_index];
        let order_side = node.order.side;
        let canceled_qty = node.order.quantity;

        let price_idx = match PriceLevel::get_index_from_price(order_price) {
            Some(idx) => idx,
            None => return,
        };

        match order_side {
            OrderSide::Buy => {
                self.bids.levels[price_idx].unlink(&mut self.pool, node_index);
                self.bids.sub_qty_at(price_idx, canceled_qty);

                if self.bids.levels[price_idx].head.is_none()
                    && self.best_bid_index == Some(price_idx)
                {
                    self.best_bid_index = self
                        .bids
                        .find_prev_non_empty_from(price_idx.saturating_sub(1));
                }
            }
            OrderSide::Sell => {
                self.asks.levels[price_idx].unlink(&mut self.pool, node_index);
                self.asks.sub_qty_at(price_idx, canceled_qty);

                if self.asks.levels[price_idx].head.is_none()
                    && self.best_ask_index == Some(price_idx)
                {
                    self.best_ask_index = self.asks.find_next_non_empty_from(price_idx + 1);
                }
            }
        }

        self.pool.deallocate(node_index);
        self.id_to_index.remove(&order_id);
        self.id_to_price.remove(&order_id);
    }

    pub fn modify_order(&mut self, order_id: u64, new_price: Price, new_qty: u64) {
        let node_index = match self.id_to_index.get(&order_id) {
            Some(&idx) => idx,
            None => return,
        };

        let node = &self.pool.nodes[node_index];
        let order = Order {
            id: node.order.id,
            user_id: node.order.user_id,
            asset_id: node.order.asset_id,
            quantity: new_qty,
            price: new_price,
            side: node.order.side,
            r#type: crate::order::OrderType::GTC,
            timestamp: node.order.timestamp,
        };

        self.cancel_order(order_id);
        self.insert_maker(order);
    }

    fn match_order(&mut self, order: &mut Order) -> u64 {
        if order.side == OrderSide::Buy {
            let current_level = self.asks.find_next_non_empty_from(0);
            let rem = Self::execute_match(
                &mut self.id_to_index,
                &mut self.id_to_price,
                &mut self.asks,
                &mut self.pool,
                order,
                current_level,
                |taker_price, level_price| taker_price.0 >= level_price.0,
                |level, idx| level.find_next_non_empty_from(idx + 1),
            );

            if let Some(best) = self.best_ask_index
                && self.asks.levels[best].head.is_none()
            {
                self.best_ask_index = self.asks.find_next_non_empty_from(best);
            }
            rem
        } else {
            let current_level = self.bids.find_prev_non_empty_from(MAX_LEVEL - 1);
            let rem = Self::execute_match(
                &mut self.id_to_index,
                &mut self.id_to_price,
                &mut self.bids,
                &mut self.pool,
                order,
                current_level,
                |taker_price, level_price| taker_price.0 <= level_price.0,
                |level, idx| {
                    if idx > 0 {
                        level.find_prev_non_empty_from(idx - 1)
                    } else {
                        None
                    }
                },
            );

            if let Some(best) = self.best_bid_index
                && self.bids.levels[best].head.is_none()
            {
                self.best_bid_index = self.bids.find_prev_non_empty_from(best);
            }
            rem
        }
    }

    fn check_available(&self, order: &Order) -> i64 {
        if order.side == OrderSide::Buy {
            let current_level = self.asks.find_next_non_empty_from(0);
            Self::available_qty(
                &self.asks,
                &self.pool,
                order,
                current_level,
                |taker_price, level_price| taker_price.0 >= level_price.0,
                |level, idx| level.find_next_non_empty_from(idx + 1),
            )
        } else {
            let current_level = self.bids.find_prev_non_empty_from(MAX_LEVEL - 1);
            Self::available_qty(
                &self.bids,
                &self.pool,
                order,
                current_level,
                |taker_price, level_price| taker_price.0 <= level_price.0,
                |level, idx| {
                    if idx > 0 {
                        level.find_prev_non_empty_from(idx - 1)
                    } else {
                        None
                    }
                },
            )
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn execute_match<Fcond, Fnext>(
        id_to_index: &mut FxHashMap<u64, usize>,
        id_to_price: &mut FxHashMap<u64, Price>,
        oposite_level: &mut PriceLevel,
        pool: &mut OrderPool,
        taker: &mut Order,
        mut current_level: Option<usize>,
        price_condition: Fcond,
        next_level: Fnext,
    ) -> u64
    where
        Fcond: Fn(Price, Price) -> bool,
        Fnext: Fn(&PriceLevel, usize) -> Option<usize>,
    {
        let mut remaining_qty = taker.quantity;

        while let Some(level_idx) = current_level {
            let level_price = PriceLevel::get_price_from_index(level_idx);

            if !price_condition(taker.price, level_price) || remaining_qty == 0 {
                break;
            }

            let mut current_node_index = oposite_level.levels[level_idx].head;

            while let Some(node_idx) = current_node_index {
                if remaining_qty == 0 {
                    break;
                }

                let node = unsafe { pool.nodes.get_unchecked(node_idx) };
                let next_node = node.next;
                let resting_user_id = node.order.user_id;
                let resting_qty = node.order.quantity;
                let resting_order_id = node.order.id;

                if resting_user_id == taker.user_id {
                    current_node_index = next_node;
                    continue;
                }

                let trade_qty = std::cmp::min(remaining_qty, resting_qty);
                remaining_qty -= trade_qty;

                let mut_node = unsafe { pool.nodes.get_unchecked_mut(node_idx) };
                mut_node.order.quantity -= trade_qty;
                oposite_level.sub_qty_at(level_idx, trade_qty);

                if mut_node.order.quantity == 0 {
                    unsafe {
                        oposite_level
                            .levels
                            .get_unchecked_mut(level_idx)
                            .unlink(pool, node_idx);
                    }
                    pool.deallocate(node_idx);
                    id_to_index.remove(&resting_order_id);
                    id_to_price.remove(&resting_order_id);
                }

                current_node_index = next_node;
            }

            if oposite_level.levels[level_idx].head.is_none() {
                current_level = next_level(oposite_level, level_idx);
            } else {
                break;
            }
        }

        remaining_qty
    }

    fn insert_maker(&mut self, order: Order) {
        let level = match PriceLevel::get_index_from_price(order.price) {
            Some(idx) => idx,
            None => return,
        };

        let remaining_qty = order.quantity;
        let order_id = order.id;
        let order_price = order.price;
        let order_side = order.side;

        let node_index = self.pool.allocate(order);
        self.id_to_index.insert(order_id, node_index);
        self.id_to_price.insert(order_id, order_price);

        match order_side {
            OrderSide::Buy => {
                self.bids.levels[level].push_back(&mut self.pool, node_index);
                self.bids.add_qty_at(level, remaining_qty);
                if self.best_bid_index.is_none_or(|best| level > best) {
                    self.best_bid_index = Some(level);
                }
            }
            OrderSide::Sell => {
                self.asks.levels[level].push_back(&mut self.pool, node_index);
                self.asks.add_qty_at(level, remaining_qty);
                if self.best_ask_index.is_none_or(|best| level < best) {
                    self.best_ask_index = Some(level);
                }
            }
        }
    }

    fn available_qty<Fcond, Fnext>(
        oposite_level: &PriceLevel,
        pool: &OrderPool,
        taker: &Order,
        mut current_level: Option<usize>,
        price_condition: Fcond,
        next_level: Fnext,
    ) -> i64
    where
        Fcond: Fn(Price, Price) -> bool,
        Fnext: Fn(&PriceLevel, usize) -> Option<usize>,
    {
        let mut remaining_qty = taker.quantity;
        while let Some(level_idx) = current_level {
            let level_price = PriceLevel::get_price_from_index(level_idx);
            if !price_condition(taker.price, level_price) || remaining_qty == 0 {
                break;
            }

            let mut current_node_index = oposite_level.levels[level_idx].head;
            while let Some(node_idx) = current_node_index {
                if remaining_qty == 0 {
                    break;
                }

                let node = &pool.nodes[node_idx];
                if node.order.user_id == taker.user_id {
                    current_node_index = node.next;
                    continue;
                }

                let trade_qty = std::cmp::min(remaining_qty, node.order.quantity);
                remaining_qty -= trade_qty;

                current_node_index = node.next;
            }

            current_level = next_level(oposite_level, level_idx);
        }

        taker.quantity as i64 - remaining_qty as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_order(
        id: u64,
        user_id: u64,
        qty: u64,
        price: u64,
        side: OrderSide,
        typ: OrderType,
    ) -> Order {
        Order::new(id, user_id, 1, qty, Price(price), side, typ)
    }

    #[test]
    fn test_add_single_maker_order() {
        let mut book = OrderBook::new(1024);
        let order = create_order(1, 1, 100, 10000, OrderSide::Buy, OrderType::GTC);

        book.add_order(order);

        assert_eq!(book.best_bid_index, Some(0)); // 10000 -> index 0
        assert_eq!(book.best_ask_index, None);
        assert_eq!(book.bids.totals[0], 100);
    }

    #[test]
    fn test_match_full_taker_order() {
        let mut book = OrderBook::new(1024);

        // Add Maker Ask: 100 units @ 10010 (index 10)
        book.add_order(create_order(
            1,
            1,
            100,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));
        assert_eq!(book.best_ask_index, Some(10));
        assert_eq!(book.asks.totals[10], 100);

        // Taker Buy: 50 units @ 10010 (index 10) -> fully matches, 50 left in book
        book.add_order(create_order(
            2,
            2,
            50,
            10010,
            OrderSide::Buy,
            OrderType::GTC,
        ));

        assert_eq!(book.asks.totals[10], 50);
        // Taker buy order should not rest since it was fully filled
        assert_eq!(book.best_bid_index, None);
    }

    #[test]
    fn test_match_partial_taker_order_rests() {
        let mut book = OrderBook::new(1024);

        // Maker Ask: 50 units @ 10010
        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));

        // Taker Buy: 100 units @ 10010 -> matches 50, rests 50
        book.add_order(create_order(
            2,
            2,
            100,
            10010,
            OrderSide::Buy,
            OrderType::GTC,
        ));

        assert_eq!(book.asks.totals[10], 0);
        assert_eq!(book.best_ask_index, None); // Level is empty

        // Remaining 50 rests at 10010 (index 10)
        assert_eq!(book.bids.totals[10], 50);
        assert_eq!(book.best_bid_index, Some(10));
    }

    #[test]
    fn test_ioc_order_does_not_rest() {
        let mut book = OrderBook::new(1024);

        // Maker Ask: 50 units @ 10010
        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));

        // Taker Buy IOC: 100 units @ 10010
        book.add_order(create_order(
            2,
            2,
            100,
            10010,
            OrderSide::Buy,
            OrderType::IOC,
        ));

        assert_eq!(book.asks.totals[10], 0);
        assert_eq!(book.best_ask_index, None);

        // IOC remainder of 50 should be discarded, not rested
        assert_eq!(book.best_bid_index, None);
    }

    #[test]
    fn test_fok_order_success() {
        let mut book = OrderBook::new(1024);

        // Maker Ask: 50 units @ 10010, 50 units @ 10020
        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));
        book.add_order(create_order(
            2,
            1,
            50,
            10020,
            OrderSide::Sell,
            OrderType::GTC,
        ));

        // Taker Buy FOK: 100 units @ 10020 -> can be fully filled
        book.add_order(create_order(
            3,
            2,
            100,
            10020,
            OrderSide::Buy,
            OrderType::FOK,
        ));

        // Entire asks side should be empty
        assert_eq!(book.asks.totals[10], 0);
        assert_eq!(book.asks.totals[20], 0);
        assert_eq!(book.best_ask_index, None);
    }

    #[test]
    fn test_fok_order_failure_due_to_price() {
        let mut book = OrderBook::new(1024);

        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));
        book.add_order(create_order(
            2,
            1,
            50,
            10020,
            OrderSide::Sell,
            OrderType::GTC,
        ));

        // Taker Buy FOK: 100 units @ 10015 -> cannot be fully filled (only 50 available at <= 10015)
        book.add_order(create_order(
            3,
            2,
            100,
            10015,
            OrderSide::Buy,
            OrderType::FOK,
        ));

        // Book should be completely untouched
        assert_eq!(book.asks.totals[10], 50);
        assert_eq!(book.asks.totals[20], 50);
        assert_eq!(book.best_ask_index, Some(10));
    }

    #[test]
    fn test_fok_order_failure_due_to_quantity() {
        let mut book = OrderBook::new(1024);

        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));

        // Taker Buy FOK: 100 units @ 10020 -> not enough total quantity
        book.add_order(create_order(
            2,
            2,
            100,
            10020,
            OrderSide::Buy,
            OrderType::FOK,
        ));

        // Book should be untouched
        assert_eq!(book.asks.totals[10], 50);
    }

    #[test]
    fn test_self_trade_prevention() {
        let mut book = OrderBook::new(1024);

        // User 1 Ask: 50 units @ 10010
        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));

        // User 1 Buy: 50 units @ 10010 -> Should skip matching their own ask!
        book.add_order(create_order(
            2,
            1,
            50,
            10010,
            OrderSide::Buy,
            OrderType::GTC,
        ));

        // Both orders should rest on book without matching
        assert_eq!(book.asks.totals[10], 50);
        assert_eq!(book.bids.totals[10], 50);
    }

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new(1024);

        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));
        assert_eq!(book.asks.totals[10], 50);
        assert_eq!(book.best_ask_index, Some(10));

        book.cancel_order(1);

        assert_eq!(book.asks.totals[10], 0);
        assert_eq!(book.best_ask_index, None);
        assert!(!book.id_to_index.contains_key(&1));
    }

    #[test]
    fn test_modify_order() {
        let mut book = OrderBook::new(1024);

        book.add_order(create_order(
            1,
            1,
            50,
            10010,
            OrderSide::Sell,
            OrderType::GTC,
        ));

        // Modify to 100 units @ 10020
        book.modify_order(1, Price(10020), 100);

        assert_eq!(book.asks.totals[10], 0);
        assert_eq!(book.asks.totals[20], 100);
        assert_eq!(book.best_ask_index, Some(20));
        assert!(book.id_to_index.contains_key(&1)); // Same ID rested
    }
}
