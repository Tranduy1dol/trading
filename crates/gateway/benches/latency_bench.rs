use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use gateway::protocol::*;

// ─── frame helper ─────────────────────────────────────────

fn write_frame<T: Sized>(buf: &mut Vec<u8>, msg_type: u8, msg: &T) {
    let payload_size = size_of::<T>();
    let len = (1 + payload_size) as u32;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.push(msg_type);
    let ptr = msg as *const T as *const u8;
    let bytes = unsafe { std::slice::from_raw_parts(ptr, payload_size) };
    buf.extend_from_slice(bytes);
}

fn read_fill(stream: &mut TcpStream, read_buf: &mut [u8]) {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = u32::from_le_bytes(len_buf) as usize;
    stream.read_exact(&mut read_buf[..len]).unwrap();
}

// ─── server lifecycle ─────────────────────────────────────

fn start_server() {
    thread::spawn(|| {
        gateway::reactor::run("127.0.0.1:19999");
    });
    // Wait for the server to start, especially binding the port and pinning to a core
    thread::sleep(Duration::from_millis(500));
}

// ─── benchmarks ───────────────────────────────────────────

fn bench_wire_to_wire(c: &mut Criterion) {
    start_server();

    // Give it more time and retry mechanism just in case
    let mut stream = loop {
        match TcpStream::connect("127.0.0.1:19999") {
            Ok(s) => break s,
            Err(_) => {
                thread::sleep(Duration::from_millis(100));
            }
        }
    };
    stream.set_nodelay(true).unwrap();

    // Warmup: send 1000 order pairs to warm up allocations
    let mut read_buf = [0u8; 256];
    for i in 0..1000u64 {
        let mut buf = Vec::with_capacity(128);
        let sell = NewOrderMsg {
            client_seq: i * 2,
            order_id: i * 2,
            user_id: 1,
            asset_id: 1,
            price: 10010,
            quantity: 100,
            side: 1,
            order_type: 0,
        };
        write_frame(&mut buf, MSG_NEW_ORDER, &sell);

        let buy = NewOrderMsg {
            client_seq: i * 2 + 1,
            order_id: i * 2 + 1,
            user_id: 2,
            asset_id: 1,
            price: 10010,
            quantity: 100,
            side: 0,
            order_type: 0,
        };
        write_frame(&mut buf, MSG_NEW_ORDER, &buy);

        stream.write_all(&buf).unwrap();
        read_fill(&mut stream, &mut read_buf);
    }

    // Benchmark: order match round-trip
    let mut group = c.benchmark_group("wire_to_wire");
    group.throughput(Throughput::Elements(1));

    let mut order_id = 10_000u64; // offset past warmup IDs

    group.bench_function("order_match_roundtrip", |b| {
        b.iter_custom(|iters| {
            let mut send_buf = Vec::with_capacity(128);
            let start = Instant::now();

            for _ in 0..iters {
                send_buf.clear();

                // Sell (rests on book, no response)
                let sell = NewOrderMsg {
                    client_seq: order_id,
                    order_id,
                    user_id: 1,
                    asset_id: 1,
                    price: 10010,
                    quantity: 100,
                    side: 1,
                    order_type: 0,
                };
                write_frame(&mut send_buf, MSG_NEW_ORDER, &sell);

                // Buy (matches sell, produces Fill)
                let buy = NewOrderMsg {
                    client_seq: order_id + 1,
                    order_id: order_id + 1,
                    user_id: 2,
                    asset_id: 1,
                    price: 10010,
                    quantity: 100,
                    side: 0,
                    order_type: 0,
                };
                write_frame(&mut send_buf, MSG_NEW_ORDER, &buy);

                // Send both, read fill
                stream.write_all(&send_buf).unwrap();
                read_fill(&mut stream, &mut read_buf);

                order_id += 2;
            }

            start.elapsed()
        });
    });

    // Benchmark: cancel reject round-trip (pure network overhead, no matching)
    group.bench_function("cancel_reject_roundtrip", |b| {
        b.iter_custom(|iters| {
            let mut send_buf = Vec::with_capacity(64);
            let start = Instant::now();

            for _ in 0..iters {
                send_buf.clear();

                // Cancel non-existent order → Reject response
                let cancel = CancelOrderMsg {
                    client_seq: order_id,
                    order_id: 999_999_999,
                    asset_id: 999_999_999,
                };
                write_frame(&mut send_buf, MSG_CANCEL_ORDER, &cancel);

                stream.write_all(&send_buf).unwrap();
                read_fill(&mut stream, &mut read_buf);

                order_id += 1;
            }

            start.elapsed()
        });
    });

    group.finish();
}

criterion_group!(benches, bench_wire_to_wire);
criterion_main!(benches);
