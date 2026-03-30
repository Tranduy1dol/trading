use crate::{order::OrderSide, price::Price};

pub struct Trade {
    pub taker_order_id: u64,
    pub maker_order_id: u64,
    pub price: Price,
    pub quantity: u64,
    pub taker_side: OrderSide,
    pub timestamp: u64,
}
