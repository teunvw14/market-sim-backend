use backend::{
    asset::*, exchange::*, exchange_configs, mp_command_decoder::{MAX_CMD_BUF_SIZE, MpCommandDecoder}, order::*, statics::*, types::*,
};
use bytes::{Bytes, BytesMut};
use tokio_util::codec::Decoder;

fn main() {
    let command = Command::OrderInsert(OrderInsertionRequest {
        account_id: 1,
        order_type: OrderType::Limit,
        pair: AssetIdPair {
            primary: 1,
            secondary: 2,
        },
        side: Side::Ask,
        volume: 5,
        price: Price::ONE,
    });
    let command_buf = vec![command; 2];
    let serialized = rmp_serde::to_vec(&command_buf).unwrap();
    let encoded: Vec<u8> = vec![0x00, 0x47];

    let decoder = MpCommandDecoder {};
    let encoded_bytes = Bytes::from(encoded);
    let decoded = decoder.decode(&mut encoded_bytes);
}
