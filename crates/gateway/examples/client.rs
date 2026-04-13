//! End-to-end test client for the trading engine gateway.
//!
//! Usage:
//!   1. Start the engine:  cargo run -p gateway
//!   2. Run this client:   cargo run -p gateway --example client
//!
//! The client sends a maker sell order, then a taker buy order that matches,
//! and verifies the Fill response comes back with correct data.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::ptr;
use std::thread;
use std::time::Duration;

use gateway::protocol::*;

// ─── helpers ──────────────────────────────────────────────

fn write_frame<T: Sized>(buf: &mut Vec<u8>, msg_type: u8, msg: &T) {
    let payload_size = size_of::<T>();
    let len = (1 + payload_size) as u32;

    buf.extend_from_slice(&len.to_le_bytes());
    buf.push(msg_type);

    let ptr = msg as *const T as *const u8;
    let bytes = unsafe { std::slice::from_raw_parts(ptr, payload_size) };
    buf.extend_from_slice(bytes);
}

fn send_msg<T: Sized>(stream: &mut TcpStream, msg_type: u8, msg: &T) {
    let mut buf = Vec::new();
    write_frame(&mut buf, msg_type, msg);
    stream.write_all(&buf).expect("failed to write");
    stream.flush().expect("failed to flush");
}

fn read_response(stream: &mut TcpStream) -> (u8, Vec<u8>) {
    // Read the 4-byte length prefix
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .expect("failed to read length");
    let len = u32::from_le_bytes(len_buf) as usize;

    // Read the type + payload
    let mut payload = vec![0u8; len];
    stream
        .read_exact(&mut payload)
        .expect("failed to read payload");

    let msg_type = payload[0];
    let body = payload[1..].to_vec();
    (msg_type, body)
}

// ─── tests ────────────────────────────────────────────────

fn test_order_matching(stream: &mut TcpStream) {
    println!("--- Test: Order Matching ---");

    // 1. Send a maker SELL order: 100 units @ price 10010
    //    This will rest on the book. The engine returns Fills{trades:[]},
    //    which encodes to zero bytes (no response sent over the wire).
    let sell = NewOrderMsg {
        client_seq: 1,
        order_id: 1,
        user_id: 100,
        asset_id: 1,
        price: 10010,
        quantity: 100,
        side: 1,       // Sell
        order_type: 0, // GTC
    };
    send_msg(stream, MSG_NEW_ORDER, &sell);
    println!("  Sent: SELL 100 @ 10010 (maker, will rest)");

    // Small delay so the reactor processes the first order
    thread::sleep(Duration::from_millis(10));

    // 2. Send a taker BUY order: 50 units @ price 10010
    //    This will match against the resting sell → produces a Fill
    let buy = NewOrderMsg {
        client_seq: 2,
        order_id: 2,
        user_id: 200,
        asset_id: 1,
        price: 10010,
        quantity: 50,
        side: 0,       // Buy
        order_type: 0, // GTC
    };
    send_msg(stream, MSG_NEW_ORDER, &buy);
    println!("  Sent: BUY 50 @ 10010 (taker, will match)");

    // 3. Read the Fill response
    let (msg_type, body) = read_response(stream);
    assert_eq!(msg_type, MSG_FILL, "expected Fill response");

    let fill: FillMsg = unsafe { ptr::read_unaligned(body.as_ptr() as *const FillMsg) };

    println!("  Received FILL:");
    println!("    engine_seq:     {}", { fill.engine_seq });
    println!("    taker_order_id: {}", { fill.taker_order_id });
    println!("    maker_order_id: {}", { fill.maker_order_id });
    println!("    price:          {}", { fill.price });
    println!("    quantity:       {}", { fill.quantity });
    println!("    taker_side:     {} (0=Buy)", { fill.taker_side });
    println!("    timestamp:      {}", { fill.timestamp });

    assert_eq!({ fill.taker_order_id }, 2);
    assert_eq!({ fill.maker_order_id }, 1);
    assert_eq!({ fill.price }, 10010);
    assert_eq!({ fill.quantity }, 50);
    assert_eq!({ fill.taker_side }, 0); // Buy

    println!("  ✓ Order matching works!\n");
}

fn test_cancel_nonexistent(stream: &mut TcpStream) {
    println!("--- Test: Cancel Non-existent Order ---");

    // Cancel an order on an asset that has no book → Reject(AssetNotFound)
    let cancel = CancelOrderMsg {
        client_seq: 3,
        order_id: 9999,
        asset_id: 9999,
    };
    send_msg(stream, MSG_CANCEL_ORDER, &cancel);
    println!("  Sent: CANCEL order_id=9999 asset_id=9999");

    let (msg_type, body) = read_response(stream);
    assert_eq!(msg_type, MSG_REJECT, "expected Reject response");

    let reject: RejectMsg = unsafe { ptr::read_unaligned(body.as_ptr() as *const RejectMsg) };

    println!("  Received REJECT:");
    println!("    engine_seq: {}", { reject.engine_seq });
    println!("    client_seq: {}", { reject.client_seq });
    println!("    reason:     {} (4=AssetNotFound)", { reject.reason });

    assert_eq!({ reject.client_seq }, 3);
    assert_eq!({ reject.reason }, 4); // AssetNotFound

    println!("  ✓ Reject works!\n");
}

fn test_cancel_existing(stream: &mut TcpStream) {
    println!("--- Test: Cancel Existing Order ---");

    // The sell order from test_order_matching still has 50 remaining units (order_id=1)
    let cancel = CancelOrderMsg {
        client_seq: 4,
        order_id: 1,
        asset_id: 1,
    };
    send_msg(stream, MSG_CANCEL_ORDER, &cancel);
    println!("  Sent: CANCEL order_id=1 asset_id=1");

    let (msg_type, body) = read_response(stream);
    assert_eq!(msg_type, MSG_ACK, "expected Ack response");

    let ack: AckMsg = unsafe { ptr::read_unaligned(body.as_ptr() as *const AckMsg) };

    println!("  Received ACK:");
    println!("    engine_seq: {}", { ack.engine_seq });
    println!("    client_seq: {}", { ack.client_seq });

    assert_eq!({ ack.client_seq }, 4);

    println!("  ✓ Cancel existing order works!\n");
}

fn main() {
    let addr = "127.0.0.1:9999";
    println!("Connecting to {}...\n", addr);

    let mut stream = TcpStream::connect(addr)
        .expect("Failed to connect. Is the engine running? Start it with: cargo run -p gateway");

    // Set a read timeout so we don't hang forever on bugs
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    test_order_matching(&mut stream);
    test_cancel_nonexistent(&mut stream);
    test_cancel_existing(&mut stream);

    println!("═══════════════════════════════════");
    println!("  All tests passed!");
    println!("═══════════════════════════════════");
}
