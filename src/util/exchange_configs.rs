/// Contains a few setups for Exchange objects. All setups have the same return type so that setups
/// can be easily interchanged.
use crate::{asset::*, exchange::*, util::types::*};

/// Create a simple exchange with a single EUR/USD market, no accounts.
pub fn exchange_eur_usd_market() -> (ExchangeHandle, Vec<AssetIdPair>, Vec<AccountId>) {
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

    (exchange_handle, vec![pair], vec![])
}

/// Create a simple exchange with a single EUR/USD market and two accounts.
pub fn exchange_eur_usd_market_2_accs() -> (ExchangeHandle, Vec<AssetIdPair>, Vec<AccountId>) {
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

    (exchange_handle, vec![pair], vec![id_1, id_2])
}

/// Create a small exchange with five FX markets and five accounts.
/// Markets
/// USD/EUR
/// USD/JPY
/// USD/CHF
/// EUR/CHF
/// EUR/JPY
pub fn exchange_5fx_markets_5_accs() -> (ExchangeHandle, Vec<AssetIdPair>, Vec<AccountId>) {
    let (mut exchange, exchange_handle) = Exchange::new();
    // Create two accounts
    let mut accounts = Vec::with_capacity(5);
    for _ in 0..5 {
        let new_acc_id = exchange.create_account();
        accounts.push(new_acc_id);
    }

    let usd_id = exchange.add_asset("United States Dollar", "USD");
    let eur_id = exchange.add_asset("Euro", "EUR");
    let jpy_id = exchange.add_asset("Japanese Yen", "JPY");
    let chf_id = exchange.add_asset("Swiss Frank", "CHF");

    // Create all possible pairs out of all listed assets
    let pairs: Vec<AssetIdPair> = vec![
        AssetIdPair {
            primary: usd_id,
            secondary: eur_id,
        },
        AssetIdPair {
            primary: usd_id,
            secondary: jpy_id,
        },
        AssetIdPair {
            primary: usd_id,
            secondary: chf_id,
        },
        AssetIdPair {
            primary: eur_id,
            secondary: chf_id,
        },
        AssetIdPair {
            primary: eur_id,
            secondary: jpy_id,
        },
    ];

    // Add pairs on exchange
    for pair in &pairs {
        exchange.add_market(*pair).unwrap();
    }

    exchange.run();

    (exchange_handle, pairs, accounts)
}
