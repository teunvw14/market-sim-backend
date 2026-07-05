use std::{
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

use num_format::{CustomFormat, ToFormattedString};

use backend::{asset::*, exchange::*, exchange_configs, order::*, statics::*, types::*};

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

    // Initialize read buffer
    let mut buf = vec![0u8; buffer_size];

    loop {
        if stream.readable().await.is_err() {
            continue;
        }
        match stream.try_read(&mut buf) {
            Ok(0) => {
                info!("Disconnected: {addr}");
                break;
            }
            Ok(n) => {
                // Echo back to client
                loop {
                    if stream.writable().await.is_err() {
                        continue;
                    };
                    match stream.try_write(&buf[..n]) {
                        Ok(n) => {
                            println!("Sent back {n} bytes");
                            break;
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                        Err(e) => {
                            warn!("Failed to write to {addr}: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                error!("Failed to read from {addr}: {}", e);
                break;
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
    let (exchange_handle, pair) = exchange_configs::exchange_eur_usd_market().await;
    let listener = TcpListener::bind("127.0.0.1:5555").await.unwrap();

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
