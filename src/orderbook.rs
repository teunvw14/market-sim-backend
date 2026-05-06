use std::cmp::min;
use std::collections::{BTreeMap, VecDeque};

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

#[derive(Debug, Clone)]
pub struct Orderbook {
    pub bids: BTreeMap<Price, VecDeque<OrderbookEntry>>,
    pub asks: BTreeMap<Price, VecDeque<OrderbookEntry>>,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderExecutionError {
    MarketDoesNotExist,
    IllegalParameters, // should never occur in practice, should be caught in frontend
    OrderKilled,
    InadequateVolume,
}

/// A transfer of `change` units of `asset_id` from the `from_id` account to the
/// `to_id` account.
pub struct BalanceTransfer {
    pub from_id: AccountId,
    pub to_id: AccountId,
    pub asset_id: AssetId,
    pub change: Balance,
}

pub struct OrderExecutionEffects {
    pub status: OrderExecutionStatus,
    pub balance_transfers: Vec<BalanceTransfer>,
    pub last_traded_price: Option<Price>,
}

pub enum OrderExecutionStatus {
    AwaitingExecution,
    PartialFill(Volume),
    Filled,
    Cancelled(Volume),
}

pub type OrderExecutionResult = Result<OrderExecutionEffects, OrderExecutionError>;


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
        let orders_at_price_opt = book_side.get_mut(&price);
        match orders_at_price_opt {
            None => {
                book_side.insert(price, VecDeque::from([order.into()]));
            }
            Some(orders_at_price) => {
                orders_at_price.push_back(order.into());
            }
        }
    }

    fn clean_orderbook(&mut self) {
        while let Some((_price, open_orders)) = self.bids.iter().next_back() {
            if open_orders.len() == 0 {
                self.bids.pop_last();
            } else {
                break;
            }
        }
        while let Some((_price, open_orders)) = self.asks.iter().next() {
            if open_orders.len() == 0 {
                self.asks.pop_first();
            } else {
                break;
            }
        }
    }

    pub fn remove_order(&mut self, order: Order) -> OrderExecutionResult {
        let side = match order.side {
            Side::Ask => &mut self.asks,
            Side::Bid => &mut self.bids,
        };
        let price = order.price;
        let mut remaining_volume: Volume = 0;
        if let Some(orders) = side.get_mut(&price) {
            let mut index = None;
            for (i, order_at_price) in orders.iter().enumerate() {
                if order_at_price.order_id == order.id {
                    remaining_volume = order_at_price.remaining_volume;
                    index = Some(i);
                }
            }
            if index.is_some() {
                orders.remove(index.unwrap());
            }
        }
        Ok(OrderExecutionEffects {
            status: OrderExecutionStatus::Cancelled(remaining_volume),
            balance_transfers: Vec::new(),
            last_traded_price: None,
        })
    }

    pub fn insert_order_limit(
        &mut self,
        asset_pair: &AssetIdPair,
        mut order: Order,
    ) -> OrderExecutionResult {
        let mut remaining = order.volume;
        let mut balance_transfers = Vec::new();
        let mut last_traded_price = None;
        if order.side == Side::Bid {
            for (price, open_orders) in self.asks.iter_mut() {
                if *price > order.price {
                    break;
                }
                while let Some(open_order) = open_orders.iter_mut().next() {
                    let diff = min(remaining, open_order.remaining_volume);
                    open_order.remaining_volume -= diff;
                    remaining -= diff;

                    let primary_change = Balance::from(diff);
                    let secondary_change = price * (diff as i64);
                    // First asset swap
                    balance_transfers.push(BalanceTransfer {
                        from_id: open_order.account_id,
                        to_id: order.account_id,
                        asset_id: asset_pair.primary,
                        change: primary_change,
                    });

                    // Second asset swap
                    balance_transfers.push(BalanceTransfer {
                        from_id: order.account_id,
                        to_id: open_order.account_id,
                        asset_id: asset_pair.secondary,
                        change: secondary_change,
                    });

                    if open_order.remaining_volume == 0 {
                        last_traded_price = Some(*price);
                        open_orders.pop_front();
                    }

                    if remaining <= 0 {
                        break;
                    }
                }
                if remaining <= 0 {
                    break;
                }
            }
        } else if order.side == Side::Ask {
            for (price, open_orders) in self.bids.iter_mut().rev() {
                if *price < order.price {
                    break;
                }
                while let Some(open_order) = open_orders.iter_mut().next() {
                    let diff = min(remaining, open_order.remaining_volume);
                    open_order.remaining_volume -= diff;
                    remaining -= diff;

                    let primary_change = Balance::from(diff);
                    let secondary_change = price * (diff as i64);
                    // First asset swap
                    balance_transfers.push(BalanceTransfer {
                        from_id: order.account_id,
                        to_id: open_order.account_id,
                        asset_id: asset_pair.primary,
                        change: primary_change,
                    });

                    // Second asset swap
                    balance_transfers.push(BalanceTransfer {
                        from_id: open_order.account_id,
                        to_id: order.account_id,
                        asset_id: asset_pair.secondary,
                        change: secondary_change,
                    });

                    if open_order.remaining_volume == 0 {
                        open_orders.pop_front();
                    }

                    if remaining <= 0 {
                        last_traded_price = Some(*price);
                        break;
                    }
                }
                if remaining <= 0 {
                    break;
                }
            }
        }

        let result = if remaining > 0 {
            // Enter limit order into orderbook
            let volume_filled = order.volume - remaining;
            order.volume = remaining;
            self.insert_limit_order_no_matching(order);
            Ok(OrderExecutionEffects {
                status: OrderExecutionStatus::PartialFill(volume_filled),
                balance_transfers,
                last_traded_price,
            })
        } else {
            Ok(OrderExecutionEffects {
                status: OrderExecutionStatus::Filled,
                balance_transfers,
                last_traded_price,
            })
        };
        self.clean_orderbook();
        result
    }

    pub fn insert_order_fill_or_kill(
        &mut self,
        asset_pair: &AssetIdPair,
        order: Order,
    ) -> OrderExecutionResult {
        unimplemented!();
    }

    pub fn insert_order_market(
        &mut self,
        asset_pair: &AssetIdPair,
        order: Order,
    ) -> OrderExecutionResult {
        unimplemented!();
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
        //     return Err(OrderExecutionError::InadequateVolume);
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
    }
}