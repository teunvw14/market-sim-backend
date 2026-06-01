# Notes

Some general notes of things that I need to remember.

- MPSC + oneshot reply gives latency of 344ns (throughput: 2.9M/s) on a single consumer, max over number of producers (message can also be a buffer of orders/commands so that throughput comes out to around 100M/s)
