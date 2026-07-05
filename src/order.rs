use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{asset::*, market::*, types::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    FillOrKill,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Bid,
    Ask,
}

/// A request to insert a particular order, without an identifying ID or status.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrderInsertionRequest {
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

impl OrderInsertionRequest {
    /// Converts an OrderInsertionRequest into an OrderInsertion by assigning it an id.
    pub fn into_insertion(self, id: OrderId) -> OrderInsertion {
        OrderInsertion {
            id,
            account_id: self.account_id,
            order_type: self.order_type,
            pair: self.pair,
            side: self.side,
            volume: self.volume,
            price: self.price,
        }
    }
}

/// OrderInsertion represents a successfully processed OrderInsertionRequest.
#[derive(Debug, Clone)]
pub struct OrderInsertion {
    /// Identifying order id
    pub id: usize,
    /// The id of the account that created the order.
    pub account_id: AccountId,
    /// The type of the order (limit, market, etc.). Field named `order_type` because `type` is a reserved keyword.
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
        let id = self.id;
        let acc_id = self.account_id;
        let ot = &self.order_type;
        let pair = &self.pair;
        let side = &self.side;
        let volume = self.volume;
        let price = self.price;
        write!(
            f,
            "Order {id} in Market {pair:?} from account {acc_id}: {side:?} {volume} at {price} ({ot:?}) "
        )
    }
}

/// OderCancellationRequest is how a request to cancel an order is received. Needs to be transformed into an OrderCancellation to be efficiently removed from an Orderbook.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderCancellationRequest {
    pub order_id: OrderId,
}

/// OderCancellation encapsulates the information needed to remove an order efficiently.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct OrderCancellation {
    pub pair: AssetIdPair,
    pub order_id: OrderId,
    pub side: Side,
    pub price: Price,
}

/// OrderModificationRequest is how a request to modify an order is received. Needs to be transformed into an OrderModification to be efficiently removed from an Orderbook.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderModificationRequest {
    pub pair: AssetIdPair,
    pub order_id: OrderId,
    pub new_volume: Volume,
}

/// OrderModificationRequest is how a request to modify an order is received. Needs to be transformed into an OrderModification to be efficiently removed from an Orderbook.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct OrderModification {
    pub pair: AssetIdPair,
    pub order_id: OrderId,
    pub new_volume: Volume,
    pub side: Side,
    pub price: Price,
}

/// PlacedOrder struct contains all the data used to efficiently keep track of open orders.
#[derive(Debug, Clone, Copy)]
pub struct PlacedOrder {
    /// Identifying order id
    pub id: usize,
    /// The id of the account that created the order.
    pub account_id: AccountId,
    /// The pair that the order should be executed on.
    pub pair: AssetIdPair,
    /// Side of the order (Bid / Ask).
    pub side: Side,
    /// Price of the order.
    pub price: Price,
    /// Volume of the order.
    pub volume: Volume,
    /// Volume left to fill on this order.
    pub remaining_volume: Volume,
    /// Status of the order (partial fill, filled, cancelled, etc.)
    pub status: OrderExecutionStatus,
}

/// Implement conversion to clone by reference
impl PlacedOrder {
    pub fn from_insertion(insertion: &OrderInsertion) -> Self {
        PlacedOrder {
            id: insertion.id,
            account_id: insertion.account_id,
            pair: insertion.pair,
            side: insertion.side,
            price: insertion.price,
            volume: insertion.volume,
            remaining_volume: insertion.volume,
            status: OrderExecutionStatus::default(),
        }
    }
}
