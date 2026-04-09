use domain::{error::OrderError, trade::Trade};

pub enum Response {
    Ack {
        engine_seq: u64,
        client_seq: u64,
    },
    Fills {
        engine_seq: u64,
        trades: Vec<Trade>,
    },
    Reject {
        engine_seq: u64,
        client_seq: u64,
        reason: OrderError,
    },
}
