use std::error::Error;
use std::hash::Hash;
use std::{collections::HashMap, fmt::Display};

use fixed::traits::Fixed;
use fixed::types::*;
use thiserror::Error;

use crate::market::*;

// Errors

#[derive(Error, Debug)]
pub enum MarketCreationError {
    #[error("Market for pair {asset_pair:?} already exists.")]
    MarketAlreadyExists { asset_pair: AssetIdPair },
    #[error("Asset {asset:?} is not traded on this exchange.")]
    AssetNotTraded { asset: AssetId },
}

#[derive(Debug)]
pub struct Exchange {
    accounts: HashMap<AccountId, Account>,
    traded_assets: HashMap<AssetId, Asset>,
    markets: HashMap<AssetIdPair, Market>,
    last_account_id: usize,
    last_asset_id: AssetId,
    last_order_id: usize,
}

pub type AccountId = usize;

#[derive(Debug)]
pub struct Account {
    id: AccountId,
    balances: HashMap<AssetId, Balance>,
}

impl Exchange {
    pub fn new() -> Self {
        Exchange {
            accounts: HashMap::new(),
            traded_assets: HashMap::new(),
            markets: HashMap::new(),
            last_account_id: 0,
            last_order_id: 0,
            last_asset_id: 0,
        }
    }

    pub fn add_asset(&mut self, name: &str, symbol: &str) -> AssetId {
        let new_id = self.last_asset_id + 1;
        let name = name.to_string();
        let symbol = symbol.to_string();
        let new_asset = Asset {
            id: new_id,
            name,
            symbol,
        };
        self.traded_assets.insert(new_id, new_asset.clone());
        self.last_asset_id += 1;
        new_id
    }

    pub fn remove_asset(&mut self, asset_id: AssetId) {
        self.traded_assets.remove(&asset_id);
    }

    pub fn add_account(&mut self) -> usize {
        let new_id = self.last_account_id + 1;
        self.accounts.insert(
            new_id,
            Account {
                id: new_id,
                balances: HashMap::new(),
            },
        );
        self.last_account_id = new_id;
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
        volume: u32,
        price: Price,
    ) -> Order {
        let new_id = self.last_order_id + 1;
        self.last_order_id = new_id;
        Order {
            id: new_id,
            account_id,
            order_type,
            side,
            volume,
            price,
        }
    }

    pub fn insert_order(
        &mut self,
        asset_pair: AssetIdPair,
        account_id: AccountId,
        order_type: OrderType,
        side: Side,
        volume: u32,
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

        for balance_transfer in execution_effects.balance_transfers {
            let asset_id = balance_transfer.asset_id;

            let from = self.accounts.get_mut(&balance_transfer.from_id).unwrap();
            let from_balance = from.balances.entry(asset_id).or_insert(Balance::ZERO);
            *from_balance -= balance_transfer.change;

            let to = self.accounts.get_mut(&balance_transfer.to_id).unwrap();
            let to_balance = to.balances.entry(asset_id).or_insert(Balance::ZERO);
            *to_balance += balance_transfer.change;
        }

        Ok(execution_effects.status)
    }
}
