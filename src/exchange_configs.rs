/// Contains a few setups for Exchange objects
use crate::{asset::*, exchange::*, order::*, types::*};

/// Create a simple exchange with a single EUR/USD market and two accounts.
pub async fn exchange_eur_usd_market() -> (ExchangeHandle, AssetIdPair) {
    let (mut exchange, exchange_handle) = Exchange::new();
    // Create two accounts

    let eur_id = exchange.add_asset("Euro", "EUR");
    let usd_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: eur_id,
        secondary: usd_id,
    };
    exchange.add_market(pair).unwrap();
    exchange.run();

    (exchange_handle, pair)
}

/// Create a simple exchange with a single EUR/USD market and two accounts.
pub async fn exchange_eur_usd_market_2_accs() -> (ExchangeHandle, AssetIdPair, AccountId, AccountId)
{
    let (mut exchange, exchange_handle) = Exchange::new();
    // Create two accounts
    let id_1 = exchange.create_account();
    let id_2 = exchange.create_account();

    let eur_id = exchange.add_asset("Euro", "EUR");
    let usd_id = exchange.add_asset("United States Dollar", "USD");
    let pair = AssetIdPair {
        primary: eur_id,
        secondary: usd_id,
    };
    exchange.add_market(pair).unwrap();
    exchange.run();

    (exchange_handle, pair, id_1, id_2)
}
