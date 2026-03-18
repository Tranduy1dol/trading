use crate::price::Price;

#[derive(PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

pub enum OrderType {
    GTC,
    IOC,
    FOK,
}

pub struct Order {
    pub id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub quantity: u64,
    pub price: Price,
    pub side: OrderSide,
    pub r#type: OrderType,
    pub timestamp: u64,
}
