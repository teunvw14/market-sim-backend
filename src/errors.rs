use serde_repr::Serialize_repr;
use thiserror::Error;

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
    #[error("The provided order insertion parameters are illegal.")]
    IllegalParameters,
    #[error("Fill-or-Kill order was killed due to a lack of liquidity.")]
    OrderKilled,
    #[error("Market order could not be filled due to a lack of liquidity.")]
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
    #[error("Market that the Order is registered for (no longer) exists.")]
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
    #[error("Market that the Order is registered for (no longer) exists.")]
    MarketDoesNotExist,
    #[error("Specified new volume is not lower than the original volume; needs to be lower.")]
    VolumeNotLower,
    #[error("Order could not be found in the Orderbook.")]
    OrderNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum GetOrderbookError {
    #[error("Market that the orderbook was requested for (no longer) exists.")]
    MarketDoesNotExist,
}
