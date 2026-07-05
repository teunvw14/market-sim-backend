use backend::{exchange_configs::*, order::*, types::*};

#[tokio::test]
async fn buy_sell() {
    let (exchange_handle, pair, acc_id_1, acc_id_2) = exchange_eur_usd_market_2_accs().await;

    let price: Price = Price::lit("0.85");
    let client = exchange_handle.get_client();
    client
        .insert_order(OrderInsertionRequest {
            account_id: acc_id_1,
            order_type: OrderType::Limit,
            pair,
            side: Side::Ask,
            volume: 5,
            price,
        })
        .await
        .unwrap();
    // Insert another ask at higher level
    let price: Price = Price::lit("0.86");
    client
        .insert_order(OrderInsertionRequest {
            account_id: acc_id_1,
            order_type: OrderType::Limit,
            pair,
            side: Side::Ask,
            volume: 5,
            price,
        })
        .await
        .unwrap();
    let price: Price = Price::lit("0.9");
    client
        .insert_order(OrderInsertionRequest {
            account_id: acc_id_2,
            order_type: OrderType::Limit,
            pair,
            side: Side::Bid,
            volume: 20,
            price,
        })
        // .insert_order(acc_id_2, OrderType::Limit, pair, Side::Bid, 20, price)
        .await
        .unwrap();

    // Confirm that trades swapped the two assets equally
    let bal_primary_1 = client.get_balance(acc_id_1, pair.primary).await.unwrap();
    let bal_primary_2 = client.get_balance(acc_id_2, pair.primary).await.unwrap();
    assert!(bal_primary_1 == -1 * bal_primary_2);

    let bal_secondary_1 = client.get_balance(acc_id_1, pair.secondary).await.unwrap();
    let bal_secondary_2 = client.get_balance(acc_id_2, pair.secondary).await.unwrap();
    assert!(bal_secondary_1 == -1 * bal_secondary_2);
}
