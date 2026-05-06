use core::num;
use std::error::Error;
use std::hash::Hash;
use std::{collections::HashMap, fmt::Display};

use fixed::traits::Fixed;
use thiserror::Error;

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

#[derive(Debug)]
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

    pub fn add_account(&mut self, num_accounts: usize, num_assets: usize) {
        for i in 0..num_assets {
            let index = (i + 1) * num_accounts + i;
            self.inner.insert(index, Balance::ZERO);
        }
    }
}

#[derive(Debug)]
pub struct Exchange {
    accounts: HashMap<AccountId, Account>,
    balances: BalanceBook,
    traded_assets: HashMap<AssetId, Asset>,
    markets: HashMap<AssetIdPair, Market>,
    last_order_id: usize,
    orders: Vec<Order>,
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
            markets: HashMap::new(),
            last_order_id: 0,
            orders: Vec::new(),
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

    pub fn add_account(&mut self) -> AccountId {
        let new_id = self.accounts.len() as u32;
        self.accounts.insert(new_id, Account { id: new_id });
        new_id
    }

    pub fn create_market(&mut self, asset_pair: AssetIdPair) -> Result<(), MarketCreationError> {
        // Only one market allowed per asset pair
        if self.markets.contains_key(&asset_pair) {
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
        self.markets.insert(asset_pair, Market::new(&asset_pair));
        Ok(())
    }

    pub fn get_last_price(&self, asset_pair: AssetIdPair) -> Option<Price> {
        let market = self.markets.get(&asset_pair)?;
        Some(market.last_traded_price)
    }

    fn create_order(
        &mut self,
        account_id: AccountId,
        order_type: OrderType,
        side: Side,
        volume: Volume,
        price: Price,
    ) -> Order {
        let new_id = self.last_order_id + 1;
        self.last_order_id = new_id;
        let result = Order {
            id: new_id,
            account_id,
            order_type,
            side,
            volume,
            price,
        };
        self.orders.push(result);
        result
    }

    pub fn get_account_mut(&mut self, id: &AccountId) -> Option<&mut Account> {
        self.accounts.get_mut(id)
    }

    pub fn insert_order(
        &mut self,
        asset_pair: AssetIdPair,
        account_id: AccountId,
        order_type: OrderType,
        side: Side,
        volume: Volume,
        price: Price,
    ) -> Result<OrderExecutionStatus, OrderExecutionError> {
        // Check order volume
        if volume <= 0 {
            return Err(OrderExecutionError::IllegalParameters);
        }
        // Check that market exists
        if !self.markets.contains_key(&asset_pair) {
            return Err(OrderExecutionError::MarketDoesNotExist);
        }

        // Execute order
        let order = self.create_order(account_id, order_type, side, volume, price);
        let market = self.markets.get_mut(&asset_pair).unwrap();
        let execution_effects = market.execute_order(order)?;

        let num_accounts = self.accounts.len();
        let balances = &mut self.balances;
        for balance_transfer in &execution_effects.balance_transfers {
            let asset_id = balance_transfer.asset_id;
            let from = balance_transfer.from_id;
            let to = balance_transfer.to_id;

            let from_balance = balances.get_mut(asset_id, from, num_accounts).unwrap();
            *from_balance -= balance_transfer.change;

            let to_balance = balances.get_mut(asset_id, to, num_accounts).unwrap();
            *to_balance += balance_transfer.change;
        }

        Ok(execution_effects.status)
    }
}
