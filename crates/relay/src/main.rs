use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

type Tx = tokio::sync::mpsc::UnboundedSender<Message>;
type ChannelMap = Arc<Mutex<HashMap<String, Vec<Tx>>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("Muxport Opaque Relay Listening on ws://{}", addr);

    let channels: ChannelMap = Arc::new(Mutex::new(HashMap::new()));

    while let Ok((stream, _)) = listener.accept().await {
        let channels_clone = Arc::clone(&channels);
        tokio::spawn(handle_connection(stream, channels_clone));
    }

    Ok(())
}

async fn handle_connection(stream: TcpStream, channels: ChannelMap) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let _ = ws_sender.send(msg).await;
        }
    });

    let channel_id = "default_test_channel".to_string();
    {
        let mut map = channels.lock().await;
        map.entry(channel_id.clone()).or_default().push(tx);
    }

    info!(channel = %channel_id, "Client connected to relay channel");

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if msg.is_binary() || msg.is_text() {
            let map = channels.lock().await;
            if let Some(subscribers) = map.get(&channel_id) {
                for sub in subscribers {
                    let _ = sub.send(msg.clone());
                }
            }
        }
    }
}
