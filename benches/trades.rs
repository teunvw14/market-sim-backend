use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use backend::asset::*;
use backend::exchange::*;
use backend::order::*;
use backend::types::*;

use rand::RngExt;

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn insertions_benchmark(c: &mut Criterion) {
    let mut exchange = Exchange::new();
    let N = 10;
    for _ in 0..N {
        exchange.add_account();
    }

    let EUR_id = exchange.add_asset("Euro", "EUR");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();
    println!("{exchange:?}");

    let mut random_generator = rand::rng();

    let mut order_id: usize = 0;
    let volume = 10;

    c.bench_function("trade_insert", |b| {
        b.iter(|| {
            order_id += 1;

            let side = if order_id % 2 == 0 {
                Side::Bid
            } else {
                Side::Ask
            };
            let account_id = if order_id % 2 == 0 { 1 } else { 2 };
            let price_val: f64 = random_generator.random_range(0.5..1.5);
            let price = Price::lit(&format!("{:.7}", price_val));
            exchange.insert_order(pair, account_id, OrderType::Limit, side, volume, price);
        })
    });
}

criterion_group!(benches, insertions_benchmark);
criterion_main!(benches);
