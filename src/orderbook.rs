use std::cmp::min;
use std::collections::btree_map::IterMut;
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZero;

use crate::asset::*;
use crate::order::*;
use crate::types::*;

#[derive(Debug, Clone)]
pub struct OrderbookEntry {
    pub order_id: OrderId,
    pub account_id: AccountId,
    pub remaining_volume: Volume,
}

impl From<Order> for OrderbookEntry {
    fn from(order: Order) -> Self {
        OrderbookEntry {
            order_id: order.id,
            account_id: order.account_id,
            remaining_volume: order.volume,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Orderbook {
    pub bids: BTreeMap<Price, VecDeque<OrderbookEntry>>,
    pub asks: BTreeMap<Price, VecDeque<OrderbookEntry>>,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderInsertionError {
    /// The specified market does not exist.
    MarketDoesNotExist,
    /// The parameters on the order are illegal. Illegal parameters should be caught by the calling frontend if possible.
    IllegalParameters,
    /// The order was killed (only for Fill-or-Kill orders).
    OrderKilled,
    /// There was not enough volume to fill the order (only for market or Fill-or-Kill orders).
    InadequateVolume,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderCancellationError {
    /// The specified order does not exist.
    OrderDoesNotExist,
    /// User was not the one who created the order.
    NotAuthorized,
    /// The specified order was already filled 
    AlreadyFilled,
    /// Market that the Order is registered for (no longer) exists. Should never happen in practice.
    MarketDoesNotExist,
    /// Order cannot be cancelled (because it is not a limit order)
    NotCancellable
}

/// A change in the remaining volume of order with id `id`. Change should always
/// be interpreted as a decrease (negative change)
#[derive(Debug)]
pub struct OrderChange {
    pub id: OrderId,
    pub change: Volume,
}

pub type OrderChangeBuffer = Vec<OrderChange>;

pub struct OrderExecutionEffects {
    pub order_changes: OrderChangeBuffer,
    pub last_traded_price: Option<Price>,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderExecutionStatus {
    AwaitingFill,
    PartialFill,
    Filled,
    Killed,
    Cancelled,
}

pub type OrderInsertionResult = Result<OrderExecutionEffects, OrderInsertionError>;
pub type OrderCancellationResult = Result<(), OrderCancellationError>;

impl Orderbook {
    pub fn new() -> Orderbook {
        Orderbook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn get_best_bid(&mut self) -> Option<OrderbookEntry> {
        let mut orders_entry = self.bids.first_entry()?;
        let orders = orders_entry.get_mut();

        let first_order = orders.pop_front()?;
        Some(first_order)
    }

    pub fn get_best_ask(&mut self) -> Option<OrderbookEntry> {
        let (_price, mut orders) = self.asks.pop_first()?;
        let first_order = orders.pop_front()?;
        Some(first_order)
    }

    pub fn insert_limit_order_no_matching(&mut self, order: Order) {
        let price = order.price;
        let book_side = match order.side {
            Side::Ask => &mut self.asks,
            Side::Bid => &mut self.bids,
        };
        let first_entry_opt = match order.side {
            Side::Ask => book_side.first_entry(),
            Side::Bid => book_side.last_entry(),
        };
        if first_entry_opt.is_none() {
            book_side.insert(price, VecDeque::from([order.into()]));
        } else {
            let mut first_entry = first_entry_opt.unwrap();
            let orders_at_price = first_entry.get_mut();
            orders_at_price.push_back(order.into());
        }
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
                return Ok(())
            }
        }
        Err(OrderCancellationError::AlreadyFilled)
    }

    pub fn insert_order_limit(
        &mut self,
        mut order: Order,
    ) -> OrderInsertionResult {
        let mut remaining = order.volume;
        let mut last_traded_price = None;
        let mut order_change_buf = Vec::new();

        let prices = match order.side {
            Side::Ask => &mut self.bids,
            Side::Bid => &mut self.asks,
        };
        fn next_price_orders<'a>(
            iterator: &'a mut IterMut<Price, VecDeque<OrderbookEntry>>,
            side: Side,
        ) -> Option<(&'a Price, &'a mut VecDeque<OrderbookEntry>)> {
            match side {
                Side::Ask => iterator.next(),
                Side::Bid => iterator.next_back(),
            }
        }

        while let Some((price, open_orders)) = next_price_orders(&mut prices.iter_mut(), order.side)
        {
            let bid_price_too_high = order.side == Side::Bid && *price > order.price;
            let ask_price_too_low = order.side == Side::Ask && *price < order.price;
            if bid_price_too_high || ask_price_too_low {
                break;
            }

            while let Some(open_order) = open_orders.iter_mut().next() {
                let diff = min(remaining, open_order.remaining_volume);
                open_order.remaining_volume -= diff;
                remaining -= diff;

                // First OrderChange
                order_change_buf.push(OrderChange {
                    id: open_order.order_id,
                    change: diff,
                });

                if open_order.remaining_volume == 0 {
                    open_orders.pop_front();
                }

                if remaining <= 0 {
                    last_traded_price = Some(*price);
                    break;
                }
            }
            if open_orders.is_empty() {
                // Clean up orderbook
                match order.side {
                    Side::Ask => {
                        prices.pop_first();
                    }
                    Side::Bid => {
                        prices.pop_last();
                    }
                }
            }
            if remaining <= 0 {
                break;
            }
        }

        // Second order change
        let volume_filled = order.volume - remaining;
        order_change_buf.push(OrderChange {
            id: order.id,
            change: volume_filled,
        });
 
            // Enter limit order into orderbook
        order.volume = remaining;
        self.insert_limit_order_no_matching(order);
        Ok(OrderExecutionEffects {
            order_changes: order_change_buf,
            last_traded_price,
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
