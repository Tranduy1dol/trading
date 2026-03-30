use domain::{error::OrderError, trade::Trade};

pub enum Response {
    Trades(Vec<Trade>),
    Ack,
    Error(OrderError),
}
