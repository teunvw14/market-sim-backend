use std::slice::GetDisjointMutError;

use crate::util::types::*;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BalanceBook {
    accounts: AccountId,
    assets: AssetId,
    balances: Vec<Balance>,
}

impl BalanceBook {
    pub fn new() -> Self {
        Self {
            accounts: 0,
            assets: 0,
            balances: Vec::new(),
        }
    }
    fn get_index(&self, asset_id: AssetId, account_id: AccountId) -> usize {
        self.accounts as usize * asset_id as usize + account_id as usize
    }

    pub fn get(&self, account_id: AccountId, asset_id: AssetId) -> Option<Balance> {
        let index = self.get_index(asset_id, account_id);
        self.balances.get(index).copied()
    }

    pub fn get_mut(&mut self, account_id: AccountId, asset_id: AssetId) -> Option<&mut Balance> {
        let index = self.get_index(asset_id, account_id);
        self.balances.get_mut(index)
    }

    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        indexing_pairs: [(AssetId, AccountId); N],
    ) -> Result<[&mut Balance; N], GetDisjointMutError> {
        let indices =
            indexing_pairs.map(|(asset_id, account_id)| self.get_index(asset_id, account_id));
        self.balances.get_disjoint_mut(indices)
    }

    /// Adds an asset, returns the id of the newly created asset.
    pub fn add_asset(&mut self) {
        for _ in 0..self.accounts {
            self.balances.push(Balance::ZERO);
        }
        self.assets += 1;
    }

    pub fn add_account(&mut self) {
        for i in 0..self.assets as usize {
            let index = (i + 1) * (self.accounts as usize) + i;
            self.balances.insert(index, Balance::ZERO);
        }
        self.accounts += 1;
    }
}
