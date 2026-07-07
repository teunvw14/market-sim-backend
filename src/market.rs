use std::fmt::Display;

use serde::Serialize;
use serde_repr::Serialize_repr;

use crate::asset::*;
use crate::errors::*;
use crate::order::*;
use crate::orderbook::*;
use crate::util::types::*;

pub type MarketCreationResult = Result<(), MarketCreationError>;

// Order (related) types

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct OrderInsertionEffects {
    // The id assigned to the order
    pub id: usize,
    pub status: OrderExecutionStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum OrderExecutionStatus {
    #[default]
    AwaitingFill,
    PartialFill,
    Filled,
    Killed,
    Cancelled,
}

pub type OrderInsertionResult = Result<OrderInsertionEffects, OrderInsertionError>;
pub type OrderCancellationResult = Result<(), OrderCancellationError>;
pub type OrderModificationResult = Result<(), OrderModificationError>;


#[derive(Debug, Clone)]
pub struct Market {
    pub asset_pair: AssetIdPair,
    pub last_traded_price: Price,
    pub orderbook: Orderbook
}

impl Market {
    pub fn new(asset_pair: AssetIdPair) -> Self {
        Market {
            asset_pair: asset_pair,
            last_traded_price: Price::ZERO,
            orderbook: Orderbook::new(),
        }
    }

    pub fn insert_order(
        &mut self,
        order: OrderInsertion,
        transaction_buf: &mut ObTransactionBuffer,
    ) -> ObOrderInsertionResult {
        let orderbook = &mut self.orderbook;
        let execution_result = orderbook.insert_order(order, transaction_buf);
        if let Ok(execution_result) = &execution_result {
            if let Some(last_traded_price) = execution_result.last_traded_price {
                self.last_traded_price = last_traded_price
            }
        }
        execution_result
    }

    pub fn cancel_order(&mut self, cancellation: OrderCancellation) -> OrderCancellationResult {
        let orderbook = &mut self.orderbook;
        orderbook.cancel_order(cancellation)
    }

    pub fn modify_order(&mut self, modification: OrderModification) -> OrderModificationResult {
        let orderbook = &mut self.orderbook;
        orderbook.modify_order(modification)
    }

    pub fn get_orderbook_size(&self) -> usize {
        let mut result = 0;
        for (_k, v) in &self.orderbook.bids {
            result += v.len();
        }
        for (_k, v) in &self.orderbook.asks {
            result += v.len();
        }
        result
    }

    pub fn get_l1(&self) -> OrderbookL1 {
        self.orderbook.get_l1()
    }

    pub fn get_l2(&self) -> OrderbookL2 {
        self.orderbook.get_l2()
    }
}

impl Display for Market {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pair = &self.asset_pair;
        write!(f, "Market {pair:?}")
    }
}

impl Display for AssetPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let primary_symbol = &self.primary.symbol;
        let secondary_symbol = &self.secondary.symbol;
        write!(f, "{primary_symbol}/{secondary_symbol}")
    }
}

/// Thin wrapper around a vec for faster lookups of key-existence
#[derive(Debug)]
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

    pub fn add_market(&mut self, asset_pair: AssetIdPair) -> MarketCreationResult {
        // Only one market allowed per asset pair
        if self.contains(&asset_pair) {
            return Err(MarketCreationError::MarketAlreadyExists);
        };

        // Create market and push to storing vec
        let market = Market::new(asset_pair);
        self.keys.push(asset_pair);
        self.inner.push(market);
        Ok(())
    }
}
