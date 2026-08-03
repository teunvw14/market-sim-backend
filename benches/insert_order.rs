use std::collections::VecDeque;

use backend::exchange_client::ExchangeClient;
use backend::order::OrderInsertionRequest;
use backend::order::OrderType;
use backend::order::Side;
use backend::util::types::Price;
use backend::exchange::{Command};
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

use tokio::runtime::Runtime;

use backend::util::exchange_configs::*;


async fn insert_order(client: &ExchangeClient, request: OrderInsertionRequest) {
    let _result = client.insert_order(request).await;
}

async fn insert_order_bulk(client: &ExchangeClient, request: OrderInsertionRequest, n: usize) {
    let command_buf = vec![Command::OrderInsert(request); n].into();
    let _result = client.send_commands(command_buf).await;
}

/// Benchmarks the latency of inserting 16 orders at once that should 
/// either end up in an empty orderbook, or be immediately matched.
fn bench_insert_order_bulk_16(c: &mut Criterion) {
    // Initialize exchange and variables
    let (exchange_handle, markets, accounts) = exchange_eur_usd_market_2_accs();
    let client = exchange_handle.get_client();
    let mut order_id = 0;
    let acc_0 = accounts[0];
    let acc_1 = accounts[1];
    let market = markets[0];
    let price = Price::from(1);
    let buf_size: usize = 16;

    // Initialize Tokio runtime (required for async bench). We use the Tokio
    // runtime for benchmarks since that's what's used in the code.
    let runtime = Runtime::new().unwrap();

    // Set up benchmark
    let mut group = c.benchmark_group("Insert Order Group");
    group.sample_size(1000);
    group.throughput(criterion::Throughput::Elements(buf_size as u64));

    group.bench_function("Insert Order Bulk (16)", |b| {
        b.to_async(&runtime).iter(|| {
            // Pick `side` and `account_id`
            let side = if order_id % 2 == 0 {
                Side::Ask
            } else {
                Side::Bid
            };
            let account_id = if order_id % 2 == 0 {
                acc_0
            } else {
                acc_1
            };
            order_id += 1;
            insert_order_bulk(
                &client,
                OrderInsertionRequest {
                    account_id,
                    order_type: OrderType::Limit,
                    pair: market,
                    side,
                    volume: 10,
                    price,
                },
                buf_size,
            )
        });
    });

    group.finish();
}

/// Benchmarks the latency of inserting 256 orders at once that should 
/// either end up in an empty orderbook, or be immediately matched.
fn bench_insert_order_bulk_1024(c: &mut Criterion) {
    // Initialize exchange and variables
    let (exchange_handle, markets, accounts) = exchange_eur_usd_market_2_accs();
    let client = exchange_handle.get_client();
    let mut order_id = 0;
    let acc_0 = accounts[0];
    let acc_1 = accounts[1];
    let market = markets[0];
    let price = Price::from(1);
    let buf_size: usize = 1024;

    // Initialize Tokio runtime (required for async bench). We use the Tokio
    // runtime for benchmarks since that's what's used in the code.
    let runtime = Runtime::new().unwrap();

    // Set up benchmark
    let mut group = c.benchmark_group("Insert Order Group");
    group.sample_size(1000);
    group.throughput(criterion::Throughput::Elements(buf_size as u64));

    group.bench_function("Insert Order Bulk (1024)", |b| {
        b.to_async(&runtime).iter(|| {
            // Pick `side` and `account_id`
            let side = if order_id % 2 == 0 {
                Side::Ask
            } else {
                Side::Bid
            };
            let account_id = if order_id % 2 == 0 {
                acc_0
            } else {
                acc_1
            };
            order_id += 1;
            insert_order_bulk(
                &client,
                OrderInsertionRequest {
                    account_id,
                    order_type: OrderType::Limit,
                    pair: market,
                    side,
                    volume: 10,
                    price,
                },
                buf_size,
            )
        });
    });

    group.finish();
}

/// Benchmarks the latency of inserting a single order that should either end
/// up in an empty orderbook, or be immediately matched.
fn bench_insert_order_single(c: &mut Criterion) {
    // Initialize exchange and variables
    let (exchange_handle, markets, accounts) = exchange_eur_usd_market_2_accs();
    let client = exchange_handle.get_client();
    let mut order_id = 0;
    let acc_0 = accounts[0];
    let acc_1 = accounts[1];
    let market = markets[0];
    let price = Price::from(1);

    // Initialize Tokio runtime (required for async bench). We use the Tokio
    // runtime for benchmarks since that's what's used in the code.
    let runtime = Runtime::new().unwrap();

    // Set up benchmark
    let mut group = c.benchmark_group("Insert Order Group");
    group.sample_size(1000);

    group.bench_function("Insert Order", |b| {
        b.to_async(&runtime).iter(|| {
            // Pick `side` and `account_id`
            let side = if order_id % 2 == 0 {
                Side::Ask
            } else {
                Side::Bid
            };
            let account_id = if order_id % 2 == 0 {
                acc_0
            } else {
                acc_1
            };
            order_id += 1;
            insert_order(
                &client,
                OrderInsertionRequest {
                    account_id,
                    order_type: OrderType::Limit,
                    pair: market,
                    side,
                    volume: 10,
                    price,
                },
            )
        });
    });

    group.finish();
}

criterion_group!(benches, 
    bench_insert_order_single,
    bench_insert_order_bulk_16,
    bench_insert_order_bulk_1024,
);
criterion_main!(benches);
