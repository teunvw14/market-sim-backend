# TODO

- Add order tracking
  - Allow cancelling orders
  - Allow order modification
- Use buffer for "transmitting" order execution effects. Vector allocations cost a lot of performance.
- Research ring-buffer
- Investigate using a different data structure (than a map) for the Limit Order Book (like simply a vector, maybe even with bids and asks adjacent, and use linear search)