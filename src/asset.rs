use crate::types::*;

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct Asset {
    pub id: AssetId,
    pub name: String,
    pub symbol: String,
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct AssetPair {
    pub primary: Asset,
    pub secondary: Asset,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub struct AssetIdPair {
    pub primary: AssetId,
    pub secondary: AssetId,
}
