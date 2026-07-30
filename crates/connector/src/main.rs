use adapter_api::AgentAdapter;
use adapter_codex::CodexAdapter;
use adapter_opencode::OpenCodeAdapter;
use event_journal::EventJournal;
use muxport_protocol::{ConnectorState, HostSnapshot};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Muxport Connector Host Daemon v0.1.0...");

    let boot_epoch = chrono::Utc::now().timestamp_millis() as u64;
    let state_db =
        std::env::var("MUXPORT_STATE_DB").unwrap_or_else(|_| "muxport-state.db".into());
    let journal = EventJournal::open_file(&state_db, boot_epoch)?;
    info!(boot_epoch, path = %state_db, "persistent event journal initialized");

    let opencode_adapter = OpenCodeAdapter::new("http://127.0.0.1:4096", None);
    let codex_adapter = CodexAdapter::new("codex");

    if let Err(error) = opencode_adapter.probe().await {
        warn!(%error, "OpenCode runtime is unavailable");
    }
    if let Err(error) = codex_adapter.probe().await {
        warn!(%error, "Codex runtime is unavailable");
    }

    let snapshot = HostSnapshot {
        host_id: "host-local-1".into(),
        hostname: "vps-host".into(),
        connector_state: ConnectorState::Degraded as i32,
        runtimes: vec![],
        credential_profiles: vec![],
        active_sessions: vec![],
        snapshot_sequence: 1,
    };

    journal.save_snapshot(&snapshot)?;
    info!("Authoritative initial snapshot saved to journal");

    warn!(
        "connector transport and mutating adapters are not implemented; running in degraded discovery mode"
    );
    tokio::signal::ctrl_c().await?;
    info!("shutdown signal received");
    Ok(())
}
