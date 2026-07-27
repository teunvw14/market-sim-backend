# Exchange Design

This document describes the design of the exchange backend (internal).

# Constraints:
Design is optimized for a small number (<100) of connections, low latency and high throughput. Though cancellations and modifications are supported, 

# Connections

Two types of connections are supported: TCP and WebSocket.


- The TCP server is for clients that want to interact with the exchange. It lets these clients do CRUD-like operations for orders, and exposes market state (orderbooks L1/L2).
- The WebSocket server is exclusively for displaying exchange state in browsers. It does not allow for interacting with the exchange (e.g. orders cannot be placed over WebSocket). The WebSocket server works with a subscribe-by-default model: exchange state / metrics are sent at a constant rate to open connections.

## TCP server description

A `TCPListener` accepts connections, and spawns a new `tokio::task` for each. Clients send `CommandBuffer`s encoded with MessagePack, prepended with message length for framing (also see `command-framing.md`). Each connection gets an `ExchangeClient` object through which commands can be sent. These structs are simply around a `MPSC` transmitter object - the receiver is held by the Exchange, which is ran on a single thread.

## WebSocket server description
A `TCPListener` accepts connections, tries to turn each into a `WebSocketStream`, and then spawns a new `tokio::task` to handle each one. The state of the exchange (see below) is sent at a constant time interval. Connections are never read from. 

### ExchangeState

The `ExchangeState` struct includes all market L1's and a `ExchangeMetrics` object, containing the p50, p99 and p99.9 command processing latency percentiles.

# Exchange Threading

The `Exchange` object runs on a single thread. The reasons for this are as follows:
1. Multithreaded designs increase code complexity *a lot*. And when I say a lot, I really mean a lot a lot. It's not just that single threaded code is easier to understand - multithreaded code quickly transforms into an unorganized, ugly monster. It really is miserable to work with. That would be worth it, if it was a lot faster, but:
2. A multithreaded design only speed up code marginally. In experiments, the maximal speedup from a multithreaded design was around 2x - on a 32 core, 64 thread machine. Because every transaction results in a balance change in the exchange's `BalanceBook`, work cannot be completely parallelized. (Note that two distinct markets (0, 1), (1, 2) may affect the balance of the same asset (1), so that some sort of locking mechanism is still required if one tried to parallelize orders across markets.) As a result, [Amdahls Law](https://en.wikipedia.org/wiki/Amdahl%27s_law) applies.

