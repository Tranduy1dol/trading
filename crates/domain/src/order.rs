use crate::price::Price;

#[derive(PartialEq, Clone, Copy)]
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

impl Order {
    pub fn new(
        id: u64,
        user_id: u64,
        asset_id: u64,
        quantity: u64,
        price: Price,
        side: OrderSide,
        r#type: OrderType,
    ) -> Self {
        Self {
            id,
            user_id,
            asset_id,
            quantity,
            price,
            side,
            r#type,
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }
}
