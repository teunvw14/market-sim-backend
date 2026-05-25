use std::{
    io::Write,
    time::{Duration, Instant},
};

use num_format::{CustomFormat, ToFormattedString};

use backend::{
    asset::*,
    exchange::*,
    order::*,
    statics::{ORDER_PRICES, VOLUMES},
    types::*,
};

fn main() {
    // Bench params
    let test_duration = Duration::from_secs(5);

    // Init exchange
    let mut exchange = Exchange::new();

    // Create some accounts to send orders from
    let N = 10;
    for _ in 0..N {
        exchange.create_account();
    }

    // Add assets and markets to exchange
    let JPY_id = exchange.add_asset("Japanese Yen", "JPY");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let EUR_id = exchange.add_asset("Euro", "EUR");
    let pair = AssetIdPair {
        primary: JPY_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();

    // Show exchange state
    println!("{exchange:?}");

    // Place a bunch of orders
    let mut order_id: usize = 0;

    let start = Instant::now();
    let format = CustomFormat::builder().separator(" ").build().unwrap();

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
        let _result =
            exchange.insert_order(account_id, OrderType::Limit, pair, side, volume, price);

        // let last_price_opt = exchange.get_last_price(pair);
        // if let Some(last_price) = last_price_opt {
        //     print!("\rLast traded price {last_price:?}                         ")
        // }
        if order_id % 5_000_000 == 0 {
            let elapsed = start.elapsed().as_secs_f64();
            let ops_per_sec = order_id as f64 / elapsed;

            let count = order_id.to_formatted_string(&format);
            let ops_per_sec = (ops_per_sec.floor() as u64).to_formatted_string(&format);

            print!("\rProcessed {count} orders in {elapsed:.2} seconds | {ops_per_sec} orders/s");
            std::io::stdout().flush().unwrap();
            if start.elapsed() > test_duration {
                break;
            }
        }
    }

    println!(); // Print newline
    let orderbook_size = exchange.get_market(&pair).unwrap().get_orderbook_size();
    println!("Orderbook size: {orderbook_size}");
    // println!("{exchange:?}");
}
