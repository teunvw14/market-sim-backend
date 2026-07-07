use backend::{
    exchange::{Command, CommandResult},
    exchange_configs::*,
    order::*,
    types::*,
};

/// Place two asks at different price levels, one bid that should consume both.
/// Check that balances reflect expected amounts.
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
    assert_eq!(bal_primary_1, -1 * bal_primary_2);

    let bal_secondary_1 = client.get_balance(acc_id_1, pair.secondary).await.unwrap();
    let bal_secondary_2 = client.get_balance(acc_id_2, pair.secondary).await.unwrap();
    assert_eq!(bal_secondary_1, -1 * bal_secondary_2);

    // Check amounts
    assert_eq!(bal_primary_1, -10);
    assert_eq!(
        bal_secondary_2,
        -Price::lit("0.85") * 5 + -Price::lit("0.86") * 5
    );
}

/// Place and cancel single order, check that a cancelled order cannot be traded with.
#[tokio::test]
async fn cancel_order() {
    let (exchange_handle, pair, acc_id_1, acc_id_2) = exchange_eur_usd_market_2_accs().await;
    let client = exchange_handle.get_client();

    // Insert command and then immediately cancel
    let price: Price = Price::lit("0.85");
    let insertion_effects = client
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

    let order_id = insertion_effects.id;

    // Cancel order and check that the cancel gave an Ok result
    let cancellation_req = Command::OrderCancel(OrderCancellationRequest {
        account_id: acc_id_1,
        order_id,
    });
    let res = client
        .send_commands([cancellation_req].into())
        .await
        .pop_back()
        .unwrap();
    assert_eq!(res, CommandResult::OrderCancel(Ok(())));

    // Insert Bid and check that no balance changes occurred
    let _insertion_effects = client
        .insert_order(OrderInsertionRequest {
            account_id: acc_id_2,
            order_type: OrderType::Limit,
            pair,
            side: Side::Bid,
            volume: 100,
            price,
        })
        .await
        .unwrap();

    // Confirm that trades only 3 of the primary asset
    let bal_primary_1 = client.get_balance(acc_id_1, pair.primary).await.unwrap();
    let bal_primary_2 = client.get_balance(acc_id_2, pair.primary).await.unwrap();
    assert_eq!(bal_primary_1, 0);
    assert_eq!(bal_primary_2, 0);
}

#[tokio::test]
async fn modify_order() {
    let (exchange_handle, pair, acc_id_1, acc_id_2) = exchange_eur_usd_market_2_accs().await;

    let client = exchange_handle.get_client();
    let price: Price = Price::lit("0.85");
    let insertion_effects = client
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

    let order_id = insertion_effects.id;

    let order_modification_req = Command::OrderModify(OrderModificationRequest {
        account_id: acc_id_1,
        order_id,
        new_volume: 3,
    });
    let command_result = client
        .send_commands([order_modification_req].into())
        .await
        .pop_back()
        .unwrap();
    if let CommandResult::OrderModify(res) = command_result {
        assert_eq!(res, Ok(()));
    }

    // Insert Bid and check that only 3 is traded
    let _insertion_effects = client
        .insert_order(OrderInsertionRequest {
            account_id: acc_id_2,
            order_type: OrderType::Limit,
            pair,
            side: Side::Bid,
            volume: 100,
            price,
        })
        .await
        .unwrap();

    // Confirm that trades only 3 of the primary asset
    let bal_primary_1 = client.get_balance(acc_id_1, pair.primary).await.unwrap();
    let bal_primary_2 = client.get_balance(acc_id_2, pair.primary).await.unwrap();
    assert_eq!(bal_primary_1, -3);
    assert_eq!(bal_primary_2, 3);
}
