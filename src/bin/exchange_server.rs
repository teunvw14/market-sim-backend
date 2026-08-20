use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{net::SocketAddr, time::Duration};

use backend::asset::{Asset, AssetIdPair, AssetPairSymbolic, NewAsset};
use backend::exchange::Transaction;
use backend::orderbook::OrderbookL1;
use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use futures_util::sink::SinkExt;
use hdrhistogram::sync::Recorder;
use hdrhistogram::{Histogram, SyncHistogram};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::{Bytes, Message, Utf8Bytes};
use tokio_util::codec::Framed;
use tracing::{Level, debug, info};
use tracing_subscriber::FmtSubscriber;

use backend::{
    exchange_client::ExchangeClient,
    util::{exchange_configs, mp_command_codec::MpCommandCodec, statics::*},
};

#[derive(Deserialize, Serialize, Clone)]
struct ExchangeServerConfig {
    /// The buffer size for TCP connections.
    tcp_buffer_size: usize,
    /// The interval at which metrics are collected
    metrics_interval_ms: u64,
    /// The interval at which the exchange state is sent to connected WebSockets
    ws_send_interval_ms: u64,
    /// Address that the ExchangeServer's client server is exposed on
    bind_address_client: String,
    /// Address that the ExchangeServer's WebSocket server is exposed on
    bind_address_websocket: String,
    /// Starting number of accounts
    starting_accounts: usize,
    /// Starting Assets
    starting_assets: Vec<NewAsset>,
    /// Starting Assets
    starting_markets: Vec<AssetPairSymbolic>,
}

impl Default for ExchangeServerConfig {
    fn default() -> ExchangeServerConfig {
        ExchangeServerConfig {
            tcp_buffer_size: MB / 2,
            metrics_interval_ms: 1000,
            ws_send_interval_ms: 500,
            bind_address_client: "127.0.0.1:5555".to_string(),
            bind_address_websocket: "127.0.0.1:5556".to_string(),
            starting_accounts: 2,
            starting_assets: Vec::new(),
            starting_markets: Vec::new(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize)]
struct ExchangeMetrics {
    timestamp: u64,
    p50: u64,
    p90: u64,
    p999: u64,
}

#[derive(Serialize)]
struct ExchangeState {
    l1s: Vec<(AssetIdPair, OrderbookL1)>,
    metrics: ExchangeMetrics,
    last_100_tx: Vec<Transaction>,
}

/// Collect metrics from HDRHistograms and publish them through a `watch`
/// channel. Collects metrics over the given `interval`.
fn collect_metrics(
    mut sync_hist: SyncHistogram<u64>,
    publisher: watch::Sender<ExchangeMetrics>,
    interval: Duration,
) {
    loop {
        let refresh_timeout = Duration::from_millis(500);
        sync_hist.refresh_timeout(refresh_timeout);

        // Unwrap safety: UNIX_EPOCH will never be earlier than now.
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let metrics = ExchangeMetrics {
            timestamp,
            p50: sync_hist.value_at_quantile(0.5f64),
            p90: sync_hist.value_at_quantile(0.9f64),
            p999: sync_hist.value_at_quantile(0.999f64),
        };
        sync_hist.clear();

        // If the send gives an error, all receivers have been dropped, so this
        // process should stop.
        if publisher.send(metrics).is_err() {
            break;
        }
        std::thread::sleep(interval);
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
                // Record start for latency metrics
                let start = Instant::now();

                // Insert commands and send result
                let num_commands = decoded_commands.len();
                let commands_results = client.send_commands(decoded_commands).await;
                let latency = start.elapsed();
                framed_stream.send(commands_results).await.unwrap();

                // Record latency in microseconds
                let _ = metrics_recorder.record_n(latency.as_micros() as u64, num_commands as u64);
            }
        }
    }
}

async fn handle_connection_ws(
    mut ws_stream: WebSocketStream<TcpStream>,
    addr: SocketAddr,
    client: ExchangeClient,
    rx_metrics: watch::Receiver<ExchangeMetrics>,
    send_interval: Duration,
) {
    info!("New WebSocket connection: {addr}");

    // Send asset information on initial connection
    let all_assets = client.get_assets().await;
    if let Ok(bytes) = rmp_serde::to_vec(&all_assets) {
        let message = Message::Binary(bytes.into());
        _ = ws_stream.send(message).await;
    }

    // Send ExchangeState in a loop
    loop {
        let l1s = client.get_all_l1().await;
        let metrics = *rx_metrics.borrow();
        let last_100_tx = client.get_last_100_transactions().await;

        let exchange_state = ExchangeState {
            l1s,
            metrics,
            last_100_tx,
        };

        if let Ok(bytes) = rmp_serde::to_vec(&exchange_state) {
            let message = Message::Binary(bytes.into());
            _ = ws_stream.send(message).await;
        }

        tokio::time::sleep(send_interval).await;
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

/// Config profile for debug builds
#[cfg(debug_assertions)]
fn get_config_profile() -> String {
    "dev".to_string()
}

/// Config profile for non-debug builds
#[cfg(not(debug_assertions))]
fn get_config_profile() -> String {
    "prod".to_string()
}

#[tokio::main]
async fn main() {
    // Set up `tracing`
    let subscriber = get_tracing_subscriber();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Tracing subscriber should be set successfully.");

    // Load Exchange Server config. Overwrite configuration from Config.toml
    // with configuration set through environment variables. First extract
    // build-specific configuration from Config.toml
    let config_build =
        Figment::from(Toml::file("Config.toml").nested()).select(get_config_profile());
    let config: ExchangeServerConfig =
        Figment::from(Serialized::defaults(ExchangeServerConfig::default()))
            .merge(config_build)
            .extract()
            .unwrap();
    let metrics_interval = Duration::from_millis(config.metrics_interval_ms);
    let ws_send_interval = Duration::from_millis(config.ws_send_interval_ms);
    println!("ws_send_interval: {ws_send_interval:?}");

    // Initialize Exchange
    let (exchange_handle, _pairs, _accounts) = exchange_configs::custom(
        config.starting_assets,
        config.starting_markets,
        config.starting_accounts,
    );

    // Bind TcpListener for client server and WebSocket server
    let listener_client = TcpListener::bind(&config.bind_address_client)
        .await
        .unwrap();
    let listener_ws = TcpListener::bind(&config.bind_address_websocket)
        .await
        .unwrap();

    // Enable metrics HDRHistogram
    let sync_hist =
        SyncHistogram::from(Histogram::<u64>::new_with_bounds(1, 1_000_000, 3).unwrap());
    let mut prime_recorder = sync_hist.recorder().into_idle();
    let (tx_metrics, rx_metrics) = watch::channel(ExchangeMetrics::default());
    std::thread::spawn(move || collect_metrics(sync_hist, tx_metrics, metrics_interval));

    // Clear terminal screen and reset cursor to (1, 1), then print start message
    print!("\x1B[2J\x1b[1;1H");
    info!(
        "Exchange server started and listening at {}.",
        &config.bind_address_client
    );
    info!(
        "Exchange server listening for WebSocket connections at {}.",
        &config.bind_address_websocket
    );

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
                    config.tcp_buffer_size,
                ));
            },
            Ok((stream, addr)) = listener_ws.accept() => {
                if let Ok(stream_ws) = tokio_tungstenite::accept_async(stream).await {
                    let exchange_client = exchange_handle.get_client();
                    tokio::task::spawn(handle_connection_ws(stream_ws, addr, exchange_client, rx_metrics.clone(), ws_send_interval));
                }
            }
        }
    }
}
