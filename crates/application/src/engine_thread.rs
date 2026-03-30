use crossbeam_channel::Receiver;
use domain::exchange::Exchange;
use tokio::sync::oneshot;

use crate::{command::Command, response::Response};

pub type EngineMessage = (Command, oneshot::Sender<Response>);

pub fn run(rx: Receiver<EngineMessage>) {
    let mut exchange = Exchange::new(1_000_000);

    while let Ok((cmd, reply)) = rx.recv() {
        let response = match cmd {
            Command::AddOrder(order) => todo,
            Command::CancelOrder { asset_id, order_id } => todo!(),
            Command::ModifyOrder {
                asset_id,
                order_id,
                new_price,
                new_qty,
            } => todo!(),
        };

        let _ = reply.send(response);
    }
}
