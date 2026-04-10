use std::ptr;

use application::{command::Command, response::Response};
use domain::{
    order::{Order, OrderSide, OrderType},
    price::Price,
};

use crate::protocol::{
    AckMsg, CancelOrderMsg, FillMsg, MSG_ACK, MSG_CANCEL_ORDER, MSG_FILL, MSG_MODIFY_ORDER,
    MSG_NEW_ORDER, MSG_REJECT, ModifyOrderMsg, NewOrderMsg, RejectMsg,
};

pub fn decode_command(msg_type: u8, payload: &[u8]) -> Option<Command> {
    match msg_type {
        MSG_NEW_ORDER => {
            if payload.len() < size_of::<NewOrderMsg>() {
                return None;
            }

            let msg = unsafe { ptr::read_unaligned(payload.as_ptr() as *const NewOrderMsg) };

            let side = OrderSide::try_from(msg.side).unwrap();
            let ordet_type = OrderType::try_from(msg.order_type).unwrap();

            let order = Order::new(
                msg.order_id,
                msg.user_id,
                msg.asset_id,
                msg.quantity,
                Price(msg.price),
                side,
                ordet_type,
            );

            Some(Command::AddOrder {
                client_seq: msg.client_seq,
                order,
            })
        }
        MSG_CANCEL_ORDER => {
            if payload.len() < size_of::<CancelOrderMsg>() {
                return None;
            }

            let msg = unsafe { ptr::read_unaligned(payload.as_ptr() as *const CancelOrderMsg) };

            Some(Command::CancelOrder {
                client_seq: msg.client_seq,
                asset_id: msg.asset_id,
                order_id: msg.order_id,
            })
        }
        MSG_MODIFY_ORDER => {
            if payload.len() < size_of::<ModifyOrderMsg>() {
                return None;
            }

            let msg = unsafe { ptr::read_unaligned(payload.as_ptr() as *const ModifyOrderMsg) };

            Some(Command::ModifyOrder {
                new_price: Price(msg.new_price),
                new_qty: msg.new_qty,
                client_seq: msg.client_seq,
                asset_id: msg.asset_id,
                order_id: msg.order_id,
            })
        }
        _ => None,
    }
}

pub fn encode_response(response: &Response, buf: &mut Vec<u8>) {
    match response {
        Response::Ack {
            engine_seq,
            client_seq,
        } => {
            let msg = AckMsg {
                engine_seq: *engine_seq,
                client_seq: *client_seq,
            };
            write_frame(buf, MSG_ACK, &msg);
        }
        Response::Fills { engine_seq, trades } => {
            for trade in trades {
                let taker_side = match trade.taker_side {
                    OrderSide::Buy => 0,
                    OrderSide::Sell => 1,
                };
                let msg = FillMsg {
                    engine_seq: *engine_seq,
                    taker_order_id: trade.taker_order_id,
                    maker_order_id: trade.maker_order_id,
                    price: trade.price.0,
                    quantity: trade.quantity,
                    taker_side,
                    timestamp: trade.timestamp,
                };

                write_frame(buf, MSG_FILL, &msg);
            }
        }
        Response::Reject {
            engine_seq,
            client_seq,
            reason,
        } => {
            let msg = RejectMsg {
                client_seq: *client_seq,
                engine_seq: *engine_seq,
                reason: u8::from(reason),
            };
            write_frame(buf, MSG_REJECT, &msg);
        }
    }
}

fn write_frame<T: Sized>(buf: &mut Vec<u8>, msg_type: u8, msg: &T) {
    let payload_size = size_of::<T>();
    let len = (1 + payload_size) as u32;

    buf.extend_from_slice(&len.to_le_bytes());
    buf.push(msg_type);

    let ptr = msg as *const T as *const u8;
    let bytes = unsafe { std::slice::from_raw_parts(ptr, payload_size) };
    buf.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::*;
    use crate::session::Session;
    use domain::error::OrderError;
    use domain::order::OrderSide;
    use domain::price::Price;
    use domain::trade::Trade;

    /// Helper: build a raw frame buffer from a message type and struct
    fn build_frame<T: Sized>(msg_type: u8, msg: &T) -> Vec<u8> {
        let mut buf = Vec::new();
        write_frame(&mut buf, msg_type, msg);
        buf
    }

    /// Helper: extract (msg_type, payload) from a frame buffer, skipping the 4-byte length prefix
    fn split_frame(buf: &[u8]) -> (u8, &[u8]) {
        let msg_type = buf[4];
        let payload = &buf[5..];
        (msg_type, payload)
    }

    // ========== Decode round-trip tests ==========

    #[test]
    fn test_decode_new_order() {
        let msg = NewOrderMsg {
            client_seq: 42,
            order_id: 100,
            user_id: 7,
            asset_id: 1,
            price: 10050,
            quantity: 500,
            side: 0,       // Buy
            order_type: 0, // GTC
        };

        let frame = build_frame(MSG_NEW_ORDER, &msg);
        let (msg_type, payload) = split_frame(&frame);
        let cmd = decode_command(msg_type, payload).expect("decode failed");

        match cmd {
            Command::AddOrder { client_seq, order } => {
                assert_eq!(client_seq, 42);
                assert_eq!(order.id, 100);
                assert_eq!(order.user_id, 7);
                assert_eq!(order.asset_id, 1);
                assert_eq!(order.price.0, 10050);
                assert_eq!(order.quantity, 500);
                assert!(order.side == OrderSide::Buy);
            }
            _ => panic!("expected AddOrder"),
        }
    }

    #[test]
    fn test_decode_new_order_sell_ioc() {
        let msg = NewOrderMsg {
            client_seq: 1,
            order_id: 200,
            user_id: 3,
            asset_id: 5,
            price: 9900,
            quantity: 100,
            side: 1,       // Sell
            order_type: 1, // IOC
        };

        let frame = build_frame(MSG_NEW_ORDER, &msg);
        let (msg_type, payload) = split_frame(&frame);
        let cmd = decode_command(msg_type, payload).expect("decode failed");

        match cmd {
            Command::AddOrder { order, .. } => {
                assert!(order.side == OrderSide::Sell);
                assert_eq!(order.price.0, 9900);
            }
            _ => panic!("expected AddOrder"),
        }
    }

    #[test]
    fn test_decode_cancel_order() {
        let msg = CancelOrderMsg {
            client_seq: 10,
            order_id: 55,
            asset_id: 2,
        };

        let frame = build_frame(MSG_CANCEL_ORDER, &msg);
        let (msg_type, payload) = split_frame(&frame);
        let cmd = decode_command(msg_type, payload).expect("decode failed");

        match cmd {
            Command::CancelOrder {
                client_seq,
                asset_id,
                order_id,
            } => {
                assert_eq!(client_seq, 10);
                assert_eq!(order_id, 55);
                assert_eq!(asset_id, 2);
            }
            _ => panic!("expected CancelOrder"),
        }
    }

    #[test]
    fn test_decode_modify_order() {
        let msg = ModifyOrderMsg {
            client_seq: 99,
            order_id: 77,
            asset_id: 3,
            new_price: 12000,
            new_qty: 250,
        };

        let frame = build_frame(MSG_MODIFY_ORDER, &msg);
        let (msg_type, payload) = split_frame(&frame);
        let cmd = decode_command(msg_type, payload).expect("decode failed");

        match cmd {
            Command::ModifyOrder {
                client_seq,
                asset_id,
                order_id,
                new_price,
                new_qty,
            } => {
                assert_eq!(client_seq, 99);
                assert_eq!(order_id, 77);
                assert_eq!(asset_id, 3);
                assert_eq!(new_price.0, 12000);
                assert_eq!(new_qty, 250);
            }
            _ => panic!("expected ModifyOrder"),
        }
    }

    // ========== Decode edge cases ==========

    #[test]
    fn test_decode_truncated_payload() {
        // Too few bytes for a NewOrderMsg
        let payload = [0u8; 4];
        assert!(decode_command(MSG_NEW_ORDER, &payload).is_none());
    }

    #[test]
    fn test_decode_unknown_msg_type() {
        let payload = [0u8; 64];
        assert!(decode_command(0xFF, &payload).is_none());
    }

    #[test]
    fn test_decode_empty_payload() {
        assert!(decode_command(MSG_NEW_ORDER, &[]).is_none());
        assert!(decode_command(MSG_CANCEL_ORDER, &[]).is_none());
        assert!(decode_command(MSG_MODIFY_ORDER, &[]).is_none());
    }

    // ========== Encode tests ==========

    #[test]
    fn test_encode_ack() {
        let response = Response::Ack {
            engine_seq: 1001,
            client_seq: 42,
        };

        let mut buf = Vec::new();
        encode_response(&response, &mut buf);

        // Verify frame structure: [len:4][type:1][AckMsg]
        let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        assert_eq!(len, 1 + size_of::<AckMsg>());
        assert_eq!(buf[4], MSG_ACK);

        // Decode the AckMsg back
        let msg: AckMsg =
            unsafe { ptr::read_unaligned(buf[5..].as_ptr() as *const AckMsg) };
        assert_eq!({ msg.client_seq }, 42);
        assert_eq!({ msg.engine_seq }, 1001);
    }

    #[test]
    fn test_encode_single_fill() {
        let trade = Trade {
            taker_order_id: 10,
            maker_order_id: 20,
            price: Price(9500),
            quantity: 50,
            taker_side: OrderSide::Buy,
            timestamp: 1234567890,
        };

        let response = Response::Fills {
            engine_seq: 500,
            trades: vec![trade],
        };

        let mut buf = Vec::new();
        encode_response(&response, &mut buf);

        // Should produce exactly one Fill frame
        let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        assert_eq!(buf[4], MSG_FILL);
        assert_eq!(buf.len(), 4 + len); // exactly one frame

        let msg: FillMsg =
            unsafe { ptr::read_unaligned(buf[5..].as_ptr() as *const FillMsg) };
        assert_eq!({ msg.engine_seq }, 500);
        assert_eq!({ msg.taker_order_id }, 10);
        assert_eq!({ msg.maker_order_id }, 20);
        assert_eq!({ msg.price }, 9500);
        assert_eq!({ msg.quantity }, 50);
        assert_eq!({ msg.taker_side }, 0); // Buy
        assert_eq!({ msg.timestamp }, 1234567890);
    }

    #[test]
    fn test_encode_multiple_fills() {
        let trades = vec![
            Trade {
                taker_order_id: 1,
                maker_order_id: 2,
                price: Price(100),
                quantity: 10,
                taker_side: OrderSide::Buy,
                timestamp: 111,
            },
            Trade {
                taker_order_id: 1,
                maker_order_id: 3,
                price: Price(101),
                quantity: 20,
                taker_side: OrderSide::Buy,
                timestamp: 222,
            },
        ];

        let response = Response::Fills {
            engine_seq: 7,
            trades,
        };

        let mut buf = Vec::new();
        encode_response(&response, &mut buf);

        // Should produce two Fill frames back-to-back
        let frame_size = 4 + 1 + size_of::<FillMsg>();
        assert_eq!(buf.len(), frame_size * 2);

        // Verify second frame
        let second_frame = &buf[frame_size..];
        assert_eq!(second_frame[4], MSG_FILL);
        let msg2: FillMsg =
            unsafe { ptr::read_unaligned(second_frame[5..].as_ptr() as *const FillMsg) };
        assert_eq!({ msg2.maker_order_id }, 3);
        assert_eq!({ msg2.price }, 101);
        assert_eq!({ msg2.quantity }, 20);
    }

    #[test]
    fn test_encode_empty_fills() {
        let response = Response::Fills {
            engine_seq: 1,
            trades: vec![],
        };

        let mut buf = Vec::new();
        encode_response(&response, &mut buf);

        // No trades = no frames
        assert!(buf.is_empty());
    }

    #[test]
    fn test_encode_reject() {
        let response = Response::Reject {
            engine_seq: 999,
            client_seq: 5,
            reason: OrderError::OrderNotFound,
        };

        let mut buf = Vec::new();
        encode_response(&response, &mut buf);

        assert_eq!(buf[4], MSG_REJECT);
        let msg: RejectMsg =
            unsafe { ptr::read_unaligned(buf[5..].as_ptr() as *const RejectMsg) };
        assert_eq!({ msg.engine_seq }, 999);
        assert_eq!({ msg.client_seq }, 5);
        assert_eq!({ msg.reason }, 3); // OrderNotFound = 3
    }

    // ========== Full pipeline: encode frame → session parse → decode command ==========

    #[test]
    fn test_session_frame_parsing_integration() {
        // Simulate: write a NewOrder frame into a Session's read buffer, then parse it
        let msg = NewOrderMsg {
            client_seq: 1,
            order_id: 42,
            user_id: 10,
            asset_id: 1,
            price: 10000,
            quantity: 100,
            side: 0,
            order_type: 0,
        };

        let frame = build_frame(MSG_NEW_ORDER, &msg);

        // "Receive" frame into session's read buffer
        let mut session = Session::new(0);
        session.read_buf[..frame.len()].copy_from_slice(&frame);
        session.read_pos = frame.len();

        // Parse and decode
        let (msg_type, payload) = session.try_parse_frame().expect("frame should be complete");
        let cmd = decode_command(msg_type, payload).expect("decode should succeed");

        match cmd {
            Command::AddOrder { client_seq, order } => {
                assert_eq!(client_seq, 1);
                assert_eq!(order.id, 42);
                assert_eq!(order.price.0, 10000);
            }
            _ => panic!("expected AddOrder"),
        }

        // Consume and verify buffer is empty
        session.consume_frame();
        assert_eq!(session.read_pos, 0);
        assert!(session.try_parse_frame().is_none());
    }

    #[test]
    fn test_session_multiple_frames_in_buffer() {
        // Two frames back-to-back in the read buffer (simulates TCP coalescing)
        let msg1 = CancelOrderMsg {
            client_seq: 1,
            order_id: 10,
            asset_id: 1,
        };
        let msg2 = CancelOrderMsg {
            client_seq: 2,
            order_id: 20,
            asset_id: 2,
        };

        let frame1 = build_frame(MSG_CANCEL_ORDER, &msg1);
        let frame2 = build_frame(MSG_CANCEL_ORDER, &msg2);

        let mut session = Session::new(0);
        session.read_buf[..frame1.len()].copy_from_slice(&frame1);
        session.read_buf[frame1.len()..frame1.len() + frame2.len()].copy_from_slice(&frame2);
        session.read_pos = frame1.len() + frame2.len();

        // Parse first frame
        let (t1, p1) = session.try_parse_frame().unwrap();
        let cmd1 = decode_command(t1, p1).unwrap();
        session.consume_frame();

        // Parse second frame
        let (t2, p2) = session.try_parse_frame().unwrap();
        let cmd2 = decode_command(t2, p2).unwrap();
        session.consume_frame();

        // Verify
        match cmd1 {
            Command::CancelOrder { client_seq, order_id, .. } => {
                assert_eq!(client_seq, 1);
                assert_eq!(order_id, 10);
            }
            _ => panic!("expected CancelOrder"),
        }
        match cmd2 {
            Command::CancelOrder { client_seq, order_id, .. } => {
                assert_eq!(client_seq, 2);
                assert_eq!(order_id, 20);
            }
            _ => panic!("expected CancelOrder"),
        }

        assert_eq!(session.read_pos, 0);
    }

    #[test]
    fn test_session_partial_frame() {
        // Only write half a frame — try_parse_frame should return None
        let msg = NewOrderMsg {
            client_seq: 1,
            order_id: 1,
            user_id: 1,
            asset_id: 1,
            price: 100,
            quantity: 10,
            side: 0,
            order_type: 0,
        };
        let frame = build_frame(MSG_NEW_ORDER, &msg);
        let half = frame.len() / 2;

        let mut session = Session::new(0);
        session.read_buf[..half].copy_from_slice(&frame[..half]);
        session.read_pos = half;

        assert!(session.try_parse_frame().is_none());
    }

    // ========== Size sanity checks ==========

    #[test]
    fn test_protocol_struct_sizes() {
        // Ensure packed structs have no padding
        assert_eq!(size_of::<FrameHeader>(), 5);    // 4 + 1
        assert_eq!(size_of::<NewOrderMsg>(), 50);    // 8*6 + 1 + 1
        assert_eq!(size_of::<CancelOrderMsg>(), 24); // 8*3
        assert_eq!(size_of::<ModifyOrderMsg>(), 40); // 8*5
        assert_eq!(size_of::<AckMsg>(), 16);         // 8*2
        assert_eq!(size_of::<FillMsg>(), 49);        // 8*5 + 1 + 8
        assert_eq!(size_of::<RejectMsg>(), 17);      // 8*2 + 1
    }
}
