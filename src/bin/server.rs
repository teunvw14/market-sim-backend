use std::time::{Duration, Instant};

use num_format::{CustomFormat, ToFormattedString};

use mini_redis::Command::{self, Get, Set};
use mini_redis::{Connection, Frame};

use tonic::{transport::Server, Request, Response, Status};

use ExchangeGRPC::exchange_service_server::{ExchangeService, ExchangeServiceServer};
use ExchangeGRPC::{CreateAccountResponse};

pub mod ExchangeGRPC {
    tonic::include_proto!("exchange_grpc");
}

use backend::{
    asset::*,
    exchange::*,
    order::*,
    statics::{ORDER_PRICES, VOLUMES},
    types::*,
};

// #[derive(Clone)]
// struct SharedMap<K, V> {
//     inner: Arc<DashMap<K, V>>,
// }

// impl<K: Eq + Hash, V: Clone> SharedMap<K, V> {
//     fn new() -> Self {
//         SharedMap {
//             inner: Arc::new(DashMap::with_shard_amount(32)),
//         }
//     }

//     fn insert(&self, key: K, value: V) {
//         self.inner.insert(key, value);
//     }

//     fn get(&self, key: &K) -> Option<V> {
//         if let Some(value) = self.inner.get(key) {
//             return Some(value.clone());
//         } else {
//             return None;
//         }
//     }
// }

// async fn process(socket: TcpStream, db: SharedMap<String, Bytes>) {
//     // Connection, provided by `mini-redis`, handles parsing frames from
//     // the socket
//     let mut connection = Connection::new(socket);

//     // Use `read_frame` to receive a command from the connection.
//     while let Some(frame) = connection.read_frame().await.unwrap() {
//         let response = match Command::from_frame(frame).unwrap() {
//             Set(cmd) => {
//                 // The value is stored as `Vec<u8>`
//                 // let k = cmd.key().to_string();
//                 // let v = cmd.value();
//                 // println!("Inserting ({k}, {v:?})");
//                 db.insert(cmd.key().to_string(), cmd.value().clone());
//                 Frame::Simple("OK".to_string())
//             }
//             Get(cmd) => {
//                 if let Some(value) = db.get(&cmd.key().into()) {
//                     // `Frame::Bulk` expects data to be of type `Bytes`. This
//                     // type will be covered later in the tutorial. For now,
//                     // `&Vec<u8>` is converted to `Bytes` using `into()`.
//                     Frame::Bulk(value.clone().into())
//                 } else {
//                     Frame::Null
//                 }
//             }
//             cmd => panic!("unimplemented {:?}", cmd),
//         };

//         // Write the response to the client
//         connection.write_frame(&response).await.unwrap();
//     }
// }

fn test_main() {
    let mut exchange = Exchange::new();
    let acc_id_1 = exchange.create_account();
    let acc_id_2 = exchange.create_account();

    let EUR_id = exchange.add_asset("Euro", "EUR");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();

    let price: Price = Price::lit("0.85");
    exchange
        .insert_order(acc_id_1, OrderType::Limit, pair, Side::Ask, 5, price)
        .unwrap();
    let price: Price = Price::lit("0.86");
    exchange
        .insert_order(acc_id_1, OrderType::Limit, pair, Side::Ask, 5, price)
        .unwrap();
    println!("{exchange:#?}");
    let price: Price = Price::lit("0.9");
    exchange
        .insert_order(acc_id_2, OrderType::Limit, pair, Side::Bid, 20, price)
        .unwrap();

    println!("");
    println!("{exchange:#?}");
}

fn bench_main() {
    let mut exchange = Exchange::new();
    let N = 10;
    for _ in 0..N {
        exchange.create_account();
    }

    let JPY_id = exchange.add_asset("Japanese Yen", "JPY");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: JPY_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();
    let EUR_id = exchange.add_asset("Euro", "EUR");
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();
    println!("{exchange:?}");

    let mut order_id: usize = 0;

    let start = Instant::now();
    let test_duration = Duration::from_secs(10);

    loop {
        order_id += 1;

        // Random price between 0.5 and 1.5
        let price = Price::from(ORDER_PRICES[order_id % ORDER_PRICES.len()]);

        // Alternate accounts to avoid self-trades
        let account_id = (order_id % N) as AccountId;
        let side = if order_id % 2 == 0 {
            Side::Bid
        } else {
            Side::Ask
        };

        let volume = VOLUMES[order_id % VOLUMES.len()];
        // Ignore errors if desired, or handle them
        let _result = exchange.insert_order(account_id, OrderType::Limit, pair, side, volume, price);

        // let last_price_opt = exchange.get_last_price(pair);
        // if let Some(last_price) = last_price_opt {
        //     print!("\rLast traded price {last_price:?}                         ")
        // }
        if order_id % 5_000_000 == 0 {
            if start.elapsed() > test_duration {
                break;
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let ops_per_sec = order_id as f64 / elapsed;

    let format = CustomFormat::builder().separator(" ").build().unwrap();
    let count = order_id.to_formatted_string(&format);
    let ops_per_sec = (ops_per_sec.floor() as u64).to_formatted_string(&format);

    println!("Processed {count} orders in {elapsed:.2} seconds");
    println!("Throughput: {ops_per_sec} orders/sec");

    let orderbook_size = exchange.get_market(&pair).unwrap().get_orderbook_size();
    println!("Orderbook size: {orderbook_size}");
    // println!("{exchange:?}");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:6379".parse()?;
    let mut exchange = Exchange::new();

    let JPY_id = exchange.add_asset("Japanese Yen", "JPY");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: JPY_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();
    let EUR_id = exchange.add_asset("Euro", "EUR");
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();
    println!("{exchange:?}");

    Ok(())
}

// fn main() {
//     // test_main();
//     bench_main();
// }
