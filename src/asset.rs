use serde::{Deserialize, Serialize};

use crate::util::types::*;

/// A new asset, which has not yet been assigned an id.
#[derive(PartialEq, Eq, Debug, Clone, Default, Hash, Serialize, Deserialize)]
pub struct NewAsset {
    pub name: String,
    pub symbol: String,
}

#[derive(PartialEq, Eq, Debug, Clone, Default, Hash, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub name: String,
    pub symbol: String,
}

#[derive(PartialEq, Eq, Debug, Clone, Default, Hash)]
pub struct AssetPair {
    pub primary: Asset,
    pub secondary: Asset,
}

/// Symbolic representation of an asset pair, e.g. EUR/USD.
#[derive(PartialEq, Eq, Debug, Clone, Default, Hash, Serialize, Deserialize)]
pub struct AssetPairSymbolic {
    pub primary: String,
    pub secondary: String,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Default, Hash, Serialize, Deserialize)]
pub struct AssetIdPair {
    pub primary: AssetId,
    pub secondary: AssetId,
}
