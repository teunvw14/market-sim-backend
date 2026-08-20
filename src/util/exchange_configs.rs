/// Contains a few setups for Exchange objects. All setups have the same return type so that setups
/// can be easily interchanged.
use crate::{asset::*, exchange::*, util::types::*};

/// Create a simple exchange with a single EUR/USD market, no accounts.
pub fn market_eur_usd() -> (ExchangeHandle, Vec<AssetIdPair>, Vec<AccountId>) {
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
pub fn market_eur_usd_accs_2() -> (ExchangeHandle, Vec<AssetIdPair>, Vec<AccountId>) {
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
pub fn markets_5_accs_5() -> (ExchangeHandle, Vec<AssetIdPair>, Vec<AccountId>) {
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
            primary: eur_id,
            secondary: usd_id,
        },
        AssetIdPair {
            primary: jpy_id,
            secondary: usd_id,
        },
        AssetIdPair {
            primary: chf_id,
            secondary: usd_id,
        },
        AssetIdPair {
            primary: chf_id,
            secondary: eur_id,
        },
        AssetIdPair {
            primary: jpy_id,
            secondary: eur_id,
        },
    ];

    // Add pairs on exchange
    for pair in &pairs {
        exchange.add_market(*pair).unwrap();
    }

    exchange.run();

    (exchange_handle, pairs, accounts)
}


/// Creates a custom exchange setup, with each of the new assets listed, and 
/// each of the given markets created. There are `num_accounts` created, which
/// get assigned id's `0` through `num_accounts -1`. Asset symbols are 
/// capitalized to make formatting standardized.
pub fn custom(
    create_assets: Vec<NewAsset>,
    create_markets: Vec<AssetPairSymbolic>,
    num_accounts: usize,
) -> (ExchangeHandle, Vec<AssetIdPair>, Vec<AccountId>) {
    let (mut exchange, exchange_handle) = Exchange::new();
    // Create two accounts
    let mut accounts = Vec::with_capacity(num_accounts);
    for _ in 0..num_accounts {
        let new_acc_id = exchange.create_account();
        accounts.push(new_acc_id);
    }

    // Store assets with id for creating markets later
    let mut assets = Vec::new();
    for asset in create_assets {
        let new_asset_id: u32 = exchange.add_asset(&asset.name, &asset.symbol.to_uppercase());
        assets.push(Asset {
            id: new_asset_id,
            name: asset.name,
            symbol: asset.symbol.to_uppercase(),
        });
    }

    let mut markets = Vec::with_capacity(create_markets.len());
    for market in create_markets {
        // Markets have to be added by AssetIdPair, so we have to translate the
        // symbolic representation to a pair of ID's.
        let primary_id = assets
            .iter()
            .find(|a| a.symbol == market.primary.to_uppercase())
            .expect(&format!(
                "Expected valid asset symbol, got {}",
                market.primary
            ))
            .id;
        let secondary_id = assets
            .iter()
            .find(|a| a.symbol == market.secondary.to_uppercase())
            .expect(&format!(
                "Expected valid asset symbol, got {}",
                market.secondary
            ))
            .id;
        let new_market = AssetIdPair {
            primary: primary_id,
            secondary: secondary_id,
        };
        exchange.add_market(new_market).unwrap();
        markets.push(new_market);
    }

    exchange.run();
    (exchange_handle, markets, accounts)
}
