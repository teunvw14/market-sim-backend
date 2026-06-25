use std::fmt::Display;

use crate::asset::*;
use crate::order::*;
use crate::orderbook::*;
use crate::types::*;

#[derive(Debug, Clone, Copy)]
pub struct OrderInsertionEffects {
    // The id assigned to the order
    pub id: usize,
    pub status: OrderExecutionStatus,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum OrderExecutionStatus {
    #[default]
    AwaitingFill,
    PartialFill,
    Filled,
    Killed,
    Cancelled,
}

pub type OrderInsertionResult = Result<OrderInsertionEffects, OrderInsertionError>;
pub type OrderCancellationResult = Result<(), OrderCancellationError>;
pub type OrderModificationResult = Result<(), OrderModificationError>;

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
    /// Other (should never occur)
    Other,
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
    NotCancellable,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderModificationError {
    /// The specified order does not exist.
    AlreadyFilled,
    /// User was not the one who created the order.
    NotAuthorized,
}

#[derive(Debug, Clone)]
pub struct Market {
    pub asset_pair: AssetIdPair,
    pub last_traded_price: Price,
    pub orderbook: Orderbook,
    pub session_orders: Vec<OrderInserted>,
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

    fn create_order(&mut self, request: OrderInsertionRequest) -> OrderInserted {
        let new_id = self.session_orders.len();
        let order = request.into_insertion(new_id);
        self.session_orders.push(order.clone());
        order
    }

    pub fn insert_order(&mut self, request: OrderInsertionRequest) -> (OrderId, ObOrderInsertionResult) {
        let order = self.create_order(request);
        let id = order.id.clone();
        let orderbook = &mut self.orderbook;
        let execution_result = match request.order_type {
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
        (id, execution_result)
    }

    pub fn cancel_order(
        &mut self,
        req: OrderCancellationRequest,
    ) -> OrderCancellationResult {
        let order = self.session_orders.get_mut(req.order_id)
            .ok_or(OrderCancellationError::OrderDoesNotExist)?;
        let cancellation = req.into_cancellation(order.side, order.price);
        let orderbook = &mut self.orderbook;
        let result = orderbook.cancel_order(cancellation);
        if result.is_ok() {
            order.status = OrderExecutionStatus::Cancelled;
        }
        result
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
