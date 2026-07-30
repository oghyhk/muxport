use adapter_api::AgentAdapter;
use adapter_codex::CodexAdapter;
use adapter_opencode::OpenCodeAdapter;
use connector_core::{CommandLedger, DesiredObservedReconciler};
use credential_vault::KeyEncryptionKey;
use event_journal::EventJournal;
use muxport_protocol::{ConnectorState, HostSnapshot, RuntimeState};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Muxport Connector Host Daemon v0.1.0...");

    let boot_epoch = chrono::Utc::now().timestamp_millis() as u64;
    let mut journal = EventJournal::open_in_memory(boot_epoch)?;
    info!(boot_epoch = boot_epoch, "Event journal initialized");

    let opencode_adapter = OpenCodeAdapter::new("http://127.0.0.1:4096", None);
    let codex_adapter = CodexAdapter::new("codex");

    let opencode_caps = opencode_adapter.probe().await?;
    let codex_caps = codex_adapter.probe().await?;

    info!(
        opencode_streaming = opencode_caps.can_stream_deltas,
        codex_streaming = codex_caps.can_stream_deltas,
        "Agent adapters probed successfully"
    );

    let snapshot = HostSnapshot {
        host_id: "host-local-1".into(),
        hostname: "vps-host".into(),
        connector_state: ConnectorState::Ready as i32,
        runtimes: vec![],
        credential_profiles: vec![],
        active_sessions: vec![],
        snapshot_sequence: 1,
    };

    journal.save_snapshot(&snapshot)?;
    info!("Authoritative initial snapshot saved to journal");

    info!("Muxport Host Connector is READY and supervising runtimes.");
    Ok(())
}
