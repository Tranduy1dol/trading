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
### Latest CI Benchmark Run
```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 10 tests
test order_book::tests::test_add_single_maker_order ... ignored
test order_book::tests::test_cancel_order ... ignored
test order_book::tests::test_fok_order_failure_due_to_price ... ignored
test order_book::tests::test_fok_order_failure_due_to_quantity ... ignored
test order_book::tests::test_fok_order_success ... ignored
test order_book::tests::test_ioc_order_does_not_rest ... ignored
test order_book::tests::test_match_full_taker_order ... ignored
test order_book::tests::test_match_partial_taker_order_rests ... ignored
test order_book::tests::test_modify_order ... ignored
test order_book::tests::test_self_trade_prevention ... ignored

test result: ok. 0 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 0.00s

OrderBook_100Level/add_taker_order_ioc
                        time:   [249.34 ns 269.56 ns 290.03 ns]
Found 2 outliers among 100 measurements (2.00%)
  1 (1.00%) high mild
  1 (1.00%) high severe
OrderBook_100Level/add_maker_order
                        time:   [281.56 ns 296.45 ns 309.67 ns]
OrderBook_100Level/cancel_best_bid
                        time:   [156.02 ns 167.33 ns 177.75 ns]
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild


running 16 tests
test codec::tests::test_decode_cancel_order ... ignored
test codec::tests::test_decode_empty_payload ... ignored
test codec::tests::test_decode_modify_order ... ignored
test codec::tests::test_decode_new_order ... ignored
test codec::tests::test_decode_new_order_sell_ioc ... ignored
test codec::tests::test_decode_truncated_payload ... ignored
test codec::tests::test_decode_unknown_msg_type ... ignored
test codec::tests::test_encode_ack ... ignored
test codec::tests::test_encode_empty_fills ... ignored
test codec::tests::test_encode_multiple_fills ... ignored
test codec::tests::test_encode_reject ... ignored
test codec::tests::test_encode_single_fill ... ignored
test codec::tests::test_protocol_struct_sizes ... ignored
test codec::tests::test_session_frame_parsing_integration ... ignored
test codec::tests::test_session_multiple_frames_in_buffer ... ignored
test codec::tests::test_session_partial_frame ... ignored

test result: ok. 0 passed; 0 failed; 16 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

pinned to core 0
replay journal from /tmp/.tmp7jkHTl
replayed 0 commands. current engine_seq 0
wire_to_wire/order_match_roundtrip
                        time:   [54.980 µs 55.866 µs 56.940 µs]
                        thrpt:  [17.562 Kelem/s 17.900 Kelem/s 18.188 Kelem/s]
wire_to_wire/cancel_reject_roundtrip
                        time:   [52.263 µs 53.097 µs 54.044 µs]
                        thrpt:  [18.503 Kelem/s 18.834 Kelem/s 19.134 Kelem/s]
Found 15 outliers among 100 measurements (15.00%)
  1 (1.00%) high mild
  14 (14.00%) high severe

```

<!-- BENCH_END -->
