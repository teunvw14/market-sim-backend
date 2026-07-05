use std::collections::HashMap;
use std::collections::VecDeque;

use ringbuf::{LocalRb, storage::Heap, traits::*};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tracing::trace;

use crate::asset::*;
use crate::balance_book::BalanceBook;
use crate::market::*;
use crate::order::*;
use crate::orderbook::*;
use crate::statics::MPSC_CAPACITY;
use crate::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Command {
    OrderInsert(OrderInsertionRequest),
    OrderCancel(OrderCancellationRequest),
    OrderModify(OrderModificationRequest),
    AddMarket(AssetIdPair),
    GetBalance(AccountId, AssetId),
}

#[derive(Debug, Clone, Serialize)]
pub enum CommandResult {
    OrderInsert(OrderInsertionResult),
    OrderCancel(OrderCancellationResult),
    OrderModify(OrderModificationResult),
    AddMarket(MarketCreationResult),
    GetBalance(Option<Balance>),
}

/// A buffer of commands for a specific market
pub type CommandBuffer = VecDeque<Command>;

/// A buffer of results from a CommandBuffer
pub type CommandResultBuffer = VecDeque<CommandResult>;

pub struct CommandBufferWithReplyChannel {
    pub command_buf: CommandBuffer,
    pub tx_reply: oneshot::Sender<CommandResultBuffer>,
}

/// Wrapper around Markets for sending orders to an exchange
pub struct ExchangeClient {
    tx_command_buf: mpsc::Sender<CommandBufferWithReplyChannel>,
}

impl ExchangeClient {
    /// Helper function to send a single order insertion
    pub async fn insert_order(
        &self,
        order_insertion_req: OrderInsertionRequest,
    ) -> OrderInsertionResult {
        let command = Command::OrderInsert(order_insertion_req);
        let buf: CommandBuffer = vec![command].into();
        let mut result_buf = self.send_commands(buf).await;
        if let Some(result) = result_buf.pop_front() {
            match result {
                CommandResult::OrderInsert(res) => res,
                _ => Err(OrderInsertionError::Other),
            }
        } else {
            Err(OrderInsertionError::Other)
        }
    }

    pub async fn get_balance(&self, account_id: AccountId, asset_id: AssetId) -> Option<Balance> {
        let command = Command::GetBalance(account_id, asset_id);
        let buf: CommandBuffer = vec![command].into();
        let mut result_buf = self.send_commands(buf).await;
        if let Some(result) = result_buf.pop_front()
            && let CommandResult::GetBalance(balance) = result
        {
            return balance;
        }
        None
    }

    pub async fn send_commands(&self, command_buf: CommandBuffer) -> CommandResultBuffer {
        let (tx_reply, rx_reply) = oneshot::channel();
        let command_buf = CommandBufferWithReplyChannel {
            command_buf: command_buf,
            tx_reply,
        };
        self.tx_command_buf.send(command_buf).await.unwrap();
        rx_reply.await.unwrap() // TODO: fix all these unwraps
    }
}

pub struct Transaction {
    pub price: Price,
    pub volume: Volume,
    pub taker_side: Side,
    pub maker: AccountId,
    pub taker: AccountId,
}

pub struct Exchange {
    accounts: HashMap<AccountId, Account>,
    traded_assets: HashMap<AssetId, Asset>,
    balance_book: BalanceBook,
    markets: Markets,
    rx_command_buf: mpsc::Receiver<CommandBufferWithReplyChannel>,
    session_orders: HashMap<OrderId, PlacedOrder>,
    transactions_last_100: LocalRb<Heap<Transaction>>,
    transaction_buf: ObTransactionBuffer,
}

#[derive(Debug, Clone)]
pub struct ExchangeHandle {
    pub tx_command_buf: mpsc::Sender<CommandBufferWithReplyChannel>,
}

impl ExchangeHandle {
    pub fn get_client(&self) -> ExchangeClient {
        ExchangeClient {
            tx_command_buf: self.tx_command_buf.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Account {
    id: AccountId,
    username: String,
}

impl Exchange {
    pub fn new() -> (Self, ExchangeHandle) {
        let (tx_command_buf, rx_command_buf) = mpsc::channel(MPSC_CAPACITY);
        (
            Exchange {
                accounts: HashMap::new(),
                traded_assets: HashMap::new(),
                balance_book: BalanceBook::new(),
                markets: Markets::new(),
                session_orders: HashMap::new(),
                rx_command_buf,
                transactions_last_100: LocalRb::new(100),
                transaction_buf: Vec::new(),
            },
            ExchangeHandle { tx_command_buf },
        )
    }

    pub fn add_asset(&mut self, name: &str, symbol: &str) -> AssetId {
        let new_id = self.traded_assets.len() as u32;
        let name = name.to_string();
        let symbol = symbol.to_string();
        let new_asset = Asset {
            id: new_id,
            name,
            symbol,
        };
        self.traded_assets.insert(new_id, new_asset.clone());

        // Update balance book to include new asset
        self.balance_book.add_asset();

        new_id
    }

    pub fn remove_asset(&mut self, asset_id: AssetId) {
        self.traded_assets.remove(&asset_id);
    }

    pub fn create_account(&mut self) -> AccountId {
        let new_id = self.accounts.len() as AccountId;
        self.accounts.insert(
            new_id,
            Account {
                id: new_id,
                username: String::default(),
            },
        );

        // Add the account to the balance book
        self.balance_book.add_account();

        // Return created account id
        new_id
    }

    pub fn create_account_with_name<S: Into<String>>(&mut self, username: S) -> AccountId {
        let new_id = self.accounts.len() as AccountId;
        self.accounts.insert(
            new_id,
            Account {
                id: new_id,
                username: username.into(),
            },
        );
        new_id
    }

    pub fn add_market(&mut self, asset_pair: AssetIdPair) -> MarketCreationResult {
        // Market can only be created for listed assets
        if !self.traded_assets.contains_key(&asset_pair.primary) {
            return Err(MarketCreationError::AssetNotTraded {
                asset: asset_pair.primary,
            });
        }
        if !self.traded_assets.contains_key(&asset_pair.secondary) {
            return Err(MarketCreationError::AssetNotTraded {
                asset: asset_pair.secondary,
            });
        };
        self.markets.add_market(asset_pair)
    }

    pub fn run(self) {
        std::thread::spawn(|| self.run_inner());
    }

    fn run_inner(mut self) {
        let mut channel_buf = Vec::with_capacity(MPSC_CAPACITY);
        loop {
            // Receive `CommandBuffer`s
            let n = self
                .rx_command_buf
                .blocking_recv_many(&mut channel_buf, MPSC_CAPACITY);
            if n == 0 {
                println!("Order channel closed. Shutting down Exchange");
                break;
            }

            // Process command buffers
            // Use `drain` so that the order of `CommandBuffer`s is maintained
            for msg in channel_buf.drain(..) {
                let mut response_buf = VecDeque::with_capacity(msg.command_buf.len());
                for command in msg.command_buf {
                    self.handle_command(command, &mut response_buf);
                }
                // Send results. Ignore send failures
                let _ = msg.tx_reply.send(response_buf);
            }
        }
    }

    fn handle_command(&mut self, command: Command, response_buf: &mut CommandResultBuffer) {
        let result = match command {
            Command::OrderInsert(insertion_req) => {
                let result = self.insert_order(insertion_req);
                CommandResult::OrderInsert(result)
            }
            Command::OrderCancel(cancellation) => {
                todo!()
            }
            Command::OrderModify(modification) => {
                todo!()
            }
            Command::AddMarket(pair) => {
                println!("Received new market {pair:?}");
                CommandResult::AddMarket(self.add_market(pair))
            }
            Command::GetBalance(account_id, asset_id) => {
                CommandResult::GetBalance(self.balance_book.get(account_id, asset_id))
            }
        };
        response_buf.push_back(result);
    }

    fn insert_order(&mut self, insertion_req: OrderInsertionRequest) -> OrderInsertionResult {
        // Get market (if it exists)
        let market = self
            .markets
            .get_mut(&insertion_req.pair)
            .ok_or(OrderInsertionError::MarketDoesNotExist)?;

        // Insert order
        let new_id = self.session_orders.len();
        let insertion = insertion_req.into_insertion(new_id);
        let ob_result = market.insert_order(insertion.clone(), &mut self.transaction_buf)?;

        // Insert OpenOrder
        let open_order = PlacedOrder::from_insertion(&insertion);
        self.session_orders.insert(new_id, open_order);

        // Process transactions resulting from order insertion
        self.process_transactions(insertion_req.pair);

        Ok(OrderInsertionEffects {
            id: new_id,
            status: ob_result.status,
        })
    }

    /// Process orders in the
    fn process_transactions(&mut self, pair: AssetIdPair) {
        while let Some(transaction) = self.transaction_buf.pop() {
            trace!("New transaction: {transaction:?}");
            let volume_primary = Balance::from(transaction.volume);
            let volume_secondary = volume_primary * transaction.price;

            let [maker_order, taker_order] = self
                .session_orders
                .get_disjoint_mut([&transaction.order_id_maker, &transaction.order_id_taker])
                .map(|opt| opt.expect("Order should exist because it is created upon insertion."));

            // Update order remaining volume
            maker_order.remaining_volume -= transaction.volume;
            taker_order.remaining_volume -= transaction.volume;

            // Update balances
            let maker_id = maker_order.account_id;
            let taker_id = taker_order.account_id;

            let primary = pair.primary;
            let secondary = pair.secondary;

            // Get balances
            let [
                balance_primary_maker,
                balance_primary_taker,
                balance_secondary_maker,
                balance_secondary_taker,
            ] = self
                .balance_book
                .get_disjoint_mut([
                    (primary, maker_id),
                    (primary, taker_id),
                    (secondary, maker_id),
                    (secondary, taker_id),
                ])
                .expect("Index overlap is impossible because self-trade returns error in `insert_order`.");

            // Update balances
            match transaction.taker_side {
                Side::Ask => {
                    // Taker asks (secondary for primary)
                    *balance_primary_taker -= volume_primary;
                    *balance_secondary_taker += volume_secondary;

                    *balance_primary_maker += volume_primary;
                    *balance_secondary_maker -= volume_secondary;
                }
                Side::Bid => {
                    // Taker bids (secondary for primary)
                    *balance_primary_taker += volume_primary;
                    *balance_secondary_taker -= volume_secondary;

                    *balance_primary_maker -= volume_primary;
                    *balance_secondary_maker += volume_secondary;
                }
            }
        }
    }
}
