use std::{
    io::Write,
    time::{Duration, Instant},
};

use num_format::{CustomFormat, ToFormattedString};

use backend::{
    asset::*, exchange::*, exchange_client::ExchangeClient, order::*, util::{
        exchange_configs::exchange_5fx_markets_5_accs, statics::{MAX_CMD_BUF_SIZE, ORDER_PRICES, VOLUMES}, types::*,
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
    let bench_duration = Duration::from_secs(5);

    let (exchange_handle, pairs, accounts) = exchange_5fx_markets_5_accs();

    // Create some accounts to send orders from
    let concurrency = 5;
    let mut handles = Vec::new();

    // Launch `concurrency` Tokio tasks sending orders.
    for i in 0..concurrency {
        let account_id = accounts[i];
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
