use std::cmp::Eq;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;
use fixed::types::*;
use num_format::{Buffer, CustomFormat, Error, Grouping, ToFormattedString};
use tokio::net::{TcpListener, TcpStream};

use rand::RngExt;

use mini_redis::Command::{self, Get, Set};
use mini_redis::{Connection, Frame};

use backend::{asset::*, exchange::*, order::*, statics::{ORDER_PRICES, VOLUMES}, types::*};

#[derive(Clone)]
struct SharedMap<K, V> {
    inner: Arc<DashMap<K, V>>,
}

impl<K: Eq + Hash, V: Clone> SharedMap<K, V> {
    fn new() -> Self {
        SharedMap {
            inner: Arc::new(DashMap::with_shard_amount(32)),
        }
    }

    fn insert(&self, key: K, value: V) {
        self.inner.insert(key, value);
    }

    fn get(&self, key: &K) -> Option<V> {
        if let Some(value) = self.inner.get(key) {
            return Some(value.clone());
        } else {
            return None;
        }
    }
}

async fn process(socket: TcpStream, db: SharedMap<String, Bytes>) {
    // Connection, provided by `mini-redis`, handles parsing frames from
    // the socket
    let mut connection = Connection::new(socket);

    // Use `read_frame` to receive a command from the connection.
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                // The value is stored as `Vec<u8>`
                // let k = cmd.key().to_string();
                // let v = cmd.value();
                // println!("Inserting ({k}, {v:?})");
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                if let Some(value) = db.get(&cmd.key().into()) {
                    // `Frame::Bulk` expects data to be of type `Bytes`. This
                    // type will be covered later in the tutorial. For now,
                    // `&Vec<u8>` is converted to `Bytes` using `into()`.
                    Frame::Bulk(value.clone().into())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented {:?}", cmd),
        };

        // Write the response to the client
        connection.write_frame(&response).await.unwrap();
    }
}

fn test_main() {
    let mut exchange = Exchange::new();
    let acc_id_1 = exchange.add_account();
    let acc_id_2 = exchange.add_account();

    let EUR_id = exchange.add_asset("Euro", "EUR");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();

    let price: Price = Price::lit("0.85");
    exchange
    .insert_order(pair, acc_id_1, OrderType::Limit, Side::Ask, 5, price)
    .unwrap();
    let price: Price = Price::lit("0.86");
    exchange
        .insert_order(pair, acc_id_1, OrderType::Limit, Side::Ask, 5, price)
        .unwrap();
    println!("{exchange:#?}");
    let price: Price = Price::lit("0.9");
    exchange
        .insert_order(pair, acc_id_2, OrderType::Limit, Side::Bid, 20, price)
        .unwrap();

    println!("");
    println!("{exchange:#?}");
}

fn bench_main() {
    let mut exchange = Exchange::new();
    let N = 10;
    for _ in 0..N {
        exchange.add_account();
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

    let mut random_generator = rand::rng();

    let mut order_id: usize = 0;
    let mut count: u64 = 0;
    let mut successful_count: u64 = 0;

    let start = Instant::now();
    let test_duration = Duration::from_secs(15);

    let mut elapsed = Duration::ZERO;

    while elapsed < test_duration {
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
        let result = exchange.insert_order(pair, account_id, OrderType::Limit, side, volume, price);

        count += 1;
        if result.is_ok() {
            successful_count += 1;
        }
        // let last_price_opt = exchange.get_last_price(pair);
        // if let Some(last_price) = last_price_opt {
        //     print!("\rLast traded price {last_price:?}                         ")
        // }
        if count % 1_000_000 == 0 {
            elapsed = start.elapsed();
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let ops_per_sec = count as f64 / elapsed;

    let format = CustomFormat::builder().separator(" ").build().unwrap();
    let count = count.to_formatted_string(&format);
    let successful_count = successful_count.to_formatted_string(&format);
    let ops_per_sec = (ops_per_sec.floor() as u64).to_formatted_string(&format);

    println!("Processed {count} orders ({successful_count} successful) in {elapsed:.2} seconds");
    println!("Throughput: {ops_per_sec} orders/sec");
    // println!("{exchange:?}");
}

// #[tokio::main]
// async fn main() {
//     let addr = "127.0.0.1:6379";
//     let listener = TcpListener::bind(addr).await.unwrap();

//     println!("Server listening at {addr}");

//     let db: SharedMap<String, Bytes> = SharedMap::new();

//     loop {
//         // The second item contains the IP and port of the new connection.
//         let (socket, _) = listener.accept().await.unwrap();
//         let db = db.clone();
//         // println!("New connection accepted.");
//         tokio::spawn(async move {
//             process(socket, db).await;
//         });
//     }
// }

fn main() {
    // test_main();
    bench_main();
}