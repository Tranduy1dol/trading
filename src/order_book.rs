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

    id_to_index: FxHashMap<i32, usize>,
    id_to_price: FxHashMap<i32, Price>,
}

impl OrderBook {
    pub fn add_order(&mut self, mut order: Order) {
        let mut remaining_qty = order.quantity;

        if order.side == OrderSide::Buy {
            let mut current_level = self.asks.find_next_non_empty_from(0);

            while let Some(level) = current_level {
                if remaining_qty == 0 || order.price.0 < level as u64 {
                    break;
                }

                let mut current_node_index = self.asks.levels[level].head;

                while let Some(node_idx) = current_node_index {
                    if remaining_qty == 0 {
                        break;
                    }

                    let node = &self.pool.nodes[node_idx];
                    let next_node = node.next;
                    let resting_user_id = node.order.user_id;
                    let resting_qty = node.order.quantity;
                    let resting_order_id = node.order.id;

                    if resting_user_id == order.user_id {
                        current_node_index = next_node;
                        continue;
                    }

                    let trade_qty = std::cmp::min(remaining_qty, resting_qty);

                    remaining_qty -= trade_qty;

                    let node = &mut self.pool.nodes[node_idx];
                    node.order.quantity -= trade_qty;
                    self.asks.sub_qty_at(level, trade_qty);

                    if self.pool.nodes[node_idx].order.quantity == 0 {
                        self.asks.levels[level].unlink(&mut self.pool, node_idx);
                        self.pool.deallocate(node_idx);
                        self.id_to_index.remove(&(resting_order_id as i32));
                        self.id_to_price.remove(&(resting_order_id as i32));
                    }

                    current_node_index = next_node;
                }

                if self.asks.levels[level].head.is_none() {
                    current_level = self.asks.find_next_non_empty_from(level + 1);
                } else {
                    current_level = self.asks.find_next_non_empty_from(level);
                    if current_level == Some(level) {
                        break;
                    }
                }
            }
        } else {
            let mut current_level = self
                .bids
                .find_prev_non_empty_from(crate::price_level::MAX_LEVEL - 1);

            while let Some(level) = current_level {
                if remaining_qty == 0 || order.price.0 > level as u64 {
                    break;
                }

                let mut current_node_index = self.bids.levels[level].head;

                while let Some(node_idx) = current_node_index {
                    if remaining_qty == 0 {
                        break;
                    }

                    let node = &self.pool.nodes[node_idx];
                    let next_node = node.next;
                    let resting_user_id = node.order.user_id;
                    let resting_qty = node.order.quantity;
                    let resting_order_id = node.order.id;

                    if resting_user_id == order.user_id {
                        current_node_index = next_node;
                        continue;
                    }

                    let trade_qty = std::cmp::min(remaining_qty, resting_qty);
                    remaining_qty -= trade_qty;

                    let node = &mut self.pool.nodes[node_idx];
                    node.order.quantity -= trade_qty;
                    self.bids.sub_qty_at(level, trade_qty);

                    if self.pool.nodes[node_idx].order.quantity == 0 {
                        self.bids.levels[level].unlink(&mut self.pool, node_idx);
                        self.pool.deallocate(node_idx);
                        self.id_to_index.remove(&(resting_order_id as i32));
                        self.id_to_price.remove(&(resting_order_id as i32));
                    }

                    current_node_index = next_node;
                }

                if self.bids.levels[level].head.is_none() {
                    if level > 0 {
                        current_level = self.bids.find_prev_non_empty_from(level - 1);
                    } else {
                        current_level = None;
                    }
                } else {
                    current_level = self.bids.find_prev_non_empty_from(level);
                    if current_level == Some(level) {
                        break;
                    }
                }
            }
        }
        if remaining_qty > 0 && matches!(order.r#type, crate::order::OrderType::GTC) {
            order.quantity = remaining_qty;

            let order_id = order.id;
            let is_buy = order.side == OrderSide::Buy;
            let price_idx = order.price.0 as usize;
            let price_clone = crate::price::Price(order.price.0);

            let new_node_index = self.pool.allocate(order);

            if is_buy {
                self.bids.levels[price_idx].push_back(&mut self.pool, new_node_index);
                self.bids.add_qty_at(price_idx, remaining_qty);
            } else {
                self.asks.levels[price_idx].push_back(&mut self.pool, new_node_index);
                self.asks.add_qty_at(price_idx, remaining_qty);
            }

            self.id_to_index.insert(order_id as i32, new_node_index);
            self.id_to_price.insert(order_id as i32, price_clone);
        }
    }
}
