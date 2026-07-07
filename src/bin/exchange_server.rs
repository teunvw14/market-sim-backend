use std::{net::SocketAddr, time::Duration};

use futures_util::sink::SinkExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use tracing::{Level, debug, info};
use tracing_subscriber::FmtSubscriber;

use backend::{
    asset::AssetIdPair, exchange::*, exchange_client::ExchangeClient, util::{exchange_configs, mp_command_codec::MpCommandCodec, statics::*},
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

async fn monitor_market(client: ExchangeClient, pair: AssetIdPair) {
    let get_l1_command = Command::GetOrderbookL1(pair);
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        let result = client.send_commands([get_l1_command].into()).await
            .pop_back()
            .unwrap();
        if let CommandResult::GetOrderbookL1(result_l1) = result {
            if let Ok(l1) = result_l1 {
                let bid_text = match l1.best_bid {
                    None => String::from("-"),
                    Some(price_aggr) => format!("({:.3}) {:.3}", price_aggr.volume, price_aggr.price),
                };
                let ask_text = match l1.best_ask {
                    None => String::from("-"),
                    Some(price_aggr) => format!("{:.3} ({:.3})", price_aggr.price, price_aggr.volume),
                };
                info!("EUR/USD: Bid {} / {} Ask", bid_text, ask_text);
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
    let (exchange_handle, pair, _id1, _id2) =
        exchange_configs::exchange_eur_usd_market_2_accs().await;
    let bind_addr = "127.0.0.1:5555";
    let listener = TcpListener::bind(bind_addr).await.unwrap();

    // Start monitor for market
    let monitor_client = exchange_handle.get_client();
    tokio::task::spawn(monitor_market(monitor_client, pair));

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
