use std::cmp::min;
use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::errors::*;
use crate::order::OrderType;
use crate::{market::*, order::*, util::types::*};

#[derive(Debug, Clone)]
pub struct OrderbookEntry {
    pub order_id: OrderId,
    pub account_id: AccountId,
    pub remaining_volume: Volume,
}

impl From<OrderInsertion> for OrderbookEntry {
    fn from(order: OrderInsertion) -> Self {
        OrderbookEntry {
            order_id: order.id,
            account_id: order.account_id,
            remaining_volume: order.volume,
        }
    }
}

pub struct ObOrderInsertionEffects {
    pub status: OrderExecutionStatus,
    pub last_traded_price: Option<Price>,
}

/// Orderbook insertion result
pub type ObOrderInsertionResult = Result<ObOrderInsertionEffects, OrderInsertionError>;
/// Orderbook cancellation result
pub type ObOrderCancellationResult = Result<(), OrderCancellationError>;

/// A change in the remaining volume of order with id `id`. Change should always
/// be interpreted as a decrease (negative change)
#[derive(Debug)]
pub struct OrderChange {
    pub id: OrderId,
    pub change: Volume,
}

pub type OrderChangeBuffer = Vec<OrderChange>;

// Orderbook Transaction type
#[derive(Debug)]
/// A transaction between two accounts on a particular pair.
pub struct ObTransaction {
    pub price: Price,
    pub volume: Volume,
    pub taker_side: Side,
    pub order_id_maker: OrderId,
    pub order_id_taker: OrderId,
}

/// A buffer of transactions resulting from an order insertion
pub type ObTransactionBuffer = Vec<ObTransaction>;

/// The aggregate (sum of sizes) of a particular price level. Used in L1 and L2
/// Orderbook views.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PriceLevelAggregate {
    pub price: Price,
    pub volume: Volume,
}

/// An L1 view of an Orderbook (best bid/ask + aggregate size)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrderbookL1 {
    pub best_bid: Option<PriceLevelAggregate>,
    pub best_ask: Option<PriceLevelAggregate>,
}

/// An L2 view of an Orderbook (aggregate size for each price level)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderbookL2 {
    pub asks: Vec<PriceLevelAggregate>,
    pub bids: Vec<PriceLevelAggregate>,
}

pub type GetOrderBookL1Result = Result<OrderbookL1, GetOrderbookError>;
pub type GetOrderBookL2Result = Result<OrderbookL2, GetOrderbookError>;

#[derive(Debug, Clone, Default)]
/// The core Orderbook struct that keeps track of all open orders.
pub struct Orderbook {
    pub bids: BTreeMap<Price, VecDeque<OrderbookEntry>>,
    pub asks: BTreeMap<Price, VecDeque<OrderbookEntry>>,
}

impl Orderbook {
    pub fn new() -> Orderbook {
        Orderbook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Insert an order into the OrderBook without matching (i.e. without generating transactions)
    pub fn insert_limit_order_no_matching(&mut self, order: OrderInsertion) {
        let price = order.price;
        let side = order.side;

        let book_side = match side {
            Side::Ask => &mut self.asks,
            Side::Bid => &mut self.bids,
        };

        match book_side.get_mut(&price) {
            None => {
                book_side.insert(price, VecDeque::from([order.into()]));
            }
            Some(orders_at_price) => {
                orders_at_price.push_back(order.into());
            }
        };
    }

    pub fn cancel_order(&mut self, cancellation: OrderCancellation) -> OrderCancellationResult {
        let side = match cancellation.side {
            Side::Ask => &mut self.asks,
            Side::Bid => &mut self.bids,
        };
        let price = cancellation.price;
        if let Some(orders) = side.get_mut(&price) {
            let mut index = None;
            for (i, order_at_price) in orders.iter().enumerate() {
                if order_at_price.order_id == cancellation.order_id {
                    index = Some(i);
                }
            }
            if index.is_some() {
                orders.remove(index.unwrap());
                return Ok(());
            }
        }
        Err(OrderCancellationError::AlreadyFilled)
    }

    pub fn modify_order(&mut self, modification: OrderModification) -> OrderModificationResult {
        let side = match modification.side {
            Side::Ask => &mut self.asks,
            Side::Bid => &mut self.bids,
        };
        let price = modification.price;

        // Do a linear search over orders at the given price
        let mut result = Err(OrderModificationError::OrderNotFound);
        if let Some(orders) = side.get_mut(&price) {
            let mut remove_index = None;
            for (i, order) in orders.iter_mut().enumerate() {
                if order.order_id == modification.order_id {
                    if modification.volume_reduction >= order.remaining_volume {
                        // Volume reduction leads to an immediate fill - remove from orderbook
                        remove_index = Some(i);
                    } else {
                        order.remaining_volume -= modification.volume_reduction;
                    };
                    result = Ok(());
                    break;
                }
            }
            if let Some(idx) = remove_index {
                orders.remove(idx);
            }
        }
        result
    }

    pub fn insert_order(
        &mut self,
        mut order: OrderInsertion,
        transaction_buf: &mut ObTransactionBuffer,
    ) -> ObOrderInsertionResult {
        // Check volume requirements for FillOrKill and Market orders
        if order.order_type == OrderType::FillOrKill {
            self.enough_volume_fok(order.side, order.volume, order.price)?;
        } else if order.order_type == OrderType::Market {
            self.enough_volume_market(order.side, order.volume)?;
        }

        let mut remaining = order.volume;
        let mut last_traded_price = None;

        // Define the side of `prices` for readability: the side of the prices
        // we want to iterate over is the opposite of the order side.
        let prices_side = match order.side {
            Side::Ask => Side::Bid, // if the order is an ask, iterate over bids
            Side::Bid => Side::Ask, // if the order is a bid, iterate over asks
        };
        let prices = match prices_side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        let mut iterator = prices.iter_mut();
        let iterator = std::iter::from_fn(move || match prices_side {
            Side::Ask => iterator.next(),      // get lowest ask
            Side::Bid => iterator.next_back(), // get highest bid
        });

        // Keep track of how many price levels are consumed (and thus need to be deleted)
        let mut price_level_deletions = 0;

        let is_bid = order.side == Side::Bid;
        let is_ask = order.side == Side::Ask;
        let is_market_order = order.order_type == OrderType::Market;
        for (open_order_price, open_orders) in iterator {
            if !(is_market_order) {
                let bid_price_too_high = is_bid && *open_order_price > order.price;
                let ask_price_too_low = is_ask && *open_order_price < order.price;
                if bid_price_too_high || ask_price_too_low {
                    break;
                }
            }

            while let Some(open_order) = open_orders.iter_mut().next() {
                let transaction_volume = min(remaining, open_order.remaining_volume);
                open_order.remaining_volume -= transaction_volume;
                remaining -= transaction_volume;

                // Check for self-transaction
                if order.account_id == open_order.account_id {
                    debug!("🚨 Self trade 🚨");
                    return Err(OrderInsertionError::SelfTrade);
                }

                // Push transaction
                transaction_buf.push(ObTransaction {
                    price: *open_order_price,
                    volume: transaction_volume,
                    taker_side: order.side,
                    order_id_maker: open_order.order_id,
                    order_id_taker: order.id,
                });

                if open_order.remaining_volume == 0 {
                    open_orders.pop_front();
                }

                if remaining <= 0 {
                    last_traded_price = Some(*open_order_price);
                    break;
                }
            }

            // Clean up orderbook
            if open_orders.is_empty() {
                price_level_deletions += 1;
            }

            if remaining <= 0 {
                break;
            }
        }

        // Clean up orderbook
        for _ in 0..price_level_deletions {
            match prices_side {
                Side::Ask => {
                    prices.pop_first(); // remove lowest ask
                }
                Side::Bid => {
                    prices.pop_last(); // remove highest bid
                }
            }
        }

        // Enter limit order into orderbook
        let status = if remaining > 0 {
            order.volume = remaining;
            self.insert_limit_order_no_matching(order);
            OrderExecutionStatus::AwaitingFill
        } else {
            OrderExecutionStatus::Filled
        };
        Ok(ObOrderInsertionEffects {
            last_traded_price,
            status,
        })
    }

    /// Check if there's enough volume to fill a Fill-or-Kill order on `side` with `volume`. Returns Ok(()) if
    /// enough volume exists, otherwise returns Err(OrderInsertionError::OrderKilled).
    fn enough_volume_fok(
        &self,
        order_side: Side,
        volume: Volume,
        price: Price,
    ) -> Result<(), OrderInsertionError> {
        let mut available_volume = 0;
        let side_offers = match order_side {
            Side::Ask => &self.bids,
            Side::Bid => &self.asks,
        };
        for (open_orders_price, open_orders) in side_offers.iter() {
            let bid_price_too_high = order_side == Side::Bid && *open_orders_price > price;
            let ask_price_too_low = order_side == Side::Ask && *open_orders_price < price;
            if bid_price_too_high || ask_price_too_low {
                break;
            }
            for open_order in open_orders {
                available_volume += open_order.remaining_volume;
                if available_volume >= volume {
                    return Ok(());
                }
            }
        }
        Err(OrderInsertionError::OrderKilled)
    }

    /// Check if there's enough volume to fill a market order on `side` with `volume`. Returns Ok(()) if
    /// enough volume exists, otherwise returns Err(OrderInsertionError::InadequateVolume).
    fn enough_volume_market(
        &self,
        order_side: Side,
        volume: Volume,
    ) -> Result<(), OrderInsertionError> {
        let mut available_volume = 0;
        let side_offers = match order_side {
            Side::Ask => &self.bids,
            Side::Bid => &self.asks,
        };
        for (_price, open_orders) in side_offers.iter() {
            for open_order in open_orders {
                available_volume += open_order.remaining_volume;
                if available_volume >= volume {
                    return Ok(());
                }
            }
        }
        Err(OrderInsertionError::InadequateVolume)
    }

    pub fn get_l1(&self) -> OrderbookL1 {
        // Get best_bid
        let last_key_value_opt = self.bids.last_key_value();
        let best_bid = match last_key_value_opt {
            None => None,
            Some((price, prices)) => {
                let volume = prices.iter().map(|o| o.remaining_volume).sum();
                Some(PriceLevelAggregate {
                    price: price.clone(),
                    volume,
                })
            }
        };

        // Get best_ask
        let first_key_value_opt = self.asks.first_key_value();
        let best_ask = match first_key_value_opt {
            None => None,
            Some((price, prices)) => {
                let volume = prices.iter().map(|o| o.remaining_volume).sum();
                Some(PriceLevelAggregate {
                    price: price.clone(),
                    volume,
                })
            }
        };

        OrderbookL1 { best_bid, best_ask }
    }

    pub fn get_l2(&self) -> OrderbookL2 {
        // Get bids
        let mut bids = Vec::with_capacity(self.bids.len());
        for (bid_price, bid_orders) in &self.bids {
            let volume = bid_orders.iter().map(|o| o.remaining_volume).sum();
            bids.push(PriceLevelAggregate {
                price: bid_price.clone(),
                volume,
            })
        }

        // Get asks
        let mut asks = Vec::with_capacity(self.asks.len());
        for (ask_price, ask_orders) in &self.asks {
            let volume = ask_orders.iter().map(|o| o.remaining_volume).sum();
            asks.push(PriceLevelAggregate {
                price: ask_price.clone(),
                volume,
            })
        }

        OrderbookL2 { bids, asks }
    }
}
