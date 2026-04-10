use crate::price::Price;

#[derive(PartialEq, Clone, Copy)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl TryFrom<u8> for OrderSide {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(OrderSide::Buy),
            1 => Ok(OrderSide::Sell),
            _ => Err("invalid order side".to_string()),
        }
    }
}

pub enum OrderType {
    GTC,
    IOC,
    FOK,
}

impl TryFrom<u8> for OrderType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(OrderType::GTC),
            1 => Ok(OrderType::IOC),
            2 => Ok(OrderType::FOK),
            _ => Err("invalid order type".to_string()),
        }
    }
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
