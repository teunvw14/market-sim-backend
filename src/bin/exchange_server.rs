use std::time::{Instant};
use std::{net::SocketAddr, time::Duration};

use futures_util::sink::SinkExt;
use hdrhistogram::{Histogram, SyncHistogram};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio_stream::StreamExt;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::{Message, Bytes, Utf8Bytes};
use tokio_util::codec::{Framed};
use tracing::{Level, info, debug};
use tracing_subscriber::FmtSubscriber;
use hdrhistogram::sync::{Recorder};

use backend::{
    exchange_client::ExchangeClient,
    util::{
        exchange_configs,
        mp_command_codec::MpCommandCodec,
        statics::*,
    },
};

// Connections < 100 makes this reasonable.
// TODO: make configurable
const BUFFER_SIZE: usize = MB / 2;

const SECOND: Duration = Duration::from_secs(1);
const MINUTE: Duration = Duration::from_secs(60);


fn handle_metrics(mut sync_hist: SyncHistogram<u64>) {
    loop {
        let refresh_timeout = Duration::from_millis(100);
        sync_hist.refresh_timeout(refresh_timeout);
        let p50 = sync_hist.value_at_quantile(0.5f64);
        let p90 = sync_hist.value_at_quantile(0.9f64);
        let p999 = sync_hist.value_at_quantile(0.999f64);
        info!("Latency: p50: {p50}, p90: {p90} p99.9: {p999}");
        sync_hist.clear();
        std::thread::sleep(MINUTE);
    }
}


/// Handle a connection to the exchange server.
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    client: ExchangeClient,
    mut metrics_recorder: Recorder<u64>,
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
                let start = Instant::now();
                // Insert commands and send result
                let num_commands = decoded_commands.len();
                let res = client.send_commands(decoded_commands).await;
                let latency = start.elapsed();
                framed_stream.send(res).await.unwrap();
                let _ = metrics_recorder.record_n(latency.as_micros() as u64, num_commands as u64);
            }
        }
    }
}

async fn handle_connection_ws(
    mut ws_stream: WebSocketStream<TcpStream>,
    addr: SocketAddr,
    client: ExchangeClient,
) {
    info!("New WebSocket connection: {addr}");

    // Send asset information
    let all_assets = client.get_assets().await;
    if let Ok(bytes) = rmp_serde::to_vec(&all_assets) {            
        let message = Message::Binary(bytes.into());
        _ = ws_stream.send(message).await;
    }
    
    // Send l1 data in a loop
    loop {        
        let all_l1s = client.get_all_l1().await;
        if let Ok(bytes) = rmp_serde::to_vec(&all_l1s) {            
            let message = Message::Binary(bytes.into());
            _ = ws_stream.send(message).await;
        }
        
        tokio::time::sleep(Duration::from_millis(10)).await;
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


#[tokio::main]
async fn main() {
    // Set up `tracing`
    let subscriber = get_tracing_subscriber();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Tracing subscriber should be set successfully.");

    // Initialize Exchange
    let (exchange_handle, _pairs, _accounts) =
        exchange_configs::exchange_5fx_markets_5_accs().await;

    // Bind TcpListener for client server and WebSocket server
    let bind_addr_client = "127.0.0.1:5555";
    let listener_client = TcpListener::bind(bind_addr_client).await.unwrap();
    
    let bind_addr_ws = "127.0.0.1:5556";
    let listener_ws = TcpListener::bind(bind_addr_ws).await.unwrap();

    // Enable metrics HDRHistogram
    let sync_hist = SyncHistogram::from(Histogram::<u64>::new_with_bounds(1, 1_000_000, 3).unwrap());
    let mut prime_recorder = sync_hist.recorder().into_idle();
    std::thread::spawn(|| handle_metrics(sync_hist));

    // Clear terminal screen and reset cursor to (1, 1), then print start message
    print!("\x1B[2J\x1b[1;1H");
    info!("Exchange server started and listening at {bind_addr_client}.");

    // Run core loop, spawning a Tokio `Task` for each connection
    loop {
        select! {
            Ok((stream, addr)) = listener_client.accept() => {
                let exchange_client = exchange_handle.get_client();
                let active_recorder = prime_recorder.activate();
                let metrics_recorder = active_recorder.clone();
                prime_recorder = active_recorder.into_idle();
                tokio::task::spawn(handle_connection(
                    stream,
                    addr,
                    exchange_client,
                    metrics_recorder,
                    BUFFER_SIZE,
                ));
            },
            Ok((stream, addr)) = listener_ws.accept() => {
                if let Ok(stream_ws) = tokio_tungstenite::accept_async(stream).await {
                    let exchange_client = exchange_handle.get_client();
                    tokio::task::spawn(handle_connection_ws(stream_ws, addr, exchange_client));
                }
            }
        }
    }
}
