use adapter_api::{AgentAdapter, EventStream};
use adapter_codex::CodexAdapter;
use adapter_opencode::OpenCodeAdapter;
use connector::{journal_runtime_event, replay_events_after_snapshot, RuntimeMirror};
use event_journal::EventJournal;
use futures::StreamExt;
use muxport_protocol::{
    event, AgentType, ConnectorState, Event, RuntimeState, RuntimeStateEvent,
};
use std::error::Error;
use std::time::Duration;
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

const OPENCODE_RUNTIME_NAME: &str = "OpenCode";
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
const HEALTH_INTERVAL: Duration = Duration::from_secs(30);
const DELTAS_PER_SNAPSHOT: usize = 100;

type DynError = Box<dyn Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("starting Muxport connector host daemon");

    // SQLite INTEGER is signed 64-bit; keep the random epoch positive and
    // representable so persistence cannot fail during the first snapshot.
    let boot_epoch = new_boot_epoch();
    let state_db =
        std::env::var("MUXPORT_STATE_DB").unwrap_or_else(|_| "muxport-state.db".into());
    let mut journal = EventJournal::open_file(&state_db, boot_epoch)?;
    if !journal.verify_integrity()? {
        return Err("event journal integrity check failed; preserve the database for recovery".into());
    }
    info!(
        boot_epoch,
        path = %state_db,
        sequence = journal.current_sequence(),
        "persistent event journal initialized"
    );

    let configured_host_id = nonempty_env("MUXPORT_HOST_ID");
    let latest_snapshot = journal.latest_snapshot()?;
    if let (Some(configured), Some(snapshot)) = (&configured_host_id, &latest_snapshot) {
        if configured != &snapshot.host_id {
            return Err(format!(
                "configured host id {configured:?} does not match persisted host identity"
            )
            .into());
        }
    }
    let host_id = latest_snapshot
        .as_ref()
        .map(|snapshot| snapshot.host_id.clone())
        .or(configured_host_id)
        .unwrap_or_else(|| format!("host-{}", uuid::Uuid::new_v4()));
    let hostname = latest_snapshot
        .as_ref()
        .map(|snapshot| snapshot.hostname.clone())
        .filter(|value| !value.is_empty())
        .or_else(|| nonempty_env("MUXPORT_HOSTNAME"))
        .or_else(|| nonempty_env("COMPUTERNAME"))
        .or_else(|| nonempty_env("HOSTNAME"))
        .unwrap_or_else(|| "unnamed-host".into());

    let mut mirror = match latest_snapshot {
        Some(snapshot) => {
            let mut mirror = RuntimeMirror::from_snapshot(snapshot);
            let replayed = replay_events_after_snapshot(&journal, &mut mirror)?;
            info!(replayed, "restored connector projection from snapshot and journal");
            mirror
        }
        None => RuntimeMirror::new(
            &host_id,
            &hostname,
            ConnectorState::Recovering,
            journal.current_sequence(),
        ),
    };
    // Direct/relay mobile transport is not yet implemented, so the connector
    // remains degraded even when its local source mirror is healthy.
    mirror.set_connector_state(ConnectorState::Degraded);
    mirror.save_snapshot(&journal)?;

    let opencode_url = std::env::var("MUXPORT_OPENCODE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4096".into());
    let opencode_password = nonempty_env("MUXPORT_OPENCODE_PASSWORD");
    let runtime_id =
        nonempty_env("MUXPORT_OPENCODE_RUNTIME_ID").unwrap_or_else(|| "opencode-local".into());
    let opencode = OpenCodeAdapter::new(opencode_url, opencode_password);

    let codex_path = nonempty_env("MUXPORT_CODEX_PATH").unwrap_or_else(|| "codex".into());
    let codex = CodexAdapter::new(codex_path);
    match codex.probe().await {
        Ok(_) => warn!(
            "Codex App Server executable is present, but its protocol adapter remains fail-closed"
        ),
        Err(error) => warn!(%error, "Codex runtime is unavailable"),
    }

    warn!(
        "mobile transport is not implemented; local OpenCode state will be mirrored and journaled"
    );

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    let mut reconnect_delay = INITIAL_RECONNECT_DELAY;
    let mut degraded_reported = false;

    'connector: loop {
        let synchronized =
            synchronize_opencode(&opencode, &runtime_id, &mut mirror, &journal).await;
        let mut events = match synchronized {
            Ok(events) => {
                if let Some(version) = opencode.observed_version()? {
                    info!(%version, runtime_id = %runtime_id, "OpenCode mirror synchronized");
                }
                reconnect_delay = INITIAL_RECONNECT_DELAY;
                events
            }
            Err(error) => {
                warn!(%error, runtime_id = %runtime_id, "OpenCode synchronization failed");
                if !degraded_reported {
                    record_degraded(
                        &mut journal,
                        &mut mirror,
                        &runtime_id,
                        "OpenCode snapshot or event subscription unavailable",
                    )?;
                    degraded_reported = true;
                }
                if wait_or_shutdown(&mut shutdown, reconnect_delay).await? {
                    break 'connector;
                }
                reconnect_delay = reconnect_delay
                    .checked_mul(2)
                    .unwrap_or(MAX_RECONNECT_DELAY)
                    .min(MAX_RECONNECT_DELAY);
                continue;
            }
        };

        let mut health_tick = tokio::time::interval(HEALTH_INTERVAL);
        health_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        health_tick.tick().await;
        let mut deltas_since_snapshot = 0;
        let disconnect_reason = loop {
            tokio::select! {
                result = &mut shutdown => {
                    result?;
                    break None;
                }
                item = events.next() => {
                    match item {
                        Some(Ok(event)) => {
                            let is_delta = matches!(
                                event.inner.as_ref(),
                                Some(event::Inner::StreamDelta(_))
                            );
                            if is_delta {
                                deltas_since_snapshot += 1;
                            }
                            let save_snapshot = !is_delta
                                || deltas_since_snapshot >= DELTAS_PER_SNAPSHOT;
                            let sequence = journal_runtime_event(
                                &mut journal,
                                &mut mirror,
                                &runtime_id,
                                event,
                                save_snapshot,
                            )?;
                            if save_snapshot {
                                deltas_since_snapshot = 0;
                            }
                            debug!(sequence, runtime_id = %runtime_id, "journaled OpenCode event");
                        }
                        Some(Err(error)) => break Some(error.to_string()),
                        None => break Some("OpenCode event stream ended".into()),
                    }
                }
                _ = health_tick.tick() => {
                    if let Err(error) =
                        refresh_opencode_snapshot(&opencode, &runtime_id, &mut mirror, &journal).await
                    {
                        break Some(format!("OpenCode health reconciliation failed: {error}"));
                    }
                }
            }
        };

        let Some(reason) = disconnect_reason else {
            break 'connector;
        };
        warn!(%reason, runtime_id = %runtime_id, "OpenCode mirror became stale");
        record_degraded(
            &mut journal,
            &mut mirror,
            &runtime_id,
            "OpenCode event stream disconnected",
        )?;
        degraded_reported = true;
        if wait_or_shutdown(&mut shutdown, reconnect_delay).await? {
            break 'connector;
        }
        reconnect_delay = reconnect_delay
            .checked_mul(2)
            .unwrap_or(MAX_RECONNECT_DELAY)
            .min(MAX_RECONNECT_DELAY);
    }

    mirror.save_snapshot(&journal)?;
    info!("shutdown signal received; final mirror snapshot persisted");
    Ok(())
}

async fn synchronize_opencode(
    adapter: &OpenCodeAdapter,
    runtime_id: &str,
    mirror: &mut RuntimeMirror,
    journal: &EventJournal,
) -> Result<EventStream, DynError> {
    refresh_opencode_snapshot(adapter, runtime_id, mirror, journal).await?;
    let events = adapter.subscribe_events().await?;
    // Subscribe first, then take a second baseline while the response stream
    // buffers source events. This closes the list-before-subscribe race.
    refresh_opencode_snapshot(adapter, runtime_id, mirror, journal).await?;
    Ok(events)
}

async fn refresh_opencode_snapshot(
    adapter: &OpenCodeAdapter,
    runtime_id: &str,
    mirror: &mut RuntimeMirror,
    journal: &EventJournal,
) -> Result<(), DynError> {
    adapter.probe().await?;
    let projects = adapter.discover_projects().await?;
    let sessions = adapter.list_sessions().await?;
    mirror.reconcile_runtime(
        runtime_id,
        AgentType::Opencode,
        OPENCODE_RUNTIME_NAME,
        projects,
        sessions,
    );
    mirror.set_connector_state(ConnectorState::Degraded);
    mirror.save_snapshot(journal)?;
    Ok(())
}

fn record_degraded(
    journal: &mut EventJournal,
    mirror: &mut RuntimeMirror,
    runtime_id: &str,
    details: &str,
) -> Result<(), event_journal::JournalError> {
    let event = Event {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        inner: Some(event::Inner::RuntimeState(RuntimeStateEvent {
            runtime_id: runtime_id.to_owned(),
            agent_type: AgentType::Opencode as i32,
            state: RuntimeState::Degraded as i32,
            active_profile_id: String::new(),
            details: details.to_owned(),
        })),
    };
    journal_runtime_event(journal, mirror, runtime_id, event, true)?;
    Ok(())
}

async fn wait_or_shutdown<F>(
    shutdown: &mut std::pin::Pin<&mut F>,
    delay: Duration,
) -> Result<bool, std::io::Error>
where
    F: std::future::Future<Output = Result<(), std::io::Error>>,
{
    tokio::select! {
        result = shutdown => {
            result?;
            Ok(true)
        }
        _ = tokio::time::sleep(delay) => Ok(false),
    }
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn new_boot_epoch() -> u64 {
    (uuid::Uuid::new_v4().as_u128() & i64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    #[test]
    fn boot_epoch_fits_sqlite_integer() {
        assert!(super::new_boot_epoch() <= i64::MAX as u64);
    }
}
