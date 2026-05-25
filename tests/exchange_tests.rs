use backend::{
    asset::*,
    exchange::*,
    order::*,
    types::*,
};

/// Create a simple exchange with a single EUR/USD market and two accounts.
#[cfg(test)]
fn exchange_eur_usd_market_2_accs() -> (Exchange, AssetIdPair, AccountId, AccountId) {
    let mut exchange = Exchange::new();
    // Create two accounts
    let id_1 = exchange.create_account();
    let id_2 = exchange.create_account();

    let eur_id = exchange.add_asset("Euro", "EUR");
    let usd_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: eur_id,
        secondary: usd_id,
    };
    exchange.create_market(pair).unwrap();

    (exchange, pair, id_1, id_2)
}


#[test]
fn buy_sell() {
    let (mut exchange, pair, acc_id_1, acc_id_2) = exchange_eur_usd_market_2_accs();

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