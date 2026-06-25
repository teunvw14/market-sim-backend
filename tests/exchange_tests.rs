use backend::{asset::*, exchange::*, order::*, types::*};

/// Create a simple exchange with a single EUR/USD market and two accounts.
#[cfg(test)]
async fn exchange_eur_usd_market_2_accs() -> (Exchange, AssetIdPair, AccountId, AccountId) {
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
    exchange.create_market(pair).await.unwrap();

    (exchange, pair, id_1, id_2)
}

#[tokio::test]
async fn buy_sell() {
    let (mut exchange, pair, acc_id_1, acc_id_2) = exchange_eur_usd_market_2_accs().await;

    let price: Price = Price::lit("0.85");
    let client = exchange.get_client();
    client
        .insert_order(acc_id_1, OrderType::Limit, pair, Side::Ask, 5, price)
        .await.unwrap();
    let price: Price = Price::lit("0.86");
    client
        .insert_order(acc_id_1, OrderType::Limit, pair, Side::Ask, 5, price)
        .await.unwrap();
    println!("{exchange:#?}");
    let price: Price = Price::lit("0.9");
    client
        .insert_order(acc_id_2, OrderType::Limit, pair, Side::Bid, 20, price)
        .await.unwrap();

    println!("");
    println!("{exchange:#?}");
}
