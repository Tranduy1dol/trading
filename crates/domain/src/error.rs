#[derive(Debug)]
pub enum OrderError {
    ZeroQuantity,
    PriceOutOfRange,
    DuplicateOrderId,
}
