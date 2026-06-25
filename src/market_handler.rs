use std::collections::VecDeque;

use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::exchange::*;
use crate::asset::*;
use crate::market::*;
use crate::order::*;
use crate::orderbook::OrderChangeBuffer;
use crate::statics::*;
use crate::balance_manager as blcm;
use crate::types::*;


#[derive(Debug, Clone, Copy)]
pub enum Command {
    OrderInsert(OrderInsertionRequest),
    OrderCancel(OrderCancellationRequest),
    OrderModify(OrderModificationRequest),
    AddMarket(AssetIdPair)
}

#[derive(Debug, Clone)]
pub enum CommandResult {
    OrderInsert(OrderInsertionResult),
    OrderCancel(OrderCancellationResult),
    OrderModify(OrderModificationResult),
    AddMarket(MarketCreationResult)
}

/// A buffer of commands for a specific market
pub struct CommandBuffer {
    pub pair: AssetIdPair,
    pub buf: VecDeque<Command>,
}
/// A buffer of results from a CommandBuffer
pub type CommandResultBuffer = VecDeque<CommandResult>;

pub struct CommandBufferWithReplyChannel {
    pub command_buf: CommandBuffer,
    pub tx_reply: oneshot::Sender<CommandResultBuffer>,
}

/// MarketHandler holds the `Sender`s needed to communicate with a thread managing
/// certain markets
#[derive(Debug, Clone)]
pub struct MarketHandler {
    tx_command_buf: mpsc::Sender<CommandBufferWithReplyChannel>
}

impl MarketHandler {
    pub fn new(tx_balance_manager: mpsc::Sender<blcm::CommandBufferWithReplyChannel>) -> Self {
        let (tx_command_buf, rx_order_buf) =
            mpsc::channel::<CommandBufferWithReplyChannel>(MPSC_CAPACITY);
        // Spawn thread that accepts new orders or markets
        tokio::task::spawn(Self::run(rx_order_buf, tx_balance_manager));
        MarketHandler {
            tx_command_buf,
        }
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
    
    pub async fn assign_market(&self, pair: AssetIdPair) -> MarketCreationResult {
        let (tx_reply, rx_reply) = oneshot::channel();
        let add_market_command = CommandBuffer {
            pair: pair,
            buf: vec![Command::AddMarket(pair)].into(),
        };
        let command_buf = CommandBufferWithReplyChannel {
            command_buf: add_market_command,
            tx_reply,
        };
        // TODO: Investigate safety of these unwraps
        // unwrap fine because if that channel has closed unexpectedly, the whole application should close too
        self.tx_command_buf.send(command_buf).await.unwrap();
        let mut reply_buf = rx_reply.await.unwrap();
        let result = reply_buf.pop_front().unwrap();
        match result {
            CommandResult::AddMarket(result_internal) => {
                result_internal
            }
            _ => Err(MarketCreationError::Other)
        }
    }

    async fn run(
        mut rx_command_buf: mpsc::Receiver<CommandBufferWithReplyChannel>,
        mut tx_balance_manager: mpsc::Sender<blcm::CommandBufferWithReplyChannel>
    ) {
        // Create a buffer to store large number of OrderBuffers into when
        // reading from MPSC channel
        let mut markets: Vec<Market> = Vec::with_capacity(10);
        loop {
            // Check if handler has been assigned a new market. This is
            // Clear channel (we don't want to extend it) and receive `OrderBuffer`s
            let mut channel_buf = Vec::with_capacity(MPSC_CAPACITY);
            let n = rx_command_buf
                .recv_many(&mut channel_buf, MPSC_CAPACITY)
                .await;
            if n == 0 {
                println!("Order channel closed. Shutting down MarketHandler");
                break;
            }

            // Process orders
            for msg in channel_buf {
                let mut response_buf = VecDeque::with_capacity(msg.command_buf.buf.len());
                for command in msg.command_buf.buf {
                    Self::handle_command(command, &mut response_buf, &mut markets, &mut tx_balance_manager).await;
                }
                let _ = msg.tx_reply.send(response_buf);
            }
        }
    }

    async fn handle_command(command: Command, response_buf: &mut CommandResultBuffer, markets: &mut Vec<Market>, tx_balance_manager: &mut mpsc::Sender<blcm::CommandBufferWithReplyChannel>) {
        let result = match command {
            Command::OrderInsert(insertion) => {
                let market_opt = markets.iter_mut().find(|m| m.asset_pair == insertion.pair);
                if let Some(market) = market_opt {
                    let (id, insertion_result) = market.insert_order(insertion);
                    match insertion_result {
                        Ok(effects) => {
                            // process order changes
                            Self::process_order_changes(effects.order_changes, market, tx_balance_manager).await;
                            CommandResult::OrderInsert(Ok(OrderInsertionEffects { id, status: effects.status }))
                        },
                        Err(e) => CommandResult::OrderInsert(Err(e))
                    }
                } else {
                    CommandResult::OrderInsert(Err(OrderInsertionError::MarketDoesNotExist))
                }
            },
            Command::OrderCancel(cancellation) => {
                todo!()
            },
            Command::OrderModify(modification) => {
                todo!()
            },
            Command::AddMarket(pair) => {
                println!("Received new market {pair:?}");
                markets.push(Market::new(pair));
                CommandResult::AddMarket(Ok(()))
            }
        };
        response_buf.push_back(result);
    }

    async fn process_order_changes(order_changes: OrderChangeBuffer, market: &mut Market, tx_balance_manager: &mut mpsc::Sender<blcm::CommandBufferWithReplyChannel>) {
        let mut command_buf = blcm::CommandBuffer::new();
        
        for order_change in order_changes {
            let order = market.session_orders.get_mut(order_change.id).unwrap();

            let primary_side_multiplier = match order.side {
                Side::Ask => -1,
                Side::Bid => 1
            };
            let change_primary = primary_side_multiplier * Balance::from(order_change.change);
            let change_secondary = -1 * change_primary * order.price;

            command_buf.push_back(blcm::Command::BalanceChange(order.account_id, order.pair.primary, change_primary));
            command_buf.push_back(blcm::Command::BalanceChange(order.account_id, order.pair.secondary, change_secondary));
        }
    
        let (tx_blcm_reply, rx_blcm_reply) = oneshot::channel();
        let balance_change_buf = blcm::CommandBufferWithReplyChannel {
            command_buf,
            tx_reply: tx_blcm_reply
        };
        // TODO: check unwrap safety
        tx_balance_manager.send(balance_change_buf).await.unwrap();
        // TODO: maybe wholly remove awaiting this send, don't really need any reply
        rx_blcm_reply.await.unwrap();
    }


}

/// Holds all the different MarketHandlers
#[derive(Debug, Clone)]
pub struct MarketHandlers {
    pub inner: Vec<(MarketHandler, Vec<AssetIdPair>)>,
}

impl MarketHandlers {
    /// Create a new MarketHandlers struct with one handler
    pub fn new(tx_balance_manager: mpsc::Sender<blcm::CommandBufferWithReplyChannel>) -> Self {
        Self::with_handlers(1, tx_balance_manager)
    }

    /// Create a new MarketHandlers struct with n handlers
    pub fn with_handlers(n: usize, tx_balance_manager: mpsc::Sender<blcm::CommandBufferWithReplyChannel>) -> Self {
        let mut result = MarketHandlers { inner: Vec::new() };
        for _ in 0..n {
            result.add_handler(tx_balance_manager.clone());
        }
        result
    }

    /// Checks if there is a handler for the given market
    pub fn contains_market(&self, key: &AssetIdPair) -> bool {
        for (_handler, handler_pairs) in &self.inner {
            let reverse_key = AssetIdPair {
                primary: key.secondary,
                secondary: key.primary
            };
            if handler_pairs.contains(key) || handler_pairs.contains(&reverse_key) {
                return true;
            }
        }
        false
    }

    /// Tries to get the MarketHandler for the given market
    pub fn get_handler(&self, asset_pair: &AssetIdPair) -> Option<&MarketHandler> {
        for (handler, pairs) in &self.inner {
            if pairs.contains(asset_pair) {
                return Some(handler);
            }
        }
        None
    }

    /// Adds a market and assigns it to the handler with the least amount of assigned markets
    pub async fn add_market(&mut self, asset_pair: AssetIdPair) -> MarketCreationResult {
        // Find the handler with least number of assigned markets
        let mut min_handler_idx = 0;
        let first = self
            .inner
            .get(0)
            .ok_or(MarketCreationError::NoMarketHandlers)?;
        let mut min_handler = &first.0;
        let mut min_assigned_pairs = first.1.len();
        for (i, (handler, assigned_pairs)) in self.inner.iter().enumerate() {
            if assigned_pairs.len() < min_assigned_pairs {
                min_handler_idx = i;
                min_assigned_pairs = assigned_pairs.len();
                min_handler = handler;
            }
        }

        min_handler.assign_market(asset_pair).await?;
        self.inner
            .get_mut(min_handler_idx)
            .unwrap()
            .1
            .push(asset_pair);

        Ok(())
    }

    /// Creates a new MarketHandler
    pub fn add_handler(&mut self, tx_balance_manager: mpsc::Sender<blcm::CommandBufferWithReplyChannel>) {
        let handler = MarketHandler::new(tx_balance_manager);
        self.inner.push((handler, Vec::new()));
    }
}