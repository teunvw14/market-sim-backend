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

async fn send_orders(exchange: &Exchange, account_id: AccountId, pair: AssetIdPair, duration: Duration) {
    let mut count = 0;
    let start = Instant::now();
    let mut elapsed = Duration::ZERO;
    while elapsed < duration {
        count += 1;

        // Random price between 0.5 and 1.5
        let price = Price::from(ORDER_PRICES[count % ORDER_PRICES.len()]);

        // Alternate accounts to avoid self-trades
        let side = if count % 2 == 0 {
            Side::Bid
        } else {
            Side::Ask
        };

        let volume = VOLUMES[count % VOLUMES.len()];
        // Ignore errors if desired, or handle them
        let _result =
            exchange.insert_order(account_id, OrderType::Limit, pair, side, volume, price).await;

        if count % 100_000 == 0 {
            elapsed = start.elapsed();
        }
    }
    dbg!(count);
}

#[tokio::main]
async fn main() {
    // Bench params
    let bench_duration = Duration::from_secs(5);

    // Init exchange
    let mut exchange = Exchange::new();

    // Add assets and markets to exchange
    let JPY_id = exchange.add_asset("Japanese Yen", "JPY");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let EUR_id = exchange.add_asset("Euro", "EUR");
    let pair = AssetIdPair {
        primary: JPY_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).await.unwrap();
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).await.unwrap();

    // Create some accounts to send orders from
    let concurrency = 10;
    // let mut handles = Vec::new();
    let mut account_ids = Vec::new();
    for _ in 0..concurrency {
        let account_id = exchange.create_account();
        account_ids.push(account_id);
    }

    // for account_id in account_ids {
    //     let handle = tokio::task::spawn(send_orders(&exchange, account_id, pair, bench_duration));
    //     handles.push(handle);
    // }

    // Place a bunch of orders
    let mut order_id: usize = 0;

    let start = Instant::now();
    let format = CustomFormat::builder().separator(" ").build().unwrap();

    send_orders(&exchange, 1, pair, bench_duration).await;

    // for handle in handles {
    //     handle.await.unwrap();
    // }
    println!(); // Print newline
    // let orderbook_size = exchange.get_market(&pair).unwrap().get_orderbook_size();
    // println!("Orderbook size: {orderbook_size}");
    // println!("{exchange:?}");
}
