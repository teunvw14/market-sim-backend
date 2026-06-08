# Notes

Some general notes of things that I need to remember.

- Performance: executing an order and processing the balance changes resulting from it takes about 50 ns on average
- Performance: MPSC + oneshot reply gives latency of 344ns (throughput: 2.9M/s) on a single consumer, max over number of producers (message can also be a buffer of orders/commands so that throughput comes out to around 100M/s)
- Performance: TCP can support up to 2.5 Gb/s (~800MB/s) using a simple async server (using try_read) and async client (4 threads, using write_all)
- Simpler design?
  - Run exchange single-threaded and send commands over a single MPSC channel. 
  - Use tokio runtime on exchange, make each order insertion a task? No -> concurrent access still a problem. What if we solve it with mutexes? Latency is a little over 100 ns, total latency for an order comes out to 150ns, still pretty fast. If locks are on orderbooks (i.e. one lock per market), then one could still scale pretty well. Depends strongly on how much overhead tokio runtime adds. Worth investigating. Also might violate first-come-first-serve assumption.
  - Ideally, the design would not have a "single point of latency" (so *not*: single threaded design / multithreaded but all orders go through a single MPSC channel / multithreaded but each order needs to acquire the same lock (not nearly as bad if lock is per-market)

# New design candidates

**Async Exchange with Mutex Locks on Orderbooks**
Latency: 100 ns

+ Simpler design: exchange doesn't need multithreaded architecture
+ Throughput scales with cores/num markets, since locks are at orderbook level (i.e. one lock per market)
+ Tokio runtime: speed + no overhead from managing both the runtime *and* `std::threads`. 
\- May not scale well
\- Contention on locks will be maximal, large added latency.

**Sync Multithreaded Exchange, Give Every Connection `mpsc::Sender`s for all MarketHandlers**
Design latency: 

Uses MPSC channels to send orders to the relevant `MarketHandler`, which sends a response through a `oneshot` channel.

+ Fast
\- Somewhat complicated design
\- Users need to disconnect to get `Sender`s for new markets
\- Each connection holds a copy of *all* MarketHandler channel `Sender`s