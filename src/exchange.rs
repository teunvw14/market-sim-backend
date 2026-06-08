use std::collections::VecDeque;
use std::{collections::HashMap};

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::asset::*;
use crate::market::*;
use crate::order::*;
use crate::orderbook::*;
use crate::types::*;

const MPSC_CAPACITY: usize = 32;

// Errors

#[derive(Error, Debug)]
pub enum MarketCreationError {
    #[error("Market for pair {asset_pair:?} already exists.")]
    MarketAlreadyExists { asset_pair: AssetIdPair },
    #[error("Asset {asset:?} is not traded on this exchange.")]
    AssetNotTraded { asset: AssetId },
    #[error("There are no market handlers to assign the market to.")]
    NoMarketHandlers,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BalanceBook {
    inner: Vec<Balance>,
}

impl BalanceBook {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }
    fn get_index(asset_id: AssetId, account_id: AccountId, num_accounts: usize) -> Option<usize> {
        if account_id as usize >= num_accounts {
            return None;
        }
        Some(num_accounts as usize * asset_id as usize + account_id as usize)
    }

    pub fn get(
        &self,
        asset_id: AssetId,
        account_id: AccountId,
        num_accounts: usize,
    ) -> Option<&Balance> {
        let index = BalanceBook::get_index(asset_id, account_id, num_accounts)?;
        self.inner.get(index)
    }

    pub fn get_mut(
        &mut self,
        asset_id: AssetId,
        account_id: AccountId,
        num_accounts: usize,
    ) -> Option<&mut Balance> {
        let index = BalanceBook::get_index(asset_id, account_id, num_accounts)?;
        self.inner.get_mut(index)
    }

    pub fn add_asset(&mut self, num_accounts: usize) {
        for _ in 0..num_accounts {
            self.inner.push(Balance::ZERO);
        }
    }

    pub fn create_account(&mut self, num_accounts: usize, num_assets: usize) {
        for i in 0..num_assets {
            let index = (i + 1) * num_accounts + i;
            self.inner.insert(index, Balance::ZERO);
        }
    }
}

#[derive(Debug)]
/// MarketHandler holds the `Sender`s needed to communicate with a thread managing 
/// certain markets
pub struct MarketHandler {
    tx_order_buf: mpsc::Sender<OrderBufferWithReplyChannel>,
    tx_market: mpsc::Sender<AssetIdPair>
}

impl MarketHandler {
    pub async fn send_order(&self, order: Order) -> OrderInsertionResult {
        let (tx_reply, rx_reply) = oneshot::channel();
        let order_buf = OrderBufferWithReplyChannel { order_buf: vec![order].into(), tx_reply };
        self.tx_order_buf.send(order_buf).await.unwrap();
        rx_reply.await.unwrap().pop_front().unwrap() // TODO: fix all these unwraps
    }
}

/// Holds all the different markets and MarketHandlers
#[derive(Debug, Default)]
pub struct Markets {
    pub keys: Vec<AssetIdPair>,
    pub handlers: Vec<(MarketHandler, Vec<AssetIdPair>)>,
}

impl Markets {
    pub fn new() -> Self {
        Markets {
            keys: Vec::new(),
            handlers: Vec::new(),
        }
    }

    pub fn contains(&self, key: &AssetIdPair) -> bool {
        self.keys.contains(key)
    }

    pub fn get_handler(&self, asset_pair: &AssetIdPair) -> Option<&MarketHandler> {
        for (handler, pairs) in &self.handlers {
            if pairs.contains(asset_pair) {
                return Some(handler);
            }
        }
        None
    }

    pub async fn add_market(&mut self, asset_pair: AssetIdPair) -> Result<(), MarketCreationError> {
        // Find the handler with least number of assigned markets
        let mut min_handler_idx = 0;
        let first = self.handlers.get(0).ok_or(MarketCreationError::NoMarketHandlers)?;
        let mut min_handler = &first.0;
        let mut min_assigned_pairs = first.1.len();
        for (i, (handler, assigned_pairs)) in self.handlers.iter().enumerate() {
            if assigned_pairs.len() < min_assigned_pairs {
                min_handler_idx = i;
                min_assigned_pairs = assigned_pairs.len();
                min_handler = handler;
            }
        }

        min_handler.tx_market.send(asset_pair).await.unwrap();
        self.handlers.get_mut(min_handler_idx).unwrap().1.push(asset_pair);

        Ok(())
    }

    async fn process_messages(mut rx_order_buf: mpsc::Receiver<OrderBufferWithReplyChannel>, mut rx_market: mpsc::Receiver<AssetIdPair>) {
        // Create a buffer to store large number of OrderBuffers into when
        // reading from MPSC channel
        let mut markets: Vec<Market> = Vec::with_capacity(10);
        // Receive first market
        if let Some(first_pair) = rx_market.recv().await {
            println!("Creating new market {first_pair:?}");
            markets.push(Market::new(first_pair));
        }
        loop {
            // Check if handler has been assigned a new market. This is 
            // Clear channel (we don't want to extend it) and receive `OrderBuffer`s
            let mut channel_buf = Vec::with_capacity(MPSC_CAPACITY);
            let n = rx_order_buf.recv_many(&mut channel_buf, MPSC_CAPACITY).await;
            if n == 0 { break; }
            
            // Process orders
            for msg in channel_buf {
                let mut order_buf = msg.order_buf;
                let mut response_buf = VecDeque::with_capacity(order_buf.len());
                while let Some(order) = order_buf.pop_front() {
                    let market_opt = markets.iter_mut().find(|m| m.asset_pair == order.pair);
                    if let Some(market) = market_opt{
                        let insertion_result = market.insert_order(order);
                        response_buf.push_back(insertion_result);
                    } else {
                        response_buf.push_back(Err(OrderInsertionError::MarketDoesNotExist));
                    }
                }
                let _ = msg.tx_reply.send(response_buf);
            }

            if let Ok(pair) = rx_market.try_recv() {
                markets.push(Market::new(pair));
            }
        }
    }

    pub fn spawn_market_handler(&mut self) {
        let (tx_order_buf, rx_order_buf) = mpsc::channel::<OrderBufferWithReplyChannel>(MPSC_CAPACITY);
        let (tx_market, rx_market) = mpsc::channel(MPSC_CAPACITY);
        // Spawn thread that accepts new orders or markets
        tokio::task::spawn(Self::process_messages(rx_order_buf, rx_market));
        self.handlers.push((MarketHandler { tx_order_buf, tx_market }, Vec::new()));
    }
}

#[derive(Debug, Default)]
pub struct Exchange {
    accounts: HashMap<AccountId, Account>,
    balances: BalanceBook,
    traded_assets: HashMap<AssetId, Asset>,
    markets: Markets,
    session_orders: Vec<Order>,
}

#[derive(Debug)]
pub struct Account {
    id: AccountId,
}

impl Exchange {
    pub fn new() -> Self {
        // Spawn at least one market handler
        let mut markets = Markets::new();
        markets.spawn_market_handler();
        Exchange {
            accounts: HashMap::new(),
            balances: BalanceBook::new(),
            traded_assets: HashMap::new(),
            session_orders: Vec::with_capacity(100_000),
            markets
        }
    }

    pub fn add_asset(&mut self, name: &str, symbol: &str) -> AssetId {
        let new_id = self.traded_assets.len() as u32;
        let name = name.to_string();
        let symbol = symbol.to_string();
        let new_asset = Asset {
            id: new_id,
            name,
            symbol,
        };
        self.traded_assets.insert(new_id, new_asset.clone());
        let num_accounts = self.accounts.len();
        self.balances.add_asset(num_accounts);
        new_id
    }

    pub fn remove_asset(&mut self, asset_id: AssetId) {
        self.traded_assets.remove(&asset_id);
    }

    pub fn create_account(&mut self) -> AccountId {
        let accounts = &mut self.accounts;
        let new_id = accounts.len() as u32;
        accounts.insert(new_id, Account { id: new_id });
        new_id
    }

    pub async fn create_market(&mut self, asset_pair: AssetIdPair) -> Result<(), MarketCreationError> {
        // Only one market allowed per asset pair
        if self.markets.contains(&asset_pair) {
            return Err(MarketCreationError::MarketAlreadyExists { asset_pair });
        };
        // Market can only be created for listed assets
        if !self.traded_assets.contains_key(&asset_pair.primary) {
            return Err(MarketCreationError::AssetNotTraded {
                asset: asset_pair.primary,
            });
        }
        if !self.traded_assets.contains_key(&asset_pair.secondary) {
            return Err(MarketCreationError::AssetNotTraded {
                asset: asset_pair.secondary,
            });
        };
        self.markets.add_market(asset_pair).await.unwrap();
        Ok(())
    }

    pub fn get_order(&self, order_id: &OrderId) -> Option<&Order> {
        self.session_orders.get(*order_id)
    }

    pub fn get_order_mut(&mut self, order_id: &OrderId) -> Option<&mut Order> {
        self.session_orders.get_mut(*order_id)
    }

    // pub fn get_last_price(&self, asset_pair: AssetIdPair) -> Option<Price> {
    //     let market = self.markets.get(&asset_pair)?;
    //     Some(market.last_traded_price)
    // }

    // Creates a new order and starts tracking it.
    fn create_order(
        &self,
        account_id: AccountId,
        order_type: OrderType,
        pair: AssetIdPair,
        side: Side,
        volume: Volume,
        price: Price,
    ) -> Order {
        let new_id = self.session_orders.len();
        let status = OrderExecutionStatus::AwaitingFill;
        let result = Order {
            id: new_id,
            account_id,
            order_type,
            pair,
            side,
            volume,
            price,
            status,
        };
        // self.session_orders.push(result);
        result
    }
    // pub fn get_account_mut(&mut self, id: &AccountId) -> Option<&mut Account> {
    //     self.accounts.get_mut(id)
    // }

    /// Insert an order (into the orderbook of the relevant market)
    pub async fn insert_order(
        &self,
        account_id: AccountId,
        order_type: OrderType,
        asset_pair: AssetIdPair,
        side: Side,
        volume: Volume,
        price: Price,
    ) -> Result<OrderId, OrderInsertionError> {
        // Check order volume
        // if volume <= 0 {
        //     return Err(OrderInsertionError::IllegalParameters);
        // }

        // Execute order
        let order = self.create_order(account_id, order_type, asset_pair, side, volume, price);
        let market_handler = self
            .markets
            .get_handler(&asset_pair)
            .ok_or(OrderInsertionError::MarketDoesNotExist)?;
        let execution_effects = market_handler.send_order(order).await;

        // let num_accounts = self.accounts.len();
        // let balances = &mut self.balances;
        // for order_change in &self.order_change_buf {
            // let change_in_primary = Balance::from(order_change.change);
            // let order_id = order_change.id;

            // let order = self.session_orders.get_mut(order_id).unwrap();
            // // order.volume -= change;
            // let asset_pair = order.pair;
            // let asset_id = match order.side {
            //     Side::Ask => asset_pair.primary,
            //     Side::Bid => asset_pair.secondary
            // };
            // let change_in_asset = match order.side {
            //     Side::Ask => change_in_primary,
            //     Side::Bid => change_in_primary * order.price
            // };
            // let balance = balances.get_mut(asset_id, account_id, num_accounts).unwrap();
            // *balance -= change_in_asset;

            // let asset_id = order.pair;
            // let from = balance_transfer.from_id;
            // let to = balance_transfer.to_id;

            // let from_balance = balances.get_mut(asset_id, from, num_accounts).unwrap();
            // *from_balance -= balance_transfer.change;

            // let to_balance = balances.get_mut(asset_id, to, num_accounts).unwrap();
            // *to_balance += balance_transfer.change;
            // let primary_change = Balance::from(diff);
            // let secondary_change = price * (diff as i64);
        // }

        Ok(order.id)
    }

    // pub fn cancel_order(&mut self, order_id: OrderId) -> OrderCancellationResult {
    //     let order_ref = self.session_orders.get_mut(order_id)
    //         .ok_or(OrderCancellationError::OrderDoesNotExist)?;
        
    //     // Any other limit type is not cancellable
    //     if order_ref.order_type != OrderType::Limit {
    //         return Err(OrderCancellationError::NotCancellable)
    //     }
        
    //     // Copy order
    //     let order = order_ref.clone();
        
    //     let pair = order.pair;
    //     let market = self.markets.get_mut(&pair)
    //     .ok_or(OrderCancellationError::MarketDoesNotExist)?;
    
    //     market.cancel_order(order)?;
    
    //     order_ref.status = OrderExecutionStatus::Cancelled;

    //     Ok(())
    // }
}
