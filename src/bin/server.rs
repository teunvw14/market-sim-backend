use std::time::{Duration, Instant};

use num_format::{CustomFormat, ToFormattedString};

use backend::{
    asset::*,
    exchange::*,
    order::*,
    statics::{ORDER_PRICES, VOLUMES},
    types::*,
};

fn test_main() {
    let mut exchange = Exchange::new();
    let acc_id_1 = exchange.create_account();
    let acc_id_2 = exchange.create_account();

    let EUR_id = exchange.add_asset("Euro", "EUR");
    let USD_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: EUR_id,
        secondary: USD_id,
    };
    exchange.create_market(pair).unwrap();

    let price: Price = Price::lit("0.85");
    exchange
        .insert_order(acc_id_1, OrderType::Limit, pair, Side::Ask, 5, price)
        .unwrap();
    let price: Price = Price::lit("0.86");
    exchange
        .insert_order(acc_id_1, OrderType::Limit, pair, Side::Ask, 5, price)
        .unwrap();
    println!("{exchange:#?}");
    let price: Price = Price::lit("0.9");
    exchange
        .insert_order(acc_id_2, OrderType::Limit, pair, Side::Bid, 20, price)
        .unwrap();

    println!("");
    println!("{exchange:#?}");
}

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let mut exchange = Exchange::new();

//     let JPY_id = exchange.add_asset("Japanese Yen", "JPY");
//     let USD_id = exchange.add_asset("United States Dollar", "USD");
//     let pair = AssetIdPair {
//         primary: JPY_id,
//         secondary: USD_id,
//     };
//     exchange.create_market(pair).unwrap();
//     let EUR_id = exchange.add_asset("Euro", "EUR");
//     let pair = AssetIdPair {
//         primary: EUR_id,
//         secondary: USD_id,
//     };
//     exchange.create_market(pair).unwrap();
//     println!("{exchange:?}");

//     Ok(())
// }

fn main() {
    test_main();
}
