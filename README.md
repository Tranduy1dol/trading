[![codecov](https://codecov.io/gh/Tranduy1dol/trading/graph/badge.svg?token=0EH5wOkx45)](https://codecov.io/gh/Tranduy1dol/trading)

# Benchmarks (Criterion)

The benchmark sets up an initial cache-warmed order book state containing **100 price levels** on the Buy side and **100 price levels** on the Sell side.

Using `cargo bench` isolated on a single physical CPU core, the results consistently achieve **sub-microsecond speeds per core action**.

| Operation | Time | Description |
|---|---|---|
| **Add Taker Order (IOC)** | **~402 ns** | Taker IOC sweeping against the best matching ask level, computing matches and tracking volumes |
| **Add Maker Order (GTC)** | **~191 ns** | Inserting a new passive order deep into the book, allocating from the pool, and updating the hash maps |
| **Cancel Best Bid** | **~58 ns** | O(1) hashmap lookup + double-linked list unlink + O(1) hardware bitmap invalidation to find the next best bid! |

Reproduce locally with core-pinning to isolate OS noise (e.g. Core 2):
```bash
taskset -c 2 cargo bench
```

## Setup & Testing
This project integrates directly into GitHub Actions with Continuous Benchmarking.
<!-- BENCH_START -->
<!-- BENCH_END -->
