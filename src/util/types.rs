use fixed::types::*;

pub type OrderId = usize;
pub type AssetId = u32;
pub type AccountId = u32;
pub type Volume = u32;
pub type Price = I33F31; // So that it's possible to cast from u32 (so we need 32 num bits + 1 sign bit = 33)
pub type Balance = I33F31; // Needs to be the same as Price
