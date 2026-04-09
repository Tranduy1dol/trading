use domain::exchange::Exchange;

use crate::{command::Command, response::Response};

pub fn process(exchange: &mut Exchange, seq: &mut u64, cmd: Command) -> Response {
    *seq += 1;
    match cmd {
        Command::AddOrder { client_seq, order } => match exchange.add_order(order) {
            Ok(trades) => Response::Fills {
                engine_seq: *seq,
                trades: trades.to_vec(),
            },
            Err(e) => Response::Reject {
                engine_seq: *seq,
                client_seq,
                reason: e,
            },
        },
        Command::CancelOrder {
            client_seq,
            asset_id,
            order_id,
        } => match exchange.cancel_order(asset_id, order_id) {
            Ok(()) => Response::Ack {
                engine_seq: *seq,
                client_seq,
            },
            Err(e) => Response::Reject {
                engine_seq: *seq,
                client_seq,
                reason: e,
            },
        },
        Command::ModifyOrder {
            client_seq,
            asset_id,
            order_id,
            new_price,
            new_qty,
        } => match exchange.modify_order(asset_id, order_id, new_price, new_qty) {
            Ok(()) => Response::Ack {
                engine_seq: *seq,
                client_seq,
            },
            Err(e) => Response::Reject {
                engine_seq: *seq,
                client_seq,
                reason: e,
            },
        },
    }
}
