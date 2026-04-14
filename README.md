[![codecov](https://codecov.io/gh/Tranduy1dol/trading/graph/badge.svg?token=0EH5wOkx45)](https://codecov.io/gh/Tranduy1dol/trading)

# ⚡ Ultra-Low-Latency Trading Engine

A single-threaded, zero-copy trading engine built in Rust, designed for **sub-11 µs wire-to-wire latency** on commodity Linux hardware. The system implements a full vertical stack from a bitmap-accelerated L3 order book to an `io_uring`-based network gateway with crash-fault tolerance.

## Key Features

- 🏗️ **Hexagonal Architecture** — Clean separation between Domain, Application, and Gateway layers with compile-time enforced boundaries
- 📖 **L3 Order Book** — Price levels indexed by a hardware-accelerated bitmap for O(1) best-bid/ask lookup and cancel operations
- 🔌 **io_uring Gateway** — Fully asynchronous TCP reactor using Linux `io_uring` for zero-syscall batched I/O
- 📡 **Market Data Broadcast** — Real-time `BboUpdate` fan-out to all connected clients on every book mutation
- 💾 **Write-Ahead Log** — Crash-fault tolerant journal with async `io_uring` persistence and startup replay
- 🛑 **Graceful Shutdown** — `SIGINT`/`SIGTERM` signal handling with journal flush and connection teardown
- 🔒 **Zero-Copy Protocol** — Packed C-repr structs transmitted directly over TCP with no serialization overhead

> 📐 For a deep dive into the system design, data structures, wire protocol, and data flow diagrams, see **[docs/architecture.md](docs/architecture.md)**.

## Getting Started

### Prerequisites
- **Linux** (required for `io_uring`)
- **Rust nightly** toolchain

### Build & Run
```bash
cargo build --release
./target/release/gateway          # Listens on 0.0.0.0:9999
```

### Run Tests
```bash
cargo test --workspace
```

### Run Benchmarks
```bash
# Pin to a single core to isolate OS scheduler noise
taskset -c 2 cargo bench
```

## Performance Benchmarks

### 1. What is Being Benchmarked?
The engine uses `criterion` to measure performance across two critical execution layers:
*   **Core Domain (In-Memory)**: Measures raw data structure speeds for placing `Maker` orders, executing `Taker` cross-matches, and handling `Cancel` operations directly in the L3 order book.
*   **Wire-to-Wire Latency**: Measures the full asynchronous TCP round-trip lifecycle (`client TCP write` → `io_uring read` → `decode` → `match` → `encode` → `io_uring write` → `client TCP read`).

### 2. How the Benchmarks Work
*   **Domain**: We pre-allocate an initial cache-warmed order book state containing 100 deep price levels on both the Buy and Sell sides to simulate realistic traversal overhead.
*   **Gateway**: Pings structured `NewOrderMsg` memory blocks over `TCP_NODELAY` loopback sockets, tracking the exact `Instant::now()` until the corresponding `FillMsg` is read from the socket.

### 3. Continuous Integration Results
These are the live benchmark results generated automatically by the latest GitHub Actions CI run:
<!-- BENCH_START -->

<!-- BENCH_END -->

## License

MIT
