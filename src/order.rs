use std::{collections::VecDeque, fmt::Display};

use tokio::sync::oneshot;

use crate::{
    asset::*,
    orderbook::{OrderExecutionStatus, OrderInsertionResult},
    types::*,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    FillOrKill,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy)]
pub struct OrderInsertion {
    /// The id of the account that created the order.
    pub account_id: AccountId,
    /// The type of the order (limit, market, etc.). Field is `order_type` because `type` is a reserved keyword.
    pub order_type: OrderType,
    /// The pair that the order should be executed on.
    pub pair: AssetIdPair,
    /// Side of the order (Bid / Ask).
    pub side: Side,
    /// Volume of the order in whole units.
    pub volume: Volume,
    /// Price of the order.
    pub price: Price,
}

impl Display for OrderInsertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let acc_id = self.account_id;
        let ot = &self.order_type;
        let side = &self.side;
        let volume = self.volume;
        let price = self.price;
        write!(
            f,
            "Order from account {acc_id}: {side:?} {volume} at {price} ({ot:?})"
        )
    }
}

pub type OrderBuffer = VecDeque<OrderInsertion>;
pub type OrderInsertionResultBuffer = VecDeque<OrderInsertionResult>;

pub struct OrderBufferWithReplyChannel {
    pub order_buf: OrderBuffer,
    pub tx_reply: oneshot::Sender<OrderInsertionResultBuffer>,
}

/// OderCancellation encapsulates the information needed to efficiently remove an order
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct OrderCancellation {
    pub order_id: OrderId,
    pub side: Side,
    pub price: Price,
}

// impl From<OrderInsertion> for OrderCancellation {
//     fn from(order: OrderInsertion) -> Self {
//         Self {
//             order_id: order.id,
//             side: order.side,
//             price: order.price,
//         }
//     }
// }
