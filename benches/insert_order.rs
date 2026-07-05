use std::time::Duration;

use backend::statics::ORDER_PRICES;
use criterion::async_executor::FuturesExecutor;
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

use backend::asset::*;
use backend::exchange::*;
use backend::order::*;
use backend::types::*;

async fn actually_insert_order() {}

async fn insert_order(c: &mut Criterion) {
    let (mut exchange, exchange_handle) = Exchange::new();

    let eur_id = exchange.add_asset("Euro", "EUR");
    let usd_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: eur_id,
        secondary: usd_id,
    };
    exchange.add_market(pair).unwrap();

    // Create 2 accounts
    for _ in 0..2 {
        exchange.create_account();
    }

    let mut order_id: usize = 0;
    let volume = 10;
    let client = exchange_handle.get_client();
    let tokio_runtime = tokio::runtime::Runtime::new().unwrap();

    // Single insertions to measure latency
    c.bench_with_input(
        BenchmarkId::new("single_insertion_latency", 1),
        &client,
        |b, &client| {
            b.to_async(tokio_runtime).iter(async || {
                order_id += 1;
                let side = if order_id % 2 == 0 {
                    Side::Bid
                } else {
                    Side::Ask
                };
                let account_id = 1;
                (order_id % 2) as u32;
                let price = Price::from(ORDER_PRICES[order_id % ORDER_PRICES.len()]);
                (&client)
                    .insert_order(account_id, OrderType::Limit, pair, side, volume, price)
                    .await
                    .unwrap();
            })
        },
    );
}

criterion_group! {
    name = benches;
    // This can be any expression that returns a `Criterion` object.
    config = Criterion::default()
        .sample_size(10_000)
        .warm_up_time(Duration::from_millis(1));
    targets = insert_order
}

criterion_main!(benches);
