use std::collections::VecDeque;

use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::asset::*;
use crate::market::*;
use crate::market_handler::*;
use crate::order::*;
use crate::statics::MPSC_CAPACITY;
use crate::types::*;

#[derive(Debug, Clone, Copy)]
pub enum Command {
    AddAccount(),
    AddAsset(),
    BalanceChange(AccountId, AssetId, Balance),
    GetBalance(AccountId, AssetId)
}

#[derive(Debug, Clone)]
pub enum CommandResult {
    AddAccount(AccountId),
    AddAsset(AssetId),
    // TODO: make this result its own type with proper error types
    BalanceChange(Result<(), ()>),
    GetBalance(Option<Balance>)
}

/// A buffer of commands for a specific market
pub type CommandBuffer = VecDeque<Command>;
/// A buffer of results from a CommandBuffer
pub type CommandResultBuffer = VecDeque<CommandResult>;

pub struct CommandBufferWithReplyChannel {
    pub command_buf: CommandBuffer,
    pub tx_reply: oneshot::Sender<CommandResultBuffer>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BalanceBook {
    accounts: AccountId,
    assets: AssetId,
    balances: Vec<Balance>,
}

impl BalanceBook {
    pub fn new() -> Self {
        Self { accounts: 0, assets: 0, balances: Vec::new() }
    }
    fn get_index(&self, asset_id: AssetId, account_id: AccountId) -> Option<usize> {
        if account_id >= self.accounts {
            return None;
        }
        Some(self.accounts as usize * asset_id as usize + account_id as usize)
    }

    pub fn get(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
    ) -> Option<&Balance> {
        let index = self.get_index(asset_id, account_id)?;
        self.balances.get(index)
    }

    pub fn get_mut(
        &mut self,
        account_id: AccountId,
        asset_id: AssetId,
    ) -> Option<&mut Balance> {
        let index = self.get_index(asset_id, account_id)?;
        self.balances.get_mut(index)
    }

    /// Adds an asset, returns the id of the newly created asset.
    pub fn add_asset(&mut self) -> AssetId {
        for _ in 0..self.accounts {
            self.balances.push(Balance::ZERO);
        }
        self.assets += 1;
        self.assets - 1
    }

    pub fn add_account(&mut self) -> AccountId {
        for i in 0..self.assets as usize {
            let index = (i + 1) * (self.accounts as usize) + i;
            self.balances.insert(index, Balance::ZERO);
        }
        self.accounts += 1;
        self.accounts - 1
    }
}


impl Drop for BalanceBook {
    fn drop(&mut self) {
        println!("Dropping balancebook {self:?}");
    }
}


#[derive(Debug, Clone)]
pub struct BalanceManager {
    pub tx_command_buf: mpsc::Sender<CommandBufferWithReplyChannel>
}

impl BalanceManager {
    pub fn new() -> Self {
        let (tx_command_buf, rx_command_buf) = mpsc::channel(MPSC_CAPACITY);
        tokio::task::spawn(Self::run(rx_command_buf));
        BalanceManager { tx_command_buf }
    }

    pub async fn add_asset(&self) -> AssetId {
        let (tx_reply, rx_reply) = oneshot::channel();
        let add_asset_command = vec![Command::AddAsset()].into();
        let command_buf = CommandBufferWithReplyChannel {
            command_buf: add_asset_command,
            tx_reply,
        };
        // TODO: Investigate safety of these unwraps
        // unwrap fine because if that channel has closed unexpectedly, the whole application should close too
        self.tx_command_buf.send(command_buf).await.unwrap();
        let mut reply_buf = rx_reply.await.unwrap();
        let result = reply_buf.pop_front().unwrap();
        if let CommandResult::AddAsset(new_asset_id) = result {
            new_asset_id
        } else {
            panic!()
        }
    }

    pub async fn add_account(&self) -> AccountId {
        let (tx_reply, rx_reply) = oneshot::channel();
        let add_account_command = vec![Command::AddAccount()].into();
        let command_buf = CommandBufferWithReplyChannel {
            command_buf: add_account_command,
            tx_reply,
        };
        // TODO: Investigate safety of these unwraps
        // unwrap fine because if that channel has closed unexpectedly, the whole application should close too
        self.tx_command_buf.send(command_buf).await.unwrap();
        let mut reply_buf = rx_reply.await.unwrap();
        let result = reply_buf.pop_front().unwrap();
        if let CommandResult::AddAccount(new_account_id) = result {
            new_account_id
        } else {
            panic!()
        }
    }
    
    pub async fn get_balance(&self, account_id: AccountId, asset_id: AssetId) -> Option<Balance> {
        let (tx_reply, rx_reply) = oneshot::channel();
        let get_balance_command = vec![Command::GetBalance(account_id, asset_id)].into();
        let command_buf = CommandBufferWithReplyChannel {
            command_buf: get_balance_command,
            tx_reply,
        };
        // TODO: Investigate safety of these unwraps
        // unwrap fine because if that channel has closed unexpectedly, the whole application should close too
        self.tx_command_buf.send(command_buf).await.unwrap();
        let mut reply_buf = rx_reply.await.unwrap();
        let result = reply_buf.pop_front().unwrap();
        if let CommandResult::GetBalance(balance) = result {
            balance
        } else {
            panic!()
        }
    }

    async fn run(mut rx_command_buf: mpsc::Receiver<CommandBufferWithReplyChannel>) {
        let mut balance_book = BalanceBook::new();
        loop {
            // Check if handler has been assigned a new market. This is
            // Clear channel (we don't want to extend it) and receive `OrderBuffer`s
            let mut channel_buf = Vec::with_capacity(MPSC_CAPACITY);
            let n = rx_command_buf
                .recv_many(&mut channel_buf, MPSC_CAPACITY)
                .await;
            if n == 0 {
                println!("BalanceManager channel closed. Shutting down.");
                break;
            }

            // Process orders
            for msg in channel_buf {
                let mut response_buf = VecDeque::with_capacity(msg.command_buf.len());
                for command in msg.command_buf {
                    Self::handle_command(command, &mut response_buf, &mut balance_book);
                }
                let _ = msg.tx_reply.send(response_buf);
            }
        }
    } 

    fn handle_command(command: Command, response_buf: &mut CommandResultBuffer, balance_book: &mut BalanceBook) {
        let result = match command {
            Command::AddAccount() => {
                let new_account_id = balance_book.add_account();
                CommandResult::AddAccount(new_account_id)
            }
            Command::AddAsset() => {
                let new_asset_id = balance_book.add_asset();
                CommandResult::AddAsset(new_asset_id)
            }
            Command::BalanceChange(account_id, asset_id, change) => {
                let balance = balance_book.get_mut(account_id, asset_id);
                if let Some(balance) = balance {
                    *balance += change;
                    CommandResult::BalanceChange(Ok(()))
                } else {
                    CommandResult::BalanceChange(Err(()))
                }
            },
            Command::GetBalance(account_id, asset_id) => {
                let balance = balance_book.get(account_id, asset_id);
                if let Some(balance) = balance {
                    CommandResult::GetBalance(Some(balance.clone()))
                } else {
                    CommandResult::GetBalance(None)
                }
            }
        };
        response_buf.push_back(result);
    }
}