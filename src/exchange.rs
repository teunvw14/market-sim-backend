use core::num;
use std::error::Error;
use std::hash::Hash;
use std::sync::Mutex;
use std::{collections::HashMap, fmt::Display};

use fixed::traits::Fixed;
use thiserror::Error;
use tokio::task::JoinHandle;

use crate::asset::*;
use crate::market::*;
use crate::order::*;
use crate::orderbook::*;
use crate::types::*;

// Errors

#[derive(Error, Debug)]
pub enum MarketCreationError {
    #[error("Market for pair {asset_pair:?} already exists.")]
    MarketAlreadyExists { asset_pair: AssetIdPair },
    #[error("Asset {asset:?} is not traded on this exchange.")]
    AssetNotTraded { asset: AssetId },
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

/// Thin wrapper around a vec for faster lookups of key-existence
#[derive(Debug, Clone, Default)]
pub struct Markets {
    pub keys: Vec<AssetIdPair>,
    pub inner: Vec<Market>,
}

impl Markets {
    pub fn new() -> Self {
        Markets {
            keys: Vec::new(),
            inner: Vec::new(),
        }
    }

    pub fn contains(&self, key: &AssetIdPair) -> bool {
        self.keys.contains(key)
    }

    pub fn get(&self, key: &AssetIdPair) -> Option<&Market> {
        let mut result = None;
        for market in &self.inner {
            if market.asset_pair == *key {
                result = Some(market);
            }
        }
        result
    }

    pub fn get_mut(&mut self, key: &AssetIdPair) -> Option<&mut Market> {
        let mut result = None;
        for market in &mut self.inner {
            if market.asset_pair == *key {
                result = Some(market);
            }
        }
        result
    }

    pub fn add_market(&mut self, market: Market) {
        self.keys.push(market.asset_pair);
        self.inner.push(market);
    }
}

#[derive(Debug, Default)]
pub struct Exchange {
    accounts: HashMap<AccountId, Account>,
    balances: BalanceBook,
    traded_assets: HashMap<AssetId, Asset>,
    markets: Markets,
    session_orders: Vec<Order>,
    order_change_buf: Vec<OrderChange>,
}

#[derive(Debug)]
pub struct Account {
    id: AccountId,
}

impl Exchange {
    pub fn new() -> Self {
        Exchange {
            accounts: HashMap::new(),
            balances: BalanceBook::new(),
            traded_assets: HashMap::new(),
            markets: Markets::new(),
            session_orders: Vec::with_capacity(100_000),
            order_change_buf: Vec::with_capacity(100),
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

    pub fn create_market(&mut self, asset_pair: AssetIdPair) -> Result<(), MarketCreationError> {
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
        self.markets.add_market(Market::new(&asset_pair));
        Ok(())
    }

    pub fn get_order(&self, order_id: &OrderId) -> Option<&Order> {
        self.session_orders.get(*order_id)
    }

    pub fn get_order_mut(&mut self, order_id: &OrderId) -> Option<&mut Order> {
        self.session_orders.get_mut(*order_id)
    }

    pub fn get_market(&self, asset_pair: &AssetIdPair) -> Option<&Market> {
        self.markets.get(asset_pair)
    }

    fn get_market_mut(&mut self, asset_pair: &AssetIdPair) -> Option<&mut Market> {
        self.markets.get_mut(asset_pair)
    }

    pub fn get_last_price(&self, asset_pair: AssetIdPair) -> Option<Price> {
        let market = self.markets.get(&asset_pair)?;
        Some(market.last_traded_price)
    }

    fn create_order(
        &mut self,
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
        self.session_orders.push(result);
        result
    }
    // pub fn get_account_mut(&mut self, id: &AccountId) -> Option<&mut Account> {
    //     self.accounts.get_mut(id)
    // }

    pub fn insert_order(
        &mut self,
        account_id: AccountId,
        order_type: OrderType,
        asset_pair: AssetIdPair,
        side: Side,
        volume: Volume,
        price: Price,
    ) -> Result<OrderExecutionStatus, OrderExecutionError> {
        // Check order volume
        // if volume <= 0 {
        //     return Err(OrderExecutionError::IllegalParameters);
        // }

        // Execute order
        let order = self.create_order(account_id, order_type, asset_pair, side, volume, price);
        let market = self
            .markets
            .get_mut(&asset_pair)
            .ok_or(OrderExecutionError::MarketDoesNotExist)?;
        self.order_change_buf.clear();
        let execution_effects = market.insert_order(order, &mut self.order_change_buf)?;

        // let num_accounts = self.accounts.len();
        // let balances = &mut self.balances;
        for order_change in &self.order_change_buf {
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
        }

        Ok(execution_effects.status)
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> OrderCancellationResult {
        let order_ref = self.session_orders.get_mut(order_id)
            .ok_or(OrderCancellationError::OrderDoesNotExist)?;
        
        // Any other limit type is not cancellable
        if order_ref.order_type != OrderType::Limit {
            return Err(OrderCancellationError::NotCancellable)
        }
        
        // Copy order
        let order = order_ref.clone();
        
        let pair = order.pair;
        let market = self.markets.get_mut(&pair)
        .ok_or(OrderCancellationError::MarketDoesNotExist)?;
    
        market.cancel_order(order)?;
    
        order_ref.status = OrderExecutionStatus::Cancelled;

        Ok(())
    }
}
