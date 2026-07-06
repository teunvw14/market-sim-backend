use backend::{asset::AssetIdPair, exchange::*, exchange_configs::*, order::*, types::*};

#[test]
fn command_get_balance() {
    let get_balance = Command::GetBalance(0, 1);
    let serialized = rmp_serde::to_vec(&get_balance).unwrap();
    let deserialized: Command = rmp_serde::decode::from_slice(&serialized).unwrap();
    assert!(deserialized == get_balance);
}

#[test]
fn command_order_insert() {
    let order_insert = Command::OrderInsert(OrderInsertionRequest {
        account_id: 0,
        order_type: OrderType::Limit,
        pair: AssetIdPair::default(),
        side: Side::Ask,
        volume: 0,
        price: Price::ONE,
    });
    let serialized = rmp_serde::to_vec(&order_insert).unwrap();
    let deserialized: Command = rmp_serde::decode::from_slice(&serialized).unwrap();
    assert!(deserialized == order_insert);
}

#[test]
fn command_buffer_order_insert() {
    let order_insert = Command::OrderInsert(OrderInsertionRequest {
        account_id: 0,
        order_type: OrderType::Limit,
        pair: AssetIdPair::default(),
        side: Side::Ask,
        volume: 0,
        price: Price::ONE,
    });
    let command_buf: CommandBuffer = vec![order_insert; 128].into();
    let serialized = rmp_serde::to_vec(&command_buf).unwrap();
    let deserialized: CommandBuffer = rmp_serde::decode::from_slice(&serialized).unwrap();
    assert_eq!(deserialized, command_buf);
}
