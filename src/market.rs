use fixed::types::*;
use tokio::sync::mpsc;

use std::cmp::min;
use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt::Display;
use std::mem;

use crate::asset::*;
use crate::order::*;
use crate::orderbook::*;
use crate::types::*;

#[derive(Debug)]
pub struct Market {
    pub asset_pair: AssetIdPair,
    pub last_traded_price: Price,
    pub orderbook: Orderbook,
}

impl Market {
    pub fn run(mut self, mut rx: mpsc::Receiver<>) {

    }

    pub fn new(asset_pair: &AssetIdPair) -> Self {
        Market {
            asset_pair: asset_pair.clone(),
            last_traded_price: Price::ZERO,
            orderbook: Orderbook::new(),
        }
    }

    pub fn execute_order(
        &mut self,
        order: Order,
        order_change_buf: &mut Vec<OrderChange>,
    ) -> OrderExecutionResult {
        let orderbook = &mut self.orderbook;
        let execution_result = match order.order_type {
            OrderType::Limit => orderbook.insert_order_limit(order, order_change_buf),
            OrderType::FillOrKill => {
                orderbook.insert_order_fill_or_kill(&self.asset_pair, order, order_change_buf)
            }
            OrderType::Market => {
                orderbook.insert_order_market(&self.asset_pair, order, order_change_buf)
            }
        };
        if execution_result.is_ok() {
            if let Some(last_traded_price) = execution_result.as_ref().unwrap().last_traded_price {
                self.last_traded_price = last_traded_price
            }
        }

        execution_result
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
