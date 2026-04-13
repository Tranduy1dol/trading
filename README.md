[![codecov](https://codecov.io/gh/Tranduy1dol/trading/graph/badge.svg?token=0EH5wOkx45)](https://codecov.io/gh/Tranduy1dol/trading)

# Performance Benchmarks

### 1. What is Being Benchmarked?
The engine uses `criterion` to measure performance across two critical execution layers:
*   **Core Domain (In-Memory)**: Measures raw data structure speeds for placing `Maker` orders, executing `Taker` cross-matches, and handling `Cancel` operations directly in the L3 order book.
*   **Wire-to-Wire Latency**: Measures the full asynchronous TCP round-trip lifecycle (`client TCP write` → `io_uring read` → `decode` → `match` → `encode` → `io_uring write` → `client TCP read`).

### 2. How the Benchmarks Work
*   **Domain**: We pre-allocate an initial cache-warmed order book state containing 100 deep price levels on both the Buy and Sell sides to simulate realistic traversal overhead.
*   **Gateway**: Pings structured `NewOrderMsg` memory blocks over `TCP_NODELAY` loopback sockets, tracking the exact `Instant::now()` until the corresponding `FillMsg` is read from the socket.

To reproduce identical results locally (with hardware core-pinning enabled to isolate OS scheduler noise):
```bash
taskset -c 2 cargo bench
```

### 3. Continuous Integration Results
These are the live benchmark results generated automatically by the latest GitHub Actions CI run:
<!-- BENCH_START -->
| Benchmark Operation | Median Time |
|---|---|
| `OrderBook_100Level/add_taker_order_ioc` | **241.21 ns** |
| `OrderBook_100Level/add_maker_order` | **281.13 ns** |
| `OrderBook_100Level/cancel_best_bid` | **149.62 ns** |
| `wire_to_wire/order_match_roundtrip` | **32.300 µs** |
| `wire_to_wire/cancel_reject_roundtrip` | **31.109 µs** |
<!-- BENCH_END -->
