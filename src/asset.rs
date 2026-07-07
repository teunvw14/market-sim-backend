use serde::{Deserialize, Serialize};

use crate::util::types::*;

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

#[derive(PartialEq, Eq, Debug, Clone, Copy, Default, Hash, Serialize, Deserialize)]
pub struct AssetIdPair {
    pub primary: AssetId,
    pub secondary: AssetId,
}
