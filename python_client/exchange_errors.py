from enum import Enum

class MarketCreationError(Enum):
    # Market for the given pair already exists.
    MarketAlreadyExists = 0
    # One of the specified assets is not traded on this exchange.
    AssetNotTraded = 1
    # There are no market handlers to assign the market to.
    NoMarketHandlers = 2
    # Unknown error occurred creating a market.
    Other = 3

class OrderInsertionError(Enum):
    # The specified market does not exist.
    MarketDoesNotExist = 0
    # The provided order insertion parameters are illegal.
    IllegalParameters = 1
    # Fill-or-Kill order was killed due to a lack of liquidity.
    OrderKilled = 2
    # Market order could not be filled due to a lack of liquidity.
    InadequateVolume = 3
    # The insertion would result in a self-trade
    SelfTrade = 4
    # Other (should never occur)
    Other = 5

class OrderCancellationError(Enum):
    # The specified order does not exist.
    OrderDoesNotExist = 0
    # User was not the one who created the order.
    Unauthorized = 1
    # The specified order was already filled
    AlreadyFilled = 2
    # The specified order was already cancelled
    AlreadyCancelled = 3
    # Market that the Order is registered for (no longer) exists.
    MarketDoesNotExist = 4
    # Order cannot be cancelled (because it is not a limit order)
    NotCancellable = 5

class OrderModificationError(Enum):
    # The specified order does not exist.
    OrderDoesNotExist = 0
    # The specified order does not exist.
    AlreadyFilled = 1
    # User was not the one who created the order.
    Unauthorized = 2
    # Market that the Order is registered for (no longer) exists.
    MarketDoesNotExist = 3
    # Specified new volume is not lower than the original volume; needs to be lower.
    VolumeNotLower = 4
    # Order could not be found in the Orderbook.
    OrderNotFound = 5

class GetOrderbookError(Enum):
    # Market that the orderbook was requested for (no longer) exists.
    MarketDoesNotExist = 0

