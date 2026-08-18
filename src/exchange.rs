use std::collections::{HashMap, VecDeque};
use std::mem::MaybeUninit;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info};

use crate::errors::*;
use crate::exchange_client::ExchangeClient;
use crate::{
    asset::*, balance_book::BalanceBook, market::*, order::*, orderbook::*,
    util::statics::MPSC_CAPACITY, util::types::*,
};

/// Macro for defining the allowed commands to an Exchange. Creates Command
/// and CommandResult enums, as well as From<R> -> CommandResult impl's for
/// each result type R.
///
/// This macro saves having to repeat the CommandName (e.g. 'OrderInsert') for
/// both the Command and CommandResult enum. Also saves having to manually
/// implement From<R> -> CommandResult for each new Command's return type R.
macro_rules! define_exchange_commands {
    ( $($CommandName:ident($($CommandArgs:ty),*), $ResultType:ty;)+) => {
        // Define Command
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        pub enum Command {
            $($CommandName($($CommandArgs,)*),)+
        }

        // Define CommandResult
        #[derive(Debug, Clone, PartialEq, Serialize)]
        pub enum CommandResult {
            $($CommandName($ResultType)),+
        }

        // Impl conversions for the different CommandResult variants so that we
        // can call `.into()` on the Result type to get a CommandResult
        $(
        impl From<$ResultType> for CommandResult {
            fn from(value: $ResultType) -> Self {
                CommandResult::$CommandName(value)
            }
        }
        )+
    };
}

// Each line "CommandName(Args), ResultType;" defines a unique command and the
// type returned internally (i.e. before it is wrapped in a CommandResult type)
// by that command.
//
// Note: due to how the macro is defined, duplicate result types will cause
// duplicate From impl's resulting in an error.
define_exchange_commands! {
    OrderInsert(OrderInsertionRequest), OrderInsertionResult;
    OrderCancel(OrderCancellationRequest), OrderCancellationResult;
    OrderModify(OrderModificationRequest), OrderModificationResult;
    AddMarket(AssetIdPair), MarketCreationResult;
    GetBalance(AccountId, AssetId), Option<Balance>;
    GetOrderbookL1(AssetIdPair), GetOrderbookL1Result;
    GetOrderbookL2(AssetIdPair), GetOrderbookL2Result;
    GetAssets(), Vec<Asset>;
    GetAllOrderbookL1(), Vec<(AssetIdPair, OrderbookL1)>;
    GetLast100Transactions(), Vec<Transaction>;
}

/// A buffer of commands for a specific market
pub type CommandBuffer = VecDeque<Command>;

/// A buffer of results from a CommandBuffer
pub type CommandResultBuffer = Vec<CommandResult>;

pub struct CommandBufferWithReplyChannel {
    pub command_buf: CommandBuffer,
    pub tx_reply: oneshot::Sender<CommandResultBuffer>,
}

/// A ringbuffer of recent transactions.
pub struct RecentTransactions {
    inner: Vec<Transaction>,
    write_idx: usize,
}

impl RecentTransactions {
    pub fn with_capacity(capacity: usize) -> RecentTransactions {
        RecentTransactions {
            inner: Vec::with_capacity(capacity),
            write_idx: 0,
        }
    }

    pub fn get(&self) -> Vec<Transaction> {
        let mut result = self.inner.clone();
        result.rotate_left(self.write_idx % self.inner.capacity());
        result
    }

    pub fn push(&mut self, transaction: Transaction) {
        let cap = self.inner.capacity();
        if self.inner.len() < cap {
            self.inner.push(transaction);
        } else {
            let entry = self.inner.get_mut(self.write_idx % cap).unwrap();
            *entry = transaction;
        }
        self.write_idx += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Transaction {
    pub market: AssetIdPair,
    pub price: Price,
    pub volume: Volume,
    pub taker_side: Side,
    pub maker: AccountId,
    pub taker: AccountId,
    pub timestamp_ms: u64,
}

pub struct Exchange {
    accounts: HashMap<AccountId, Account>,
    traded_assets: Vec<Asset>,
    balance_book: BalanceBook,
    markets: Markets,
    rx_command_buf: mpsc::Receiver<CommandBufferWithReplyChannel>,
    session_orders: Vec<(OrderId, PlacedOrder)>,
    last_100_tx: RecentTransactions,
    transaction_buf: ObTransactionBuffer,
}

/// The handle is a thin wrapper around the Sender side of the command channel
/// for the exchange. Under the hood, it's the same as the client, but it serves
/// as a sort of "primary client". Concretely,  clients should be derived from
/// it (using `get_client`), and it allows for controlling exchange shutdown,
/// since dropping this handle is a requirement for shutdown.
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
    /// Create a new exchange and handle to that exchange
    pub fn new() -> (Self, ExchangeHandle) {
        let (tx_command_buf, rx_command_buf) = mpsc::channel(MPSC_CAPACITY);
        (
            Exchange {
                accounts: HashMap::new(),
                traded_assets: Vec::new(),
                balance_book: BalanceBook::new(),
                markets: Markets::new(),
                session_orders: Vec::with_capacity(10_000_000),
                rx_command_buf,
                last_100_tx: RecentTransactions::with_capacity(100),
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
        self.traded_assets.push(new_asset);

        // Update balance book to include new asset
        self.balance_book.add_asset();

        new_id
    }

    /// Check if an asset is traded on the exchange.
    pub fn is_traded_asset(&self, asset_id: AssetId) -> bool {
        self.traded_assets
            .iter()
            .map(|asset| asset.id)
            .any(|id| id == asset_id)
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

    pub fn add_market(&mut self, pair: AssetIdPair) -> MarketCreationResult {
        // Market can only be created for listed assets
        if !self.is_traded_asset(pair.primary) || !self.is_traded_asset(pair.secondary) {
            return Err(MarketCreationError::AssetNotTraded);
        }
        self.markets.add_market(pair)
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
                info!("Exchange command channel closed. Shutting down Exchange.");
                break;
            }

            // Process command buffers
            // Use `drain` so that the order of `CommandBuffer`s is maintained
            for msg in channel_buf.drain(..) {
                let len = msg.command_buf.len();
                let mut response_buf = Vec::with_capacity(len);
                let spare = response_buf.spare_capacity_mut();
                for (i, command) in msg.command_buf.into_iter().enumerate() {
                    let result = self.handle_command(command);
                    spare[i] = MaybeUninit::new(result);
                }
                // SAFETY: we write `len` elements into `response_buf`
                unsafe {
                    response_buf.set_len(len);
                }

                // Send results. Ignore send failures
                let _ = msg.tx_reply.send(response_buf);
            }
        }
    }

    fn handle_command(&mut self, command: Command) -> CommandResult {
        let result: CommandResult = match command {
            Command::OrderInsert(insertion_req) => self.insert_order(insertion_req).into(),
            Command::OrderCancel(cancellation) => self.cancel_order(cancellation).into(),
            Command::OrderModify(modification) => self.modify_order(modification).into(),
            Command::AddMarket(pair) => {
                info!("New market command received: {pair:?}");
                self.add_market(pair).into()
            }
            Command::GetBalance(account_id, asset_id) => {
                self.balance_book.get(account_id, asset_id).into()
            }
            Command::GetOrderbookL1(pair) => self.get_orderbook_l1(pair).into(),
            Command::GetOrderbookL2(pair) => self.get_orderbook_l2(pair).into(),
            Command::GetAssets() => self.traded_assets.clone().into(),
            Command::GetAllOrderbookL1() => self.get_all_orderbook_l1().into(),
            Command::GetLast100Transactions() => self.get_last_100_transactions().into(),
        };
        result
    }

    fn insert_order(&mut self, insertion_req: OrderInsertionRequest) -> OrderInsertionResult {
        debug!("Insertion request: {insertion_req:?}");

        // Get market (if it exists)
        let market = self
            .markets
            .get_mut(&insertion_req.pair)
            .ok_or(OrderInsertionError::MarketDoesNotExist)?;

        if insertion_req.account_id as usize >= self.accounts.len() {
            return Err(OrderInsertionError::AccountDoesNotExist);
        }

        // Insert order
        let new_id = self.session_orders.len();
        let insertion = insertion_req.into_insertion(new_id);
        let ob_result = market.insert_order(insertion.clone(), &mut self.transaction_buf)?;

        // Insert OpenOrder
        let open_order = PlacedOrder::from_insertion(&insertion);
        self.session_orders.push((new_id, open_order));

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
            debug!("New transaction: {transaction:?}");
            let volume_primary = Balance::from(transaction.volume);
            let volume_secondary = volume_primary * transaction.price;

            let [(_maker_order_id, maker_order), (_taker_order_id, taker_order)] = self
                .session_orders
                .get_disjoint_mut([transaction.order_id_maker, transaction.order_id_taker])
                .expect("get_disjoint_mut should never error, since order id's should be valid, and self-trades cannot happen.");

            // Update order remaining volume
            maker_order.remaining_volume -= transaction.volume;
            taker_order.remaining_volume -= transaction.volume;
            if maker_order.remaining_volume == 0 {
                maker_order.status = OrderExecutionStatus::Filled;
            }
            if taker_order.remaining_volume == 0 {
                taker_order.status = OrderExecutionStatus::Filled;
            }

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

            // Add transaction to `last_100_tx`.
            // Conversion safety: log_10(timestamp) ≈ 12, log_10(u64::MAX) ≈ 19.
            let timestamp_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            self.last_100_tx.push(Transaction {
                market: pair,
                price: transaction.price,
                volume: transaction.volume,
                taker_side: transaction.taker_side,
                maker: maker_id,
                taker: taker_id,
                timestamp_ms,
            });
        }
    }

    fn cancel_order(
        &mut self,
        cancellation_req: OrderCancellationRequest,
    ) -> OrderCancellationResult {
        debug!("Cancellation request: order {}", cancellation_req.order_id);

        // Look up order
        let (_id, order) = self
            .session_orders
            .iter_mut()
            .find(|(id, _order)| id == &cancellation_req.order_id)
            .ok_or(OrderCancellationError::OrderDoesNotExist)?;

        if order.account_id != cancellation_req.account_id {
            return Err(OrderCancellationError::Unauthorized);
        }

        if order.status == OrderExecutionStatus::Cancelled {
            return Err(OrderCancellationError::AlreadyCancelled);
        } else if order.status == OrderExecutionStatus::Filled {
            return Err(OrderCancellationError::AlreadyFilled);
        }

        // Get market (if it exists - it should)
        let market = self
            .markets
            .get_mut(&order.pair)
            .ok_or(OrderCancellationError::MarketDoesNotExist)?;

        let cancellation = cancellation_req.into_cancellation(order.pair, order.side, order.price);
        let result = market.cancel_order(cancellation);

        if result.is_ok() {
            order.status = OrderExecutionStatus::Cancelled;
        }

        result
    }

    /// Modify an order (only volume decreased are allowed). If the new volume is lower than the
    /// amount that was already filled, the order is marked as Filled.
    fn modify_order(
        &mut self,
        modification_req: OrderModificationRequest,
    ) -> OrderModificationResult {
        debug!(
            "Modification request: order {} new_volume={}",
            modification_req.order_id, modification_req.new_volume
        );

        // Look up order
        let (_id, order) = self
            .session_orders
            .iter_mut()
            .find(|(id, _order)| id == &modification_req.order_id)
            .ok_or(OrderModificationError::OrderDoesNotExist)?;

        if order.account_id != modification_req.account_id {
            return Err(OrderModificationError::Unauthorized);
        }

        if order.status == OrderExecutionStatus::Filled {
            return Err(OrderModificationError::AlreadyFilled);
        }

        if modification_req.new_volume > order.volume {
            return Err(OrderModificationError::VolumeNotLower);
        }

        // Get market (if it exists - it should)
        let market = self
            .markets
            .get_mut(&order.pair)
            .ok_or(OrderModificationError::MarketDoesNotExist)?;

        // Apply modification
        let modification = modification_req.into_order_modification(
            order.pair,
            order.side,
            order.price,
            order.volume,
        );
        let result = market.modify_order(modification);

        if result.is_ok() {
            let volume_filled = order.volume - order.remaining_volume;
            if volume_filled > modification_req.new_volume {
                order.volume = volume_filled;
                order.status = OrderExecutionStatus::Filled;
            } else {
                order.volume = modification_req.new_volume;
            };
        }
        result
    }

    fn get_orderbook_l1(&self, pair: AssetIdPair) -> GetOrderbookL1Result {
        let market = self
            .markets
            .get(&pair)
            .ok_or(GetOrderbookError::MarketDoesNotExist)?;
        Ok(market.get_l1())
    }

    fn get_orderbook_l2(&self, pair: AssetIdPair) -> GetOrderbookL2Result {
        let market = self
            .markets
            .get(&pair)
            .ok_or(GetOrderbookError::MarketDoesNotExist)?;
        Ok(market.get_l2())
    }

    fn get_all_orderbook_l1(&self) -> Vec<(AssetIdPair, OrderbookL1)> {
        let mut result = Vec::with_capacity(self.markets.keys.len());
        for market in &self.markets.inner {
            result.push((market.asset_pair, market.get_l1()));
        }
        result
    }

    fn get_last_100_transactions(&self) -> Vec<Transaction> {
        self.last_100_tx.get()
    }
}
