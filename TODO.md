# TODO

- BUG: 0 volume orders remaining in the orderbook
- Use pre-determined price levels
- Research ring-buffer
- Investigate using a different data structure (than a map) for the Limit Order Book (like simply a vector, maybe even with bids and asks adjacent, and use linear search)
- Fix high-latency order-insertions
- Create exchange config type or exchange builder for configurability
- Switch to anyhow
- Maybe: write completed orders to disk? So that memory is saved? Does that really save memory (you would still need to keep track of which orders are on disk). Better idea(?): write all orders to disk at the end of session.
- Maybe: Add transaction logs (save to disk)?

# Done

- Add order tracking
  - Allow order modification (with auth, " ")
  - Allow cancelling orders (with auth, is the person requesting cancellation the person who placed the order?)
- Block self-trades on insertion v
- add OrderInsert type which elides the status field present on Order
- Use buffer for "transmitting" order execution effects. Vector allocations cost a lot of performance.
