use std::{collections::HashMap, net::SocketAddr, time::Duration};

use futures_util::sink::SinkExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use tracing::{Level, debug, info};
use tracing_subscriber::FmtSubscriber;

use backend::{
    asset::AssetIdPair,
    exchange::*,
    exchange_client::ExchangeClient,
    util::{
        exchange_configs,
        format_exact_width::{pad_left, pad_right},
        mp_command_codec::MpCommandCodec,
        statics::*,
    },
};

// Connections < 100 makes this reasonable.
// TODO: make configurable
const BUFFER_SIZE: usize = MB / 2;

/// Handle a connection to the exchange server.
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

/// FmtSubscriber for debug builds
#[cfg(debug_assertions)]
fn get_tracing_subscriber() -> FmtSubscriber {
    FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish()
}

/// FmtSubscriber for non-debug builds
#[cfg(not(debug_assertions))]
fn get_tracing_subscriber() -> FmtSubscriber {
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish()
}

async fn monitor_markets(client: ExchangeClient) {
    let get_all_l1_command = Command::GetAllOrderbookL1();
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut last_messages: HashMap<AssetIdPair, String> = HashMap::new();

    let assets = client.get_assets().await;

    let price_width = 7;
    let vol_width = 4;
    let full_width = price_width + vol_width + 1;
    loop {
        let result = client
            .send_commands([get_all_l1_command].into())
            .await
            .pop_back()
            .unwrap();
        let result_l1s = match result {
            CommandResult::GetAllOrderbookL1(l1s) => l1s,
            _ => panic!(),
        };
        for (pair, l1) in result_l1s {
            let bid_text = match l1.best_bid {
                None => pad_right(String::from(" -"), full_width),
                Some(price_aggr) => format!(
                    "{} {}",
                    pad_right(price_aggr.volume, vol_width),
                    pad_left(format!("{:.3}", price_aggr.price), price_width)
                ),
            };
            let ask_text = match l1.best_ask {
                None => pad_left(String::from("- "), full_width),
                Some(price_aggr) => format!(
                    "{} {}",
                    pad_right(format!("{:.3}", price_aggr.price), price_width),
                    pad_left(price_aggr.volume, vol_width)
                ),
            };

            let primary_symbol = assets
                .iter()
                .find(|asset| asset.id == pair.primary)
                .unwrap()
                .symbol
                .clone();
            let secondary_symbol = assets
                .iter()
                .find(|asset| asset.id == pair.secondary)
                .unwrap()
                .symbol
                .clone();
            let new_message = format!(
                "{primary_symbol}/{secondary_symbol}: [Bid {} | {} Ask]",
                bid_text, ask_text
            );

            // Check if a message was already set, and if so, if the new message is actually different from the previous one.
            let last_message = last_messages.get_mut(&pair);
            match last_message {
                None => {
                    info!("{new_message}");
                    last_messages.insert(pair, new_message);
                }
                Some(message) => {
                    if new_message != *message {
                        info!("{new_message}");
                        last_messages.insert(pair, new_message);
                    }
                }
            }
        }
        interval.tick().await;
    }
}

#[tokio::main]
async fn main() {
    // Set up `tracing`
    let subscriber = get_tracing_subscriber();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Tracing subscriber should be set successfully.");

    // Initialize Exchange and TcpListener
    let (exchange_handle, _pairs, _accounts) =
        exchange_configs::exchange_5fx_markets_5_accs().await;
    let bind_addr = "127.0.0.1:5555";
    let listener = TcpListener::bind(bind_addr).await.unwrap();

    // Start monitor for market
    let monitor_client = exchange_handle.get_client();
    tokio::task::spawn(monitor_markets(monitor_client));

    // Clear terminal screen and reset cursor to (1, 1), then print start message
    print!("\x1B[2J\x1b[1;1H");
    info!("Exchange server started and listening at {bind_addr}.");

    // Run core loop, spawning a Tokio `Task` for each connection
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
