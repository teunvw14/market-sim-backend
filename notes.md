# Notes

Some general notes of things that I need to remember.

- Performance: MPSC + oneshot reply gives latency of 344ns (throughput: 2.9M/s) on a single consumer, max over number of producers (message can also be a buffer of orders/commands so that throughput comes out to around 100M/s)
- Performance: TCP can support up to 2.5 Gb/s (~800MB/s) using a simple async server (using try_read) and async client (4 threads, using write_all)
- Simpler design? 
  - Run exchange single-threaded and send commands over a single MPSC channel
  - Use tokio runtime on exchange, make each order insertion a task? No -> concurrent access still a problem. What if we solve it with mutexes? Latency is a little over 100 ns, total latency for an order comes out to 150ns, still pretty fast. If locks are on orderbooks (i.e. one lock per market), then one could still scale pretty well. Depends strongly on how much overhead tokio runtime adds. Worth investigating. 