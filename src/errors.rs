use serde::Serialize;
use serde_repr::Serialize_repr;
use thiserror::Error;

use crate::{asset::*, util::types::*};

#[derive(Error, Debug, Clone, Copy, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum MarketCreationError {
    #[error("Market for the given pair already exists.")]
    MarketAlreadyExists,
    #[error("One of the specified assets is not traded on this exchange.")]
    AssetNotTraded,
    #[error("There are no market handlers to assign the market to.")]
    NoMarketHandlers,
    #[error("Unknown error occurred creating a market.")]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum OrderInsertionError {
    #[error("The specified market does not exist.")]
    MarketDoesNotExist,
    #[error(
        "The parameters on the order are illegal. Illegal parameters should be caught by the calling frontend if possible."
    )]
    IllegalParameters,
    #[error("The order was killed (only for Fill-or-Kill orders).")]
    OrderKilled,
    #[error(
        "There was not enough volume to fill the order (only for market or Fill-or-Kill orders)."
    )]
    InadequateVolume,
    #[error("The insertion would result in a self-trade")]
    SelfTrade,
    #[error("Other (should never occur)")]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum OrderCancellationError {
    #[error("The specified order does not exist.")]
    OrderDoesNotExist,
    #[error("User was not the one who created the order.")]
    Unauthorized,
    #[error("The specified order was already filled")]
    AlreadyFilled,
    #[error("The specified order was already cancelled")]
    AlreadyCancelled,
    #[error(
        "Market that the Order is registered for (no longer) exists. Should never happen in practice."
    )]
    MarketDoesNotExist,
    #[error("Order cannot be cancelled (because it is not a limit order)")]
    NotCancellable,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum OrderModificationError {
    #[error("The specified order does not exist.")]
    OrderDoesNotExist,
    #[error("The specified order does not exist.")]
    AlreadyFilled,
    #[error("User was not the one who created the order.")]
    Unauthorized,
    #[error(
        "Market that the Order is registered for (no longer) exists. Should never happen in practice."
    )]
    MarketDoesNotExist,
    #[error("Specified new volume is not lower than the original volume; needs to be lower.")]
    VolumeNotLower,
    #[error("Order could not be found in the Orderbook. Should never happen in practice.")]
    OrderNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum GetOrderbookError {
    #[error("Market that the orderbook was requested for (no longer) exists.")]
    MarketDoesNotExist,
}
