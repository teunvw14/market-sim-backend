use std::cmp::min;
use std::collections::btree_map::IterMut;
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZero;

use tracing::debug;

use crate::asset::*;
use crate::market::*;
use crate::order::*;
use crate::types::*;

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

#[derive(Debug, Clone, Default)]
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

    pub fn insert_order_limit(
        &mut self,
        mut order: OrderInsertion,
        transaction_buf: &mut ObTransactionBuffer,
    ) -> ObOrderInsertionResult {
        let mut remaining = order.volume;
        let mut last_traded_price = None;

        let side = order.side;
        let price = order.price;

        let prices = match side {
            Side::Ask => &mut self.bids,
            Side::Bid => &mut self.asks,
        };

        let mut iterator = prices.iter_mut();
        let iterator = std::iter::from_fn(move || match side {
            Side::Ask => iterator.next_back(), // prices = self.bids, so get highest
            Side::Bid => iterator.next(),      // prices = self.asks, so get lowest
        });

        // Keep track of how many price levels are consumed (and thus need to be deleted)
        let mut price_level_deletions = 0;

        for (open_order_price, open_orders) in iterator {
            let bid_price_too_high = side == Side::Bid && *open_order_price > price;
            let ask_price_too_low = side == Side::Ask && *open_order_price < price;
            if bid_price_too_high || ask_price_too_low {
                break;
            }

            while let Some(open_order) = open_orders.iter_mut().next() {
                let transaction_volume = min(remaining, open_order.remaining_volume);
                open_order.remaining_volume -= transaction_volume;
                remaining -= transaction_volume;

                // Check for self-transaction
                if order.account_id == open_order.account_id {
                    debug!("Self trade!!!!!");
                    return Err(OrderInsertionError::SelfTrade);
                }

                // Push transaction
                transaction_buf.push(ObTransaction {
                    price: *open_order_price,
                    volume: transaction_volume,
                    taker_side: side,
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
            match side {
                Side::Ask => {
                    // prices = self.bids, so remove highest
                    prices.pop_last();
                }
                Side::Bid => {
                    // prices = self.bids, so remove highest
                    prices.pop_first();
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

    // pub fn insert_order_fill_or_kill(
    //     &mut self,
    //     asset_pair: &AssetIdPair,
    //     order: Order,
    // ) -> OrderInsertionResult {
    //     unimplemented!();
    // }

    // pub fn insert_order_market(
    //     &mut self,
    //     asset_pair: &AssetIdPair,
    //     order: Order,
    // ) -> OrderInsertionResult {
    //     unimplemented!();
    // let side_offers = match order.side {
    //     Side::Ask => &mut self.bids,
    //     Side::Bid => &mut self.asks,
    // };

    // // Check if order can be filled
    // let mut available_volume = 0;
    // for (_price, open_orders) in side_offers.iter() {
    //     for open_order in open_orders {
    //         available_volume += open_order.remaining_volume;
    //         if available_volume > order.volume {
    //             break;
    //         }
    //     }
    //     if available_volume > order.volume {
    //         break;
    //     }
    // }
    // if available_volume < order.volume {
    //     return Err(OrderInsertionError::InadequateVolume);
    // }

    // let (taker_increasing_asset_id, taker_decreasing_asset_id) = match order.side {
    //     Side::Ask => (asset_pair.secondary, asset_pair.primary),
    //     Side::Bid => (asset_pair.primary, asset_pair.secondary),
    // };

    // // Fill order
    // let mut remaining = order.volume;
    // let mut balance_transfers = Vec::new();
    // for (price, open_orders) in side_offers {
    //     while let Some(mut open_order) = open_orders.pop_front() {
    //         let diff = min(remaining, open_order.remaining_volume);
    //         open_order.remaining_volume -= diff;
    //         remaining -= diff;

    //         let primary_change = Balance::from(diff);
    //         let secondary_change = price * (diff as i128);
    //         let (change_decr, change_incr) = match order.side {
    //             Side::Bid => (secondary_change, primary_change),
    //             Side::Ask => (primary_change, secondary_change),
    //         };
    //         // First asset swap
    //         balance_transfers.push(BalanceTransfer {
    //             from_id: order.account_id,
    //             to_id: open_order.original_order.account_id,
    //             asset_id: taker_decreasing_asset_id,
    //             change: change_decr,
    //         });

    //         // Second asset swap
    //         balance_transfers.push(BalanceTransfer {
    //             from_id: open_order.original_order.account_id,
    //             to_id: order.account_id,
    //             asset_id: taker_increasing_asset_id,
    //             change: change_incr,
    //         });

    //         if open_order.remaining_volume > 0 {
    //             open_orders.push_front(open_order);
    //         }

    //         if remaining <= 0 {
    //             break;
    //         }
    //     }
    //     if remaining <= 0 {
    //         break;
    //     }
    // }

    // Ok(OrderExecutionEffects {
    //     status: OrderExecutionStatus::Filled,
    //     balance_transfers,
    // })
    // }
}
