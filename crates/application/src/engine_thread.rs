use crossbeam_channel::Receiver;
use domain::exchange::Exchange;
use tokio::sync::oneshot;

use crate::{command::Command, response::Response};

pub type EngineMessage = (Command, oneshot::Sender<Response>);

pub fn run(rx: Receiver<EngineMessage>) {
    let mut exchange = Exchange::new(1_000_000);

    while let Ok((cmd, reply)) = rx.recv() {
        let response = match cmd {
            Command::AddOrder(order) => match exchange.add_order(order) {
                Ok(trades) => Response::Trades(trades.to_vec()),
                Err(e) => Response::Error(e),
            },
            Command::CancelOrder { asset_id, order_id } => {
                exchange.cancel_order(asset_id, order_id);
                Response::Ack
            }
            Command::ModifyOrder {
                asset_id,
                order_id,
                new_price,
                new_qty,
            } => {
                exchange.modify_order(asset_id, order_id, new_price, new_qty);
                Response::Ack
            }
        };

        let _ = reply.send(response);
    }
}
