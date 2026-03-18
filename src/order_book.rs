use rustc_hash::FxHashMap;

use crate::{order_pool::OrderPool, price::Price, price_level::PriceLevel};

pub struct OrderBook {
    pool: OrderPool,
    bids: PriceLevel,
    asks: PriceLevel,

    id_to_index: FxHashMap<i32, usize>,
    id_to_price: FxHashMap<i32, Price>
}
