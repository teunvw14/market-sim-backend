# Exchange Design

This document describes the design of the exchange backend (internal).

# Constraints:
Design is optimized for a small number (<100) of connections, low latency and high throughput. 

# Connections

A `TCPListener` accepts connections, and spawns a new `tokio::task` for each. Each connection gets a `mpsc`