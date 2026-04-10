#[derive(Debug)]
pub enum OrderError {
    ZeroQuantity,
    PriceOutOfRange,
    DuplicateOrderId,
    OrderNotFound,
    AssetNotFound,
}

impl From<&OrderError> for u8 {
    fn from(val: &OrderError) -> Self {
        match val {
            OrderError::ZeroQuantity => 0,
            OrderError::PriceOutOfRange => 1,
            OrderError::DuplicateOrderId => 2,
            OrderError::OrderNotFound => 3,
            OrderError::AssetNotFound => 4,
        }
    }
}
