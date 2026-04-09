use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use domain::order::{Order, OrderSide, OrderType};
use domain::order_book::OrderBook;
use domain::price::Price;

fn create_order(id: u64, qty: u64, price: u64, side: OrderSide, typ: OrderType) -> Order {
    Order::new(id, 1, 1, qty, Price(price), side, typ)
}

fn setup_warmed_orderbook(size: usize) -> OrderBook {
    // Large pool size to prevent re-allocations skewing the benchmark
    let mut book = OrderBook::new(1_000_000);

    // Add `size` levels of asks and bids
    for i in 1..=size {
        // Bids: 10499 -> 10400
        book.add_order(create_order(
            i as u64,
            100,
            10500 - i as u64,
            OrderSide::Buy,
            OrderType::GTC,
        ))
        .unwrap();

        // Asks: 10501 -> 10600
        book.add_order(create_order(
            i as u64 + size as u64,
            100,
            10500 + i as u64,
            OrderSide::Sell,
            OrderType::GTC,
        ))
        .unwrap();
    }
    book
}

fn bench_orderbook(c: &mut Criterion) {
    let mut group = c.benchmark_group("OrderBook_100Level");

    group.bench_function("add_taker_order_ioc", |b| {
        b.iter_batched_ref(
            || setup_warmed_orderbook(100),
            |book| {
                // Taker Buy IOC: matches against the best ask (10501)
                let order = create_order(99999, 10, 10501, OrderSide::Buy, OrderType::IOC);
                book.add_order(black_box(order)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("add_maker_order", |b| {
        b.iter_batched_ref(
            || setup_warmed_orderbook(100),
            |book| {
                // Deep bid that won't match anything
                let order = create_order(99999, 10, 10100, OrderSide::Buy, OrderType::GTC);
                book.add_order(black_box(order)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("cancel_best_bid", |b| {
        b.iter_batched_ref(
            || setup_warmed_orderbook(100),
            |book| {
                // ID 1 is the best bid (10499)
                book.cancel_order(black_box(1)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_orderbook);
criterion_main!(benches);
