# 📈 Market Sim Backend 📉

This is the backend for my market simulation. The core is a fast matching engine handling up to **20M orders/second** on a single CPU core. It processes [MessagePack](https://msgpack.org/index.html)-encoded commands received from connections over TCP. It supports inserting multiple order types (Limit, Market, FillOrKill), cancelling and modifying orders. Bulk orders are also supported. It's optimized for handling a small number of connections sending a large number of orders. All this powered by the [Tokio](https://tokio.rs/) runtime. You can see it live [here](https://teunvanwezel.nl/market-sim).

For more information about the design of the exchange server, check out `exchange-design.md`

For more information about the MessagePack based command encoding / framing, check out `command-framing.md`. 

# Frontend

The code for the frontend, i.e. the website (written with Svelte), can be found [here](https://github.com/teunvw14/market-sim-frontend/tree/main).
