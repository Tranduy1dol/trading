use serde::{Deserialize, Serialize};

use crate::{order::OrderSide, price::Price};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Trade {
    pub taker_order_id: u64,
    pub maker_order_id: u64,
    pub price: Price,
    pub quantity: u64,
    pub taker_side: OrderSide,
    pub timestamp: u64,
}
