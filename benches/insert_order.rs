use std::time::Duration;

use backend::statics::ORDER_PRICES;
use criterion::async_executor::FuturesExecutor;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use backend::asset::*;
use backend::exchange::*;
use backend::order::*;
use backend::types::*;

async fn insert_order(c: &mut Criterion) {
    let mut exchange = Exchange::new();

    let eur_id = exchange.add_asset("Euro", "EUR");
    let usd_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: eur_id,
        secondary: usd_id,
    };
    exchange.create_market(pair).await.unwrap();

    // Create 2 accounts
    for _ in 0..2 {
        exchange.create_account();
    }

    let mut order_id: usize = 0;
    let volume = 10;

    // Single insertions to measure latency
    c.bench_function("single_insertion_latency", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            order_id += 1;
            let side = if order_id % 2 == 0 {
                Side::Bid
            } else {
                Side::Ask
            };
            let account_id = (order_id % 2) as u32;
            let price = Price::from(ORDER_PRICES[order_id % ORDER_PRICES.len()]);
            exchange
                .insert_order(account_id, OrderType::Limit, pair, side, volume, price)
                .unwrap();
        })
    });
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
