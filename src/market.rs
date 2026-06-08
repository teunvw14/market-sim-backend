use std::fmt::Display;
use tokio::sync::mpsc;

use crate::asset::*;
use crate::order::*;
use crate::orderbook::*;
use crate::types::*;

#[derive(Debug, Clone, Default)]
pub struct Market {
    pub asset_pair: AssetIdPair,
    pub last_traded_price: Price,
    pub orderbook: Orderbook,
    session_orders: Vec<(OrderId, OrderInsertion)>,
}

impl Market {
    pub fn new(asset_pair: AssetIdPair) -> Self {
        Market {
            asset_pair: asset_pair,
            last_traded_price: Price::ZERO,
            orderbook: Orderbook::new(),
            session_orders: Vec::new(),
        }
    }

    pub fn insert_order(&mut self, order: OrderInsertion) -> OrderInsertionResult {
        let orderbook = &mut self.orderbook;
        let execution_result = match order.order_type {
            OrderType::Limit => orderbook.insert_order_limit(order),
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

    pub fn cancel_order<T: Into<OrderCancellation>>(
        &mut self,
        cancellation: T,
    ) -> OrderCancellationResult {
        let cancellation = cancellation.into();
        let orderbook = &mut self.orderbook;
        orderbook.cancel_order(cancellation)
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
