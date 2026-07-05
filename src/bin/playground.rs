use backend::{
    asset::*, exchange::*, exchange_configs, mp_command_encoding::{MpCommandDecoder, MpCommandEncoder}, order::*, statics::*, types::*,
};
use bytes::{Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

fn main() {
    let command = Command::OrderInsert(OrderInsertionRequest {
        account_id: 0,
        order_type: OrderType::Limit,
        pair: AssetIdPair {
            primary: 0,
            secondary: 1,
        },
        side: Side::Ask,
        volume: 5,
        price: Price::ONE,
    });
    let command_buf = vec![command; 2];

    let mut decoder = MpCommandDecoder {};
    let mut encoder = MpCommandEncoder {};

    let mut bytes = BytesMut::new();
    encoder.encode(&command_buf, &mut bytes).unwrap();
    println!("Encoded bytes: {bytes:?}");
    let bytes_vec = bytes.to_vec();
    println!("Encoded bytes (vec): {bytes_vec:?}");

    let bytes_decoded = decoder.decode(&mut bytes).unwrap().unwrap();
    println!("Decoded bytes: {bytes_decoded:?}")
}
