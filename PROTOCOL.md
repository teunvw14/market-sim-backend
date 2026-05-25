# Exchange Protocol

This document defines the protocol for communicating with the exchange. 

# Transport Layer (TCP)

We use TCP for connecting to the server. Connections are unencrypted. 

# Presentation Layer / Data Format (MessagePack)

Messages are (de)serialized with [MessagePack](https://msgpack.org/).

# Message Types

## Client-Generated Message Types

### CreateAccount -> Result<Id>

Creates an account on the exchange, creating balances. Returns `Ok(id)` where `id` is the account id (which is required for any other request).

### GetAssets -> Vec<Asset>

### GetMarkets -> Vec<Market>

### GetBalance(AccountId, AssetId) -> Balance

### GetAllBalances(AccountId, AssetId) -> Vec<(AssetId, Balance)>

### InsertOrder(AccountId,...) -> Result<OrderId>

### ModifyOrder(AccountId, OrderId, volume) -> Result(())

Request to cancel order `id`

### CancelOrder (id)

Request to cancel order `id`

## Server-Generated Message Types

### Result<T>

Either Ok(t: T), or Err .

