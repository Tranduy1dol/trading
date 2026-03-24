# Benchmarks (Criterion)

The benchmark sets up an initial cache-warmed order book state containing **100 price levels** on the Buy side and **100 price levels** on the Sell side.

Using `cargo bench`, the results consistently achieve **sub-microsecond speeds per core action** on a single thread.

| Operation | Time | Description |
|---|---|---|
| **Add Taker Order (IOC)** | **~519 ns** | Taker IOC sweeping against the best matching ask level, computing matches and tracking volumes |
| **Add Maker Order (GTC)** | **~258 ns** | Inserting a new passive order deep into the book, allocating from the pool, and updating the hash maps |
| **Cancel Best Bid** | **~84 ns** | O(1) hashmap lookup + double-linked list unlink + O(1) hardware bitmap invalidation to find the next best bid! |

Reproduce locally with:
```bash
cargo bench
```
