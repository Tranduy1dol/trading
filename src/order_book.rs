use rustc_hash::FxHashMap;

use crate::{
    order::{Order, OrderSide},
    order_pool::OrderPool,
    price::Price,
    price_level::PriceLevel,
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
        let remaining_qty = if order.side == OrderSide::Buy {
            let current_level = self.asks.find_next_non_empty_from(0);
            Self::execute_match(
                &mut self.id_to_index,
                &mut self.id_to_price,
                &mut self.asks,
                &mut self.pool,
                &mut order,
                current_level,
                |taker_price, level_price| taker_price.0 >= level_price.0,
                |level, idx| level.find_next_non_empty_from(idx + 1),
            )
        } else {
            let current_level = self
                .bids
                .find_prev_non_empty_from(crate::price_level::MAX_LEVEL - 1);
            Self::execute_match(
                &mut self.id_to_index,
                &mut self.id_to_price,
                &mut self.bids,
                &mut self.pool,
                &mut order,
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
        };

        if remaining_qty > 0 {
            order.quantity = remaining_qty;
            self.insert_maker(order);
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

                let node = &pool.nodes[node_idx];
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

                let node = &mut pool.nodes[node_idx];
                node.order.quantity -= trade_qty;
                oposite_level.sub_qty_at(level_idx, trade_qty);

                if pool.nodes[node_idx].order.quantity == 0 {
                    oposite_level.levels[level_idx].unlink(pool, node_idx);
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
        if !matches!(order.r#type, crate::order::OrderType::GTC) {
            return;
        }

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
}
