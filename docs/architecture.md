# Architecture

## 1. System Overview

A high-performance, low-latency matching engine for limit order books.

The system uses a **thread-per-core** architecture built on **`io_uring`**: network I/O and order matching run on the same pinned CPU core, eliminating inter-thread communication entirely. The domain logic follows **hexagonal (ports & adapters)** principles — the matching engine has zero dependencies on I/O, serialization, or networking.

### Why This Combination Works

The original design used a hybrid **Hexagonal + LMAX** pattern: a multi-threaded gateway forwarding commands over a crossbeam channel to a single-threaded engine consumer. This works, but the channel hop costs ~30ns and forces a context switch.

With `io_uring` and thread-per-core, the architecture **simplifies**:

| Aspect | LMAX (crossbeam channel) | Thread-per-core (io_uring) |
|---|---|---|
| Gateway ↔ Engine | crossbeam channel (~30ns hop) | Direct function call (~0ns) |
| Network I/O | tokio epoll (syscall per I/O op) | io_uring (batched, async syscalls) |
| Thread model | Gateway threads + 1 engine thread | 1 pinned core does everything |
| Cache behavior | Cross-core cache invalidation | All hot data in L1/L2 of one core |
| Determinism | Deterministic within engine thread | Deterministic within the entire loop |

The matching engine processes an order in ~30ns. The network round-trip dominates at ~3–10µs. Putting both on the same thread means:

```
io_uring completion (~1–2µs)
  → FlatBuffers decode (~50ns)
    → exchange.add_order() (~30ns)
  → FlatBuffers encode (~50ns)
→ io_uring submit (~1–2µs)

Total: ~3–5µs wire-to-wire
```

The matching engine never blocks the network loop because it's 50x faster than the I/O.

### Hexagonal Boundaries Survive

The hexagonal pattern doesn't require separate threads — it requires **dependency inversion**. The domain crate still has zero knowledge of `io_uring`, FlatBuffers, or TCP. The boundary is enforced at compile time by Cargo workspace crate dependencies:

```
gateway → application → domain
  ✓           ✓           ✗ (no I/O deps)
```

The "port" is no longer a channel — it's a trait or a direct function call. The adapter is no longer a thread — it's a codec layer. The principle holds; only the mechanism changes.

---

## 2. Architecture Diagram

```mermaid
graph LR
    subgraph Core ["Single Pinned Core"]
        direction TB

        subgraph IO ["io_uring Reactor"]
            RING["Submission/Completion<br/>Queue"]
        end

        subgraph Gateway ["Gateway Layer"]
            CODEC["FlatBuffers Codec"]
            SESS["Session Manager<br/>(seq tracking, auth)"]
        end

        subgraph Engine ["Engine (Domain)"]
            ENG["Exchange → OrderBook<br/>(matching)"]
        end
    end

    subgraph Outbound ["Outbound"]
        MDF["Market Data<br/>Dissemination"]
        JRNL["Journal<br/>(append-only WAL)"]
    end

    CLIENT["TCP Clients"] -->|binary frame| RING
    RING --> CODEC
    CODEC --> SESS
    SESS -->|Command| ENG
    ENG -->|Response| SESS
    SESS --> CODEC
    CODEC --> RING
    RING -->|binary frame| CLIENT

    ENG -->|events| MDF
    ENG -->|events| JRNL
    MDF -->|binary frame| RING
```

---

## 3. Wire Protocol

### Framing

All messages use length-prefixed binary frames over persistent TCP connections:

```
┌──────────┬──────────┬──────────────────────────────┐
│ len (4B) │ type (1B)│ FlatBuffers payload (N bytes)│
└──────────┴──────────┴──────────────────────────────┘
```

- **len**: `u32` little-endian — total frame size excluding the length field itself
- **type**: message type discriminant
- **payload**: FlatBuffers-encoded message body (zero-copy decode)

### Message Types

| Direction | Type | Tag | Description |
|---|---|---|---|
| Client → Engine | `NewOrder` | `0x01` | Place a new limit order |
| Client → Engine | `CancelOrder` | `0x02` | Cancel an existing order |
| Client → Engine | `ModifyOrder` | `0x03` | Modify price and/or quantity |
| Engine → Client | `Ack` | `0x10` | Order accepted, engine seq assigned |
| Engine → Client | `Fill` | `0x11` | Trade execution report |
| Engine → Client | `Reject` | `0x12` | Order rejected with reason |
| Engine → All | `MarketData` | `0x20` | BBO update / level change / trade tick |

### Sequencing

- **Client seq** (`client_seq: u64`): monotonically increasing per session; set by the client
- **Engine seq** (`engine_seq: u64`): globally monotonic; assigned by the engine to every processed command
- On reconnect, client sends its last `engine_seq` → engine replays from journal

---

## 4. Layer Responsibilities

### Domain (`crates/domain`)

Pure Rust. **Zero I/O dependencies.** Only `rustc-hash` for the hash map.

| Module | Responsibility |
|---|---|
| `order_book.rs` | Single-asset matching: add, cancel, modify. Returns `&[Trade]`. |
| `exchange.rs` | Multi-asset router (`asset_id → OrderBook`). All operations return `Result`. |
| `order_pool.rs` | Zero-alloc memory pool — `Vec<Node>` + free-list index stack |
| `order_queue.rs` | Intrusive doubly-linked list per price level |
| `price_level.rs` | `[u64; 16]` bitmap + totals array. O(1) best-price via hardware TZCNT/LZCNT. |
| `market_data.rs` | `MarketDataEvent` enum — emitted by the order book after state changes |

> **Rule**: No `serde`, no `std::io`, no `tokio`, no `async`. If a dependency does I/O, it does not belong here.

### Application (`crates/application`)

Command/response types and the engine entry point.

```rust
pub enum Command {
    NewOrder { client_seq: u64, order: Order },
    CancelOrder { client_seq: u64, asset_id: u64, order_id: u64 },
    ModifyOrder { client_seq: u64, asset_id: u64, order_id: u64, new_price: Price, new_qty: u64 },
}

pub enum Response {
    Ack { engine_seq: u64, client_seq: u64 },
    Fills { engine_seq: u64, trades: Vec<Trade> },
    Reject { engine_seq: u64, client_seq: u64, reason: OrderError },
}
```

In the thread-per-core model, the application layer is a thin function:

```rust
pub fn process(exchange: &mut Exchange, seq: &mut u64, cmd: Command) -> Response {
    *seq += 1;
    match cmd {
        Command::NewOrder { client_seq, order } => {
            match exchange.add_order(order) {
                Ok(trades) => Response::Fills { engine_seq: *seq, trades: trades.to_vec() },
                Err(e) => Response::Reject { engine_seq: *seq, client_seq, reason: e },
            }
        }
        // ...
    }
}
```

### Gateway (`crates/gateway`)

Owns io_uring reactor, TCP session management, and FlatBuffers codec. This is the **only** crate that knows about networking and serialization.

| Module | Responsibility |
|---|---|
| `reactor.rs` | io_uring event loop — accept, read, write, timer |
| `session.rs` | Per-client state: buffer, `client_seq` tracking, auth |
| `codec.rs` | FlatBuffers encode/decode — translates wire bytes ↔ `Command`/`Response` |

---

## 5. Data Flow

### Write Path (New Order)

```mermaid
sequenceDiagram
    participant C as Client
    participant R as io_uring
    participant GW as Codec
    participant E as Engine

    C->>R: TCP frame [NewOrder, client_seq=42]
    R->>GW: completion event + buffer
    GW->>GW: FlatBuffers decode → Command
    GW->>E: process(&mut exchange, cmd)
    E->>E: exchange.add_order(order) → trades
    E->>GW: Response::Fills { engine_seq=1001 }
    GW->>GW: FlatBuffers encode → buffer
    GW->>R: submit write
    R->>C: TCP frame [Fill, engine_seq=1001]

    Note over R,E: All steps on the same pinned core
```

### Market Data Dissemination

```mermaid
sequenceDiagram
    participant E as Engine
    participant GW as Codec
    participant R as io_uring
    participant C1 as Client 1
    participant C2 as Client 2

    E->>E: order matched → MarketDataEvent::BestPriceChanged
    E->>GW: event
    GW->>GW: encode once, reference for all subscribers
    GW->>R: submit write (client 1 fd)
    GW->>R: submit write (client 2 fd)
    R->>C1: TCP frame [MarketData]
    R->>C2: TCP frame [MarketData]

    Note over GW: Single encode, multiple sends (zero-copy)
```

---

## 6. Core Data Structures

### OrderPool (zero-alloc arena)

```
┌──────┬──────┬──────┬──────┬──────┐
│Node 0│Node 1│Node 2│Node 3│ ...  │  ← Vec<Node> (contiguous, pre-allocated)
└──────┴──────┴──────┴──────┴──────┘
free_list: [2, 5, 8]   ← O(1) alloc via pop, O(1) dealloc via push
```

- Pre-allocates capacity at startup — zero heap allocation during trading
- Node stores order data + intrusive linked-list pointers (prev/next indices)

### PriceLevel (bitmap-indexed price array)

```
Index:    [  0  ][  1  ][  2  ] ... [ 999 ]
levels:   [Queue][Queue][Queue] ... [Queue]
totals:   [ 500 ][ 0   ][ 200 ] ... [  0  ]
bitmap:   [1 0 1 0 0 ...] ← 16 × u64, hardware TZCNT for best bid, LZCNT for best ask
```

- Bitmap gives O(1) best-price discovery — a single `TZCNT` instruction
- Each queue is an intrusive doubly-linked list → O(1) insert and cancel

---

## 7. Performance Design

| Technique | Impact | Location |
|---|---|---|
| **io_uring** | Batched async syscalls, no epoll overhead, ~1–2µs per I/O | Gateway reactor |
| **Thread-per-core** | No cross-thread communication, all hot data in L1/L2 | Architecture-wide |
| **FlatBuffers** | Zero-copy decode ~50ns (vs JSON ~1–5µs) | Gateway codec |
| **Single-threaded engine** | No locks, no contention, deterministic ordering | Application layer |
| **u64 bitmap** | O(1) best-price via hardware TZCNT/LZCNT | `price_level.rs` |
| **Vec + free-list** | Zero heap allocation during trading | `order_pool.rs` |
| **Intrusive linked list** | O(1) insert/remove, cache-friendly | `order_queue.rs` |
| **`unsafe get_unchecked`** | Eliminates bounds checks in matching hot loop | `order_book.rs` |
| **`target-cpu=native`** | Enables hardware TZCNT/LZCNT instructions | `.cargo/config.toml` |
| **LTO fat + codegen-units=1** | Maximum cross-crate inlining | `Cargo.toml` |

---

## 8. Evolution Roadmap

The architecture is designed to evolve without rewriting the domain:

```
Current     TCP + tokio (epoll)          ~20–50µs wire-to-wire
   ↓
Phase 1     TCP + io_uring               ~3–10µs wire-to-wire
            FlatBuffers codec
            Thread-per-core reactor
   ↓
Phase 2     AF_XDP (kernel bypass lite)  ~1–3µs wire-to-wire
            eBPF + XDP in NIC driver
            Still uses kernel driver
   ↓
Phase 3     DPDK (full kernel bypass)    ~0.5–1µs wire-to-wire
            Userspace TCP stack
            Dedicated NIC, huge pages
   ↓
Phase 4     FPGA NIC                     ~sub-µs
            Matching logic on hardware
```

Each phase only replaces the **gateway crate**. The domain and application layers are untouched — that's the payoff of hexagonal architecture.

### Current Priorities

- [x] Domain core — matching engine, order pool, price levels
- [ ] Application layer — sequenced commands/responses, `Result` returns for all operations
- [ ] Gateway — io_uring reactor, FlatBuffers codec, TCP session management
- [ ] Journal — append-only WAL for crash recovery and replay
- [ ] Market data — wire `MarketDataEvent` from engine → broadcast to subscribers
- [ ] Admin API — REST endpoints for monitoring (health, metrics, book snapshots)
