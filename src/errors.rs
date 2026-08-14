use serde_repr::Serialize_repr;
use thiserror::Error;

#[derive(Error, Debug, Clone, Copy, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum MarketCreationError {
    #[error("Market for the given pair already exists.")]
    MarketAlreadyExists = 0,
    #[error("One of the specified assets is not traded on this exchange.")]
    AssetNotTraded = 1,
    #[error("There are no market handlers to assign the market to.")]
    NoMarketHandlers = 2,
    #[error("Unknown error occurred creating a market.")]
    Other = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum OrderInsertionError {
    #[error("The specified market does not exist.")]
    MarketDoesNotExist = 0,
    #[error("The provided order insertion parameters are illegal.")]
    IllegalParameters = 1,
    #[error("Fill-or-Kill order was killed due to a lack of liquidity.")]
    OrderKilled = 2,
    #[error("Market order could not be filled due to a lack of liquidity.")]
    InadequateVolume = 3,
    #[error("The insertion would result in a self-trade")]
    SelfTrade = 4,
    #[error("Other (should never occur)")]
    Other = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum OrderCancellationError {
    #[error("The specified order does not exist.")]
    OrderDoesNotExist = 0,
    #[error("User was not the one who created the order.")]
    Unauthorized = 1,
    #[error("The specified order was already filled")]
    AlreadyFilled = 2,
    #[error("The specified order was already cancelled")]
    AlreadyCancelled = 3,
    #[error("Market that the Order is registered for (no longer) exists.")]
    MarketDoesNotExist = 4,
    #[error("Order cannot be cancelled (because it is not a limit order)")]
    NotCancellable = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum OrderModificationError {
    #[error("The specified order does not exist.")]
    OrderDoesNotExist = 0,
    #[error("The specified order does not exist.")]
    AlreadyFilled = 1,
    #[error("User was not the one who created the order.")]
    Unauthorized = 2,
    #[error("Market that the Order is registered for (no longer) exists.")]
    MarketDoesNotExist = 3,
    #[error("Specified new volume is not lower than the original volume; needs to be lower.")]
    VolumeNotLower = 4,
    #[error("Order could not be found in the Orderbook.")]
    OrderNotFound = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Error, Serialize_repr)]
#[repr(u8)]
pub enum GetOrderbookError {
    #[error("Market that the orderbook was requested for (no longer) exists.")]
    MarketDoesNotExist = 0,
}
