use crate::{order::OrderSide, price::Price};

pub enum MarketDataEvent {
    TradeExecuted {
        price: Price,
        quantity: u64,
        taker_side: OrderSide,
    },
    LevelUpdated {
        price: Price,
        side: OrderSide,
        total_qty: u64,
    },
    BestPriceChanged {
        best_bid: Option<Price>,
        best_ask: Option<Price>,
    },
}
