use std::fmt::Display;

use serde::Serialize;
use thiserror::Error;

use crate::asset::*;
use crate::exchange::CommandResult;
use crate::order::*;
use crate::orderbook::*;
use crate::types::*;

// Errors

#[derive(Error, Debug, Clone, Copy, Serialize)]
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

// Order (related) types

#[derive(Debug, Clone, Copy, Serialize)]
pub struct OrderInsertionEffects {
    // The id assigned to the order
    pub id: usize,
    pub status: OrderExecutionStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
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

#[derive(Debug, Clone, Copy, Serialize)]
pub enum OrderInsertionError {
    /// The specified market does not exist.
    MarketDoesNotExist,
    /// The parameters on the order are illegal. Illegal parameters should be caught by the calling frontend if possible.
    IllegalParameters,
    /// The order was killed (only for Fill-or-Kill orders).
    OrderKilled,
    /// There was not enough volume to fill the order (only for market or Fill-or-Kill orders).
    InadequateVolume,
    /// The insertion would result in a self-trade
    SelfTrade,
    /// Other (should never occur)
    Other,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum OrderCancellationError {
    /// The specified order does not exist.
    OrderDoesNotExist,
    /// User was not the one who created the order.
    Unauthorized,
    /// The specified order was already filled
    AlreadyFilled,
    /// The specified order was already cancelled
    AlreadyCancelled,
    /// Market that the Order is registered for (no longer) exists. Should never happen in practice.
    MarketDoesNotExist,
    /// Order cannot be cancelled (because it is not a limit order)
    NotCancellable,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum OrderModificationError {
    /// The specified order does not exist.
    OrderDoesNotExist,
    /// The specified order does not exist.
    AlreadyFilled,
    /// User was not the one who created the order.
    Unauthorized,
    /// Market that the Order is registered for (no longer) exists. Should never happen in practice.
    MarketDoesNotExist,
    /// Specified new volume is not lower than the original volume; needs to be lower.
    VolumeNotLower,
    /// Order could not be found in the Orderbook. Should never happen in practice.
    OrderNotFound,
}

#[derive(Debug, Clone)]
pub struct Market {
    pub asset_pair: AssetIdPair,
    pub last_traded_price: Price,
    pub orderbook: Orderbook,
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
        let execution_result = match order.order_type {
            OrderType::Limit => orderbook.insert_order_limit(order, transaction_buf),
            OrderType::FillOrKill => {
                unimplemented!();
                // orderbook.insert_order_fill_or_kill(&self.asset_pair, order, order_change_buf)
            }
            OrderType::Market => {
                unimplemented!();
                // orderbook.insert_order_market(&self.asset_pair, order, order_change_buf)
            }
        };
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
            return Err(MarketCreationError::MarketAlreadyExists { asset_pair });
        };

        // Create market and push to storing vec
        let market = Market::new(asset_pair);
        self.keys.push(asset_pair);
        self.inner.push(market);
        Ok(())
    }
}
