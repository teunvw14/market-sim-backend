# Exchange Design

This document describes the design of the exchange backend (internal).

# Constraints:
Design is optimized for a small number (<100) of connections, low latency and high throughput. Though cancellations and modifications are supported, 

# Connections

A `TCPListener` accepts connections, and spawns a new `tokio::task` for each. Clients send `CommandBuffer`s encoded with MessagePack, prepended with message length for framing (also see `command-framing.md`). Each connection gets an `ExchangeClient` object through which commands can be sent. These structs are simply around a `MPSC` transmitter object - the receiver is held by the Exchange, which is ran on a single thread.

# Exchange Threading

The `Exchange` object runs on a single thread. The reasons for this are as follows:
1. Multithreaded designs increase code complexity *a lot*. And when I say a lot, I really mean a lot a lot. It's not just that single threaded code is easier to understand - multithreaded code quickly transforms into an unorganized, ugly monster. It really is miserable to work with. That would be worth it, if it was a lot faster, but:
2. A multithreaded design only speed up code marginally. In experiments, the maximal speedup from a multithreaded design was around 2x - on a 32 core, 64 thread machine. Because every transaction results in a balance change in the exchange's `BalanceBook`, work cannot be completely parallelized. (Note that two distinct markets (0, 1), (1, 2) may affect the balance of the same asset (1), so that some sort of locking mechanism is still required if one tried to parallelize orders across markets.) As a result, [Amdahls Law](https://en.wikipedia.org/wiki/Amdahl%27s_law) applies.

