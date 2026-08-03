use backend::exchange_client::ExchangeClient;
use backend::order::OrderInsertionRequest;
use backend::order::OrderType;
use backend::order::Side;
use backend::util::types::Price;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

use tokio::runtime::Runtime;

use backend::util::exchange_configs::*;

// Here we have an async function to benchmark
async fn insert_order(client: &ExchangeClient, request: OrderInsertionRequest) {
    let _result = client.insert_order(request).await;
}

fn from_elem(c: &mut Criterion) {
    let (exchange_handle, markets, accounts) = exchange_eur_usd_market_2_accs();
    let client = exchange_handle.get_client();
    let mut order_id = 0;

    let runtime = Runtime::new().unwrap();
    c.bench_function("Insert Order", |b| {
        let market = markets[0];
        let price = Price::from(10);

        // Insert a call to `to_async` to convert the bencher to async mode.
        // The timing loops are the same as with the normal bencher.
        b.to_async(&runtime).iter(|| {
            let side = if order_id % 2 == 0 {
                Side::Ask
            } else {
                Side::Bid
            };
            order_id += 1;
            insert_order(
                &client,
            OrderInsertionRequest {
                account_id: 0,
                order_type: OrderType::Limit,
                pair: market,
                side,
                volume: 10,
                price,
            })
        });
    });
}

criterion_group!(benches, from_elem);
criterion_main!(benches);