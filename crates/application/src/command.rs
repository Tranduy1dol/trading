use domain::{order::Order, price::Price};

pub enum Command {
    AddOrder(Order),
    CancelOrder {
        asset_id: u64,
        order_id: u64,
    },
    ModifyOrder {
        asset_id: u64,
        order_id: u64,
        new_price: Price,
        new_qty: u64,
    },
}
