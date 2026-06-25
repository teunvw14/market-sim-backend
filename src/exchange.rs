use std::collections::HashMap;
use std::collections::VecDeque;

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::asset::*;
use crate::balance_manager::*;
use crate::market::*;
use crate::order::*;
use crate::types::*;
use crate::market_handler::*;

// Shorthands for disambiguating market_handler::Command[...] and balance_manager::Command[...]
use crate::market_handler as mkth;
use crate::balance_manager as blcm;


// Errors

#[derive(Error, Debug, Clone, Copy)]
pub enum MarketCreationError {
    #[error("Market for pair {asset_pair:?} already exists.")]
    MarketAlreadyExists { asset_pair: AssetIdPair },
    #[error("Asset {asset:?} is not traded on this exchange.")]
    AssetNotTraded { asset: AssetId },
    #[error("There are no market handlers to assign the market to.")]
    NoMarketHandlers,
    #[error("Unknown error occurred creating a market.")]
    Other,
}

pub type MarketCreationResult = Result<(), MarketCreationError>;

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for MarketCreationError {
    fn from(_value: tokio::sync::mpsc::error::SendError<T>) -> Self {
        MarketCreationError::Other
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for MarketCreationError {
    fn from(_value: tokio::sync::oneshot::error::RecvError) -> Self {
        MarketCreationError::Other
    }
}


/// Wrapper around Markets for sending orders to an exchange
pub struct ExchangeClient {
    market_handlers: MarketHandlers,
    balance_manager: BalanceManager
}

impl ExchangeClient {
    pub fn get_handler(&self, asset_pair: &AssetIdPair) -> Option<&MarketHandler> {
        self.market_handlers.get_handler(asset_pair)
    }

    pub async fn insert_order(&self,
        account_id: AccountId,
        order_type: OrderType,
        pair: AssetIdPair,
        side: Side,
        volume: Volume,
        price: Price,
    ) -> OrderInsertionResult {
        let order_insertion_req = OrderInsertionRequest {
            account_id,
            order_type,
            pair,
            side,
            volume,
            price,
        };
        let command = Command::OrderInsert(order_insertion_req);
        let buf = mkth::CommandBuffer { pair, buf: vec![command].into() };
        let result_buf = self.send_market_commands(buf).await;
        match result_buf[0] {
            CommandResult::OrderInsert(result) => {
                result
            }
            _ => {
                // Should never occur
                Err(OrderInsertionError::Other)
            }
        }
    }
    
    pub async fn send_market_commands(&self, command_buf: mkth::CommandBuffer) -> mkth::CommandResultBuffer {
        let handler = self
            .get_handler(&command_buf.pair);
        if let None = handler {
            // TODO: make this neater, so that it doesn't have to return a Vec of errors meant for something else entirely
            return vec![CommandResult::OrderInsert(Err(OrderInsertionError::MarketDoesNotExist)); command_buf.buf.len()].into();
        }
        let handler = handler.unwrap();
        return handler.send_commands(command_buf).await
    }

    pub async fn get_balance(&self, account_id: AccountId, asset_id: AssetId) -> Option<Balance> {
        self.balance_manager.get_balance(account_id, asset_id).await
    }
}

#[derive(Debug)]
pub struct Exchange {
    accounts: HashMap<AccountId, Account>,
    balance_manager: BalanceManager,
    traded_assets: HashMap<AssetId, Asset>,
    market_handlers: MarketHandlers,
}

#[derive(Debug)]
pub struct Account {
    id: AccountId,
}

impl Exchange {
    pub fn new() -> Self {
        // Spawn at least one market handler
        let balance_manager = BalanceManager::new();
        let tx_blcm_command_buf = balance_manager.tx_command_buf.clone();
        Exchange {
            accounts: HashMap::new(),
            balance_manager: balance_manager,
            traded_assets: HashMap::new(),
            market_handlers: MarketHandlers::new(tx_blcm_command_buf),
        }
    }

    pub fn with_market_handlers(mut self, n: usize) -> Self {
        while self.market_handlers.inner.len() < n {
            let tx_balance_manager = self.balance_manager.tx_command_buf.clone();
            self.market_handlers.add_handler(tx_balance_manager);
        }
        self
    }

    pub fn get_client(&self) -> ExchangeClient {
        ExchangeClient {
            market_handlers: self.market_handlers.clone(),
            balance_manager: self.balance_manager.clone()
        }
    }

    pub async fn add_asset(&mut self, name: &str, symbol: &str) -> AssetId {
        let new_id = self.balance_manager.add_asset().await;
        let name = name.to_string();
        let symbol = symbol.to_string();
        let new_asset = Asset {
            id: new_id,
            name,
            symbol,
        };
        self.traded_assets.insert(new_id, new_asset.clone());
        new_id
    }

    pub fn remove_asset(&mut self, asset_id: AssetId) {
        self.traded_assets.remove(&asset_id);
    }

    pub async fn create_account(&mut self) -> AccountId {
        let accounts = &mut self.accounts;
        let new_id = self.balance_manager.add_account().await;
        accounts.insert(new_id, Account { id: new_id });
        new_id
    }

    pub async fn create_market(
        &mut self,
        asset_pair: AssetIdPair,
    ) -> MarketCreationResult {
        // Only one market allowed per asset pair
        if self.market_handlers.contains_market(&asset_pair) {
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
        self.market_handlers.add_market(asset_pair).await
    }

    // pub fn get_last_price(&self, asset_pair: AssetIdPair) -> Option<Price> {
    //     let market = self.markets.get(&asset_pair)?;
    //     Some(market.last_traded_price)
    // }

    // Creates a new order and starts tracking it.
    // fn create_order(
    //     &self,
    //     account_id: AccountId,
    //     order_type: OrderType,
    //     pair: AssetIdPair,
    //     side: Side,
    //     volume: Volume,
    //     price: Price,
    // ) -> OrderInsertion {
    //     let new_id = self.session_orders.len();
    //     let status = OrderExecutionStatus::AwaitingFill;
    //     let result = OrderInsertion {
    //         id: new_id,
    //         account_id,
    //         order_type,
    //         pair,
    //         side,
    //         volume,
    //         price,
    //         status,
    //     };
    //     self.session_orders.push(result);
    //     result
    // }
    // pub fn get_account_mut(&mut self, id: &AccountId) -> Option<&mut Account> {
    //     self.accounts.get_mut(id)
    // }

    // /// Insert an order (into the orderbook of the relevant market)
    // pub async fn insert_order(
    //     &self,
    //     account_id: AccountId,
    //     order_type: OrderType,
    //     asset_pair: AssetIdPair,
    //     side: Side,
    //     volume: Volume,
    //     price: Price,
    // ) -> Result<OrderId, OrderInsertionError> {
    // Check order volume
    // if volume <= 0 {
    //     return Err(OrderInsertionError::IllegalParameters);
    // }

    // Execute order
    // let order = self.create_order(account_id, order_type, asset_pair, side, volume, price);
    // let market_handler = self
    //     .markets
    //     .get_handler(&asset_pair)
    //     .ok_or(OrderInsertionError::MarketDoesNotExist)?;
    // let execution_effects = market_handler.send_order(order).await;

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

    //     Ok(order.id)
    // }

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
