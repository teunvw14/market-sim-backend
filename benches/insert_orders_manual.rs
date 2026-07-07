use std::{
    io::Write,
    time::{Duration, Instant},
};

use num_format::{CustomFormat, ToFormattedString};

use backend::{
    asset::*,
    exchange::*,
    exchange_client::ExchangeClient,
    order::*,
    util::{
        statics::{MAX_CMD_BUF_SIZE, ORDER_PRICES, VOLUMES},
        types::*,
    },
};

const BUF_SIZE: usize = MAX_CMD_BUF_SIZE;

async fn send_orders(
    client: ExchangeClient,
    account_id: AccountId,
    pair: AssetIdPair,
    duration: Duration,
    side: Side,
) -> usize {
    let mut count = 0;
    let mut messages = 0;
    let start = Instant::now();
    let mut elapsed = Duration::ZERO;
    while elapsed < duration {
        count += 1;

        // Random price between 0.5 and 1.5
        let price = Price::from(ORDER_PRICES[count % ORDER_PRICES.len()]);
        let volume = VOLUMES[count % VOLUMES.len()];

        let command_buffer: CommandBuffer = [Command::OrderInsert(OrderInsertionRequest {
            account_id,
            order_type: OrderType::Limit,
            pair,
            side,
            volume,
            price,
        }); BUF_SIZE]
            .into();
        // Ignore errors if desired, or handle them
        let _result = client.send_commands(command_buffer).await;

        // let _result = client.insert_order(account_id, OrderType::Limit, pair, side, volume, price).await;

        messages += BUF_SIZE;
        if count % 1_000 == 0 {
            elapsed = start.elapsed();
        }
    }

    messages
}

#[tokio::main]
async fn main() {
    // Bench params
    let bench_duration = Duration::from_secs(1);

    // Init exchange
    let (mut exchange, exchange_handle) = Exchange::new();

    // Add assets and markets to exchange
    let jpy_id = exchange.add_asset("Japanese Yen", "JPY");
    let usd_id = exchange.add_asset("United States Dollar", "USD");
    let eur_id = exchange.add_asset("Euro", "EUR");
    let chf_id = exchange.add_asset("Swiss Frank", "CHF");
    let cad_id = exchange.add_asset("Canadian Dollar", "CAD");
    let gbp_id = exchange.add_asset("British Pound", "GBP");
    let assets = vec![jpy_id, usd_id, eur_id, chf_id, cad_id, gbp_id];

    // Create all possible pairs out of all listed assets
    let mut pairs = Vec::new();
    for asset1 in &assets {
        for asset2 in &assets {
            if asset1 != asset2 {
                let pair = AssetIdPair {
                    primary: *asset1,
                    secondary: *asset2,
                };
                let pair_rev = AssetIdPair {
                    primary: *asset2,
                    secondary: *asset1,
                };
                if !(pairs.contains(&pair) || pairs.contains(&pair_rev)) {
                    pairs.push(AssetIdPair {
                        primary: *asset1,
                        secondary: *asset2,
                    });
                }
            }
        }
    }
    dbg!(&pairs);
    // Add pairs on exchange
    for pair in &pairs {
        exchange.add_market(*pair).unwrap();
    }

    // Create some accounts to send orders from
    let concurrency = 2;
    let mut handles = Vec::new();

    // Launch `concurrency` Tokio tasks sending orders.
    for i in 0..concurrency {
        let account_id = exchange.create_account();
        let client = exchange_handle.get_client();
        let pair_index = account_id as usize % pairs.len();
        let pair = pairs[pair_index];
        let side = match i % 2 {
            0 => Side::Ask,
            1 => Side::Bid,
            _ => unreachable!(),
        };
        let handle =
            tokio::task::spawn(send_orders(client, account_id, pair, bench_duration, side));
        handles.push(handle);
    }

    exchange.run();

    let start = Instant::now();
    let mut total: usize = 0;
    // Place a bunch of orders
    for handle in handles {
        total += handle.await.unwrap();
    }
    let duration = start.elapsed();
    println!("Processed {total} orders in {duration:?}."); // Print newline
    // let orderbook_size = exchange.get_market(&pair).unwrap().get_orderbook_size();
    // println!("Orderbook size: {orderbook_size}");
    // println!("{exchange:?}");
}
