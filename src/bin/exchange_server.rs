use std::{
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};
use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::{Framed, FramedRead};
use tokio_stream::StreamExt;
use futures_util::sink::SinkExt;
use tracing::{Level, debug, info, warn, error};
use tracing_subscriber::FmtSubscriber;

use num_format::{CustomFormat, ToFormattedString};

use backend::{asset::*, exchange::*, exchange_configs, mp_command_encoding::{MpCommandCodec}, order::*, statics::*, types::*};

// Default, should be made configurable. Connections < 100 makes this a
// reasonable default.
const BUFFER_SIZE: usize = MB / 2;

/// Handle connections reading and writing
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    client: ExchangeClient,
    buffer_size: usize,
) {
    info!("New connection: {addr}");

    let command_codec = MpCommandCodec::new();
    let mut framed_stream = Framed::with_capacity(stream, command_codec, buffer_size);

    loop {
        match framed_stream.next().await {
            None => {
                info!("Disconnected: {addr}");
                break;
            }
            Some(Err(e)) => {
                info!("Disconnecting {addr} due to error: '{e}'");
                break;
            }
            Some(Ok(decoded_commands)) => {
                // Insert commands and send result
                let res = client.send_commands(decoded_commands).await;
                framed_stream.send(res).await.unwrap();
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up `tracing`
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Tracing subscriber should be set successfully.");

    
    // Initialize Exchange and TcpListener
    let (exchange_handle, pair, id1, id2) = exchange_configs::exchange_eur_usd_market_2_accs().await;
    let bind_addr = "127.0.0.1:5555";
    let listener = TcpListener::bind(bind_addr).await.unwrap();
    
    info!("Exchange server started and listening at {bind_addr}.");
    
    // Run core loop
    loop {
        if let Ok((stream, addr)) = listener.accept().await {
            let exchange_client = exchange_handle.get_client();
            tokio::task::spawn(handle_connection(
                stream,
                addr,
                exchange_client,
                BUFFER_SIZE,
            ));
        }
    }
}
