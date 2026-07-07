use tokio::sync::{mpsc, oneshot};

use crate::{
    exchange::{
        Command, CommandBuffer, CommandBufferWithReplyChannel, CommandResult, CommandResultBuffer,
    },
    market::*,
    order::*,
    util::types::*,
};

/// Wrapper around MPSC Sender for sending orders to an exchange
pub struct ExchangeClient {
    pub tx_command_buf: mpsc::Sender<CommandBufferWithReplyChannel>,
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
