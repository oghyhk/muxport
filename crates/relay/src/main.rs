use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

const DEFAULT_BIND: &str = "127.0.0.1:8080";
const DEFAULT_MAX_FRAME_BYTES: usize = 1024 * 1024;
const OUTBOUND_QUEUE_CAPACITY: usize = 64;

type Tx = mpsc::Sender<Message>;

#[derive(Clone)]
struct Subscriber {
    id: Uuid,
    tx: Tx,
}

type ChannelMap = Arc<Mutex<HashMap<String, Vec<Subscriber>>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let addr = std::env::var("MUXPORT_RELAY_BIND").unwrap_or_else(|_| DEFAULT_BIND.into());
    let max_frame_bytes = std::env::var("MUXPORT_RELAY_MAX_FRAME_BYTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_MAX_FRAME_BYTES);
    let socket_addr: SocketAddr = addr.parse()?;
    if !socket_addr.ip().is_loopback() {
        warn!(
            bind = %socket_addr,
            "relay is exposed beyond loopback; terminate TLS and enforce network access controls"
        );
    }

    let listener = TcpListener::bind(socket_addr).await?;
    info!(bind = %socket_addr, "Muxport opaque relay listening");
    let channels: ChannelMap = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (stream, peer) = listener.accept().await?;
        let channels = Arc::clone(&channels);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, channels, max_frame_bytes).await {
                warn!(%peer, %error, "relay connection closed with error");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    channels: ChannelMap,
    max_frame_bytes: usize,
) -> Result<(), String> {
    let mut ws_stream = accept_async(stream)
        .await
        .map_err(|error| format!("WebSocket handshake failed: {error}"))?;

    let first_message = timeout(Duration::from_secs(10), ws_stream.next())
        .await
        .map_err(|_| "route handshake timed out".to_string())?
        .ok_or_else(|| "connection closed before route handshake".to_string())?
        .map_err(|error| format!("route handshake failed: {error}"))?;
    let channel_id = match first_message {
        Message::Text(text) => parse_route_handshake(&text)
            .ok_or_else(|| "invalid route handshake".to_string())?,
        _ => return Err("first message must be a route handshake".into()),
    };

    let connection_id = Uuid::new_v4();
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::channel::<Message>(OUTBOUND_QUEUE_CAPACITY);
    let writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if ws_sender.send(message).await.is_err() {
                break;
            }
        }
    });

    {
        let mut map = channels.lock().await;
        map.entry(channel_id.clone()).or_default().push(Subscriber {
            id: connection_id,
            tx: tx.clone(),
        });
    }
    info!(channel = %channel_id, connection = %connection_id, "client joined relay channel");

    while let Some(message) = ws_receiver.next().await {
        match message.map_err(|error| error.to_string())? {
            Message::Binary(payload) => {
                if payload.len() > max_frame_bytes {
                    warn!(
                        channel = %channel_id,
                        connection = %connection_id,
                        size = payload.len(),
                        max = max_frame_bytes,
                        "dropping oversized relay frame"
                    );
                    break;
                }

                let recipients = {
                    let mut map = channels.lock().await;
                    let subscribers = map.entry(channel_id.clone()).or_default();
                    subscribers.retain(|subscriber| !subscriber.tx.is_closed());
                    subscribers
                        .iter()
                        .filter(|subscriber| subscriber.id != connection_id)
                        .map(|subscriber| subscriber.tx.clone())
                        .collect::<Vec<_>>()
                };
                for recipient in recipients {
                    if recipient.try_send(Message::Binary(payload.clone())).is_err() {
                        warn!(
                            channel = %channel_id,
                            "dropping frame for a slow relay consumer"
                        );
                    }
                }
            }
            Message::Ping(payload) => {
                if tx.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            Message::Text(_) => {
                warn!(
                    channel = %channel_id,
                    connection = %connection_id,
                    "rejecting plaintext application frame"
                );
                break;
            }
            _ => {}
        }
    }

    {
        let mut map = channels.lock().await;
        if let Some(subscribers) = map.get_mut(&channel_id) {
            subscribers.retain(|subscriber| subscriber.id != connection_id);
            if subscribers.is_empty() {
                map.remove(&channel_id);
            }
        }
    }
    drop(tx);
    if let Err(error) = writer.await {
        error!(%error, "relay writer task failed");
    }
    info!(channel = %channel_id, connection = %connection_id, "client left relay channel");
    Ok(())
}

fn parse_route_handshake(message: &str) -> Option<String> {
    let channel = message.strip_prefix("MUXPORT/1 ")?;
    let valid_length = (32..=128).contains(&channel.len());
    let valid_chars = channel
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    (valid_length && valid_chars).then(|| channel.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_handshake_requires_a_high_entropy_safe_token_shape() {
        assert_eq!(
            parse_route_handshake("MUXPORT/1 0123456789abcdef0123456789abcdef"),
            Some("0123456789abcdef0123456789abcdef".into())
        );
        assert_eq!(parse_route_handshake("MUXPORT/1 short"), None);
        assert_eq!(
            parse_route_handshake("MUXPORT/1 0123456789abcdef0123456789abcde/"),
            None
        );
        assert_eq!(
            parse_route_handshake("0123456789abcdef0123456789abcdef"),
            None
        );
    }
}
