use std::fmt::Display;

use crate::{asset::*, orderbook::OrderExecutionStatus, types::*};

#[derive(Debug, Clone, Copy)]
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
pub struct Order {
    pub id: OrderId,
    pub account_id: AccountId,
    pub order_type: OrderType,
    pub pair: AssetIdPair,
    pub side: Side,
    pub volume: Volume,
    pub price: Price,
    pub status: OrderExecutionStatus,
}

impl Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = self.id;
        let acc_id = self.account_id;
        let ot = &self.order_type;
        let side = &self.side;
        let volume = self.volume;
        let price = self.price;
        write!(
            f,
            "Order {id} from account {acc_id}: {side:?} {volume} at {price} ({ot:?})"
        )
    }
}
