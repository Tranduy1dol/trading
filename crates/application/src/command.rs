use domain::{order::Order, price::Price};

pub enum Command {
    AddOrder {
        client_seq: u64,
        order: Order,
    },
    CancelOrder {
        client_seq: u64,
        asset_id: u64,
        order_id: u64,
    },
    ModifyOrder {
        client_seq: u64,
        asset_id: u64,
        order_id: u64,
        new_price: Price,
        new_qty: u64,
    },
}
