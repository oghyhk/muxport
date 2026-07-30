use adapter_api::{
    AdapterError, AgentAdapter, EventStream, ProjectInfo, SessionSummary,
};
use adapter_codex::CodexAdapter;
use adapter_opencode::OpenCodeAdapter;
use connector::{
    journal_runtime_event, replay_events_after_snapshot, CommandRouter, RuntimeMirror,
};
use event_journal::EventJournal;
use futures::StreamExt;
use muxport_protocol::{
    event, AgentType, ConnectorState, Event, RuntimeState, RuntimeStateEvent,
};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
const HEALTH_INTERVAL: Duration = Duration::from_secs(30);
const DELTAS_PER_SNAPSHOT: usize = 100;
const SOURCE_UPDATE_CAPACITY: usize = 512;

type DynError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct RuntimeConfig {
    runtime_id: String,
    agent_type: AgentType,
    runtime_name: &'static str,
}

enum SourceUpdate {
    Snapshot {
        config: RuntimeConfig,
        projects: Vec<ProjectInfo>,
        sessions: Vec<SessionSummary>,
    },
    Event {
        config: RuntimeConfig,
        event: Event,
    },
    Degraded {
        config: RuntimeConfig,
        details: String,
    },
}

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
    // remains degraded even when all local source mirrors are healthy.
    mirror.set_connector_state(ConnectorState::Degraded);
    mirror.save_snapshot(&journal)?;

    let opencode_url = std::env::var("MUXPORT_OPENCODE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4096".into());
    let opencode_password = nonempty_env("MUXPORT_OPENCODE_PASSWORD");
    let opencode_config = RuntimeConfig {
        runtime_id: nonempty_env("MUXPORT_OPENCODE_RUNTIME_ID")
            .unwrap_or_else(|| "opencode-local".into()),
        agent_type: AgentType::Opencode,
        runtime_name: "OpenCode",
    };
    let opencode: Arc<dyn AgentAdapter> =
        Arc::new(OpenCodeAdapter::new(opencode_url, opencode_password));

    let codex_path = nonempty_env("MUXPORT_CODEX_PATH").unwrap_or_else(|| "codex".into());
    let codex_config = RuntimeConfig {
        runtime_id: nonempty_env("MUXPORT_CODEX_RUNTIME_ID")
            .unwrap_or_else(|| "codex-local".into()),
        agent_type: AgentType::Codex,
        runtime_name: "Codex",
    };
    let codex: Arc<dyn AgentAdapter> = Arc::new(CodexAdapter::new(codex_path));

    let runtimes = vec![
        (opencode_config, opencode),
        (codex_config, codex),
    ];
    let command_db = std::env::var("MUXPORT_COMMAND_DB")
        .unwrap_or_else(|_| "muxport-commands.db".into());
    let adapter_registry = runtimes
        .iter()
        .map(|(config, adapter)| {
            (config.runtime_id.clone(), Arc::clone(adapter))
        })
        .collect();
    let _command_router = CommandRouter::open_sqlite(&command_db, adapter_registry)?;
    info!(path = %command_db, "persistent command ledger initialized");

    let (updates_tx, mut updates_rx) = mpsc::channel(SOURCE_UPDATE_CAPACITY);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut monitor_tasks = Vec::new();
    for (config, adapter) in &runtimes {
        monitor_tasks.push(tokio::spawn(monitor_runtime(
            Arc::clone(adapter),
            config.clone(),
            updates_tx.clone(),
            shutdown_rx.clone(),
        )));
    }
    drop(updates_tx);

    warn!(
        "mobile transport is not implemented; local OpenCode and Codex state will be mirrored and journaled"
    );

    let mut shutdown = Box::pin(tokio::signal::ctrl_c());
    let mut deltas_since_snapshot: HashMap<String, usize> = HashMap::new();
    loop {
        tokio::select! {
            result = &mut shutdown => {
                result?;
                break;
            }
            update = updates_rx.recv() => {
                let Some(update) = update else {
                    warn!("all runtime monitor tasks stopped");
                    break;
                };
                apply_source_update(
                    update,
                    &mut journal,
                    &mut mirror,
                    &mut deltas_since_snapshot,
                )?;
            }
        }
    }

    let _ = shutdown_tx.send(true);
    drop(updates_rx);
    for (config, adapter) in &runtimes {
        if let Err(error) = adapter.shutdown_gracefully().await {
            warn!(
                %error,
                runtime_id = %config.runtime_id,
                "runtime adapter did not shut down cleanly"
            );
        }
    }
    for task in monitor_tasks {
        if let Err(error) = task.await {
            warn!(%error, "runtime monitor task failed");
        }
    }

    mirror.save_snapshot(&journal)?;
    info!("shutdown signal received; final mirror snapshot persisted");
    Ok(())
}

fn apply_source_update(
    update: SourceUpdate,
    journal: &mut EventJournal,
    mirror: &mut RuntimeMirror,
    deltas_since_snapshot: &mut HashMap<String, usize>,
) -> Result<(), event_journal::JournalError> {
    match update {
        SourceUpdate::Snapshot {
            config,
            projects,
            sessions,
        } => {
            mirror.reconcile_runtime(
                &config.runtime_id,
                config.agent_type,
                config.runtime_name,
                projects,
                sessions,
            );
            mirror.set_connector_state(ConnectorState::Degraded);
            mirror.save_snapshot(journal)?;
            info!(
                runtime_id = %config.runtime_id,
                runtime = config.runtime_name,
                "runtime mirror synchronized"
            );
        }
        SourceUpdate::Event { config, event } => {
            let is_delta = matches!(
                event.inner.as_ref(),
                Some(event::Inner::StreamDelta(_))
            );
            let delta_count = deltas_since_snapshot
                .entry(config.runtime_id.clone())
                .or_default();
            if is_delta {
                *delta_count += 1;
            }
            let save_snapshot = !is_delta || *delta_count >= DELTAS_PER_SNAPSHOT;
            let sequence = journal_runtime_event(
                journal,
                mirror,
                &config.runtime_id,
                event,
                save_snapshot,
            )?;
            if save_snapshot {
                *delta_count = 0;
            }
            debug!(
                sequence,
                runtime_id = %config.runtime_id,
                "journaled runtime event"
            );
        }
        SourceUpdate::Degraded { config, details } => {
            record_degraded(journal, mirror, &config, &details)?;
            warn!(
                runtime_id = %config.runtime_id,
                runtime = config.runtime_name,
                %details,
                "runtime mirror became stale"
            );
        }
    }
    Ok(())
}

async fn monitor_runtime(
    adapter: Arc<dyn AgentAdapter>,
    config: RuntimeConfig,
    updates: mpsc::Sender<SourceUpdate>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut reconnect_delay = INITIAL_RECONNECT_DELAY;
    let mut degraded_reported = false;
    loop {
        if *shutdown.borrow() || updates.is_closed() {
            return;
        }

        let synchronized =
            synchronize_runtime(adapter.as_ref(), &config, &updates).await;
        let mut events = match synchronized {
            Ok(events) => {
                reconnect_delay = INITIAL_RECONNECT_DELAY;
                degraded_reported = false;
                events
            }
            Err(error) => {
                if updates.is_closed() || *shutdown.borrow() {
                    return;
                }
                warn!(
                    %error,
                    runtime_id = %config.runtime_id,
                    runtime = config.runtime_name,
                    "runtime synchronization failed"
                );
                if !degraded_reported {
                    if updates
                        .send(SourceUpdate::Degraded {
                            config: config.clone(),
                            details: "runtime snapshot or event subscription unavailable".into(),
                        })
                        .await
                        .is_err()
                    {
                        return;
                    }
                    degraded_reported = true;
                }
                if wait_for_shutdown(&mut shutdown, reconnect_delay).await {
                    return;
                }
                reconnect_delay = next_reconnect_delay(reconnect_delay);
                continue;
            }
        };

        let mut health_tick = tokio::time::interval(HEALTH_INTERVAL);
        health_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        health_tick.tick().await;
        let disconnect_reason = loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return;
                    }
                }
                item = events.next() => {
                    match item {
                        Some(Ok(event)) => {
                            if updates
                                .send(SourceUpdate::Event {
                                    config: config.clone(),
                                    event,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Some(Err(error)) => break error.to_string(),
                        None => break "runtime event stream ended".into(),
                    }
                }
                _ = health_tick.tick() => {
                    if let Err(error) =
                        refresh_runtime_snapshot(adapter.as_ref(), &config, &updates).await
                    {
                        if updates.is_closed() || *shutdown.borrow() {
                            return;
                        }
                        break format!("health reconciliation failed: {error}");
                    }
                }
            }
        };

        if !degraded_reported {
            if updates
                .send(SourceUpdate::Degraded {
                    config: config.clone(),
                    details: disconnect_reason,
                })
                .await
                .is_err()
            {
                return;
            }
            degraded_reported = true;
        }
        if wait_for_shutdown(&mut shutdown, reconnect_delay).await {
            return;
        }
        reconnect_delay = next_reconnect_delay(reconnect_delay);
    }
}

async fn synchronize_runtime(
    adapter: &dyn AgentAdapter,
    config: &RuntimeConfig,
    updates: &mpsc::Sender<SourceUpdate>,
) -> Result<EventStream, AdapterError> {
    refresh_runtime_snapshot(adapter, config, updates).await?;
    let events = adapter.subscribe_events().await?;
    // Subscribe first, then take a second baseline while the adapter buffers
    // source events. This closes the list-before-subscribe race.
    refresh_runtime_snapshot(adapter, config, updates).await?;
    Ok(events)
}

async fn refresh_runtime_snapshot(
    adapter: &dyn AgentAdapter,
    config: &RuntimeConfig,
    updates: &mpsc::Sender<SourceUpdate>,
) -> Result<(), AdapterError> {
    adapter.probe().await?;
    let projects = adapter.discover_projects().await?;
    let sessions = adapter.list_sessions().await?;
    updates
        .send(SourceUpdate::Snapshot {
            config: config.clone(),
            projects,
            sessions,
        })
        .await
        .map_err(|_| AdapterError::Internal("connector update receiver closed".into()))
}

fn record_degraded(
    journal: &mut EventJournal,
    mirror: &mut RuntimeMirror,
    config: &RuntimeConfig,
    details: &str,
) -> Result<(), event_journal::JournalError> {
    let event = Event {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        inner: Some(event::Inner::RuntimeState(RuntimeStateEvent {
            runtime_id: config.runtime_id.clone(),
            agent_type: config.agent_type as i32,
            state: RuntimeState::Degraded as i32,
            active_profile_id: String::new(),
            details: details.to_owned(),
        })),
    };
    journal_runtime_event(journal, mirror, &config.runtime_id, event, true)?;
    Ok(())
}

async fn wait_for_shutdown(
    shutdown: &mut watch::Receiver<bool>,
    delay: Duration,
) -> bool {
    tokio::select! {
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
        _ = tokio::time::sleep(delay) => false,
    }
}

fn next_reconnect_delay(delay: Duration) -> Duration {
    delay
        .checked_mul(2)
        .unwrap_or(MAX_RECONNECT_DELAY)
        .min(MAX_RECONNECT_DELAY)
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
    use super::*;
    use test_harness::DeterministicFakeAdapter;

    #[test]
    fn boot_epoch_fits_sqlite_integer() {
        assert!(new_boot_epoch() <= i64::MAX as u64);
    }

    #[test]
    fn reconnect_delay_is_bounded() {
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(30)),
            MAX_RECONNECT_DELAY
        );
    }

    #[tokio::test]
    async fn synchronization_brackets_subscription_with_snapshots() {
        let adapter = DeterministicFakeAdapter::new(AgentType::Codex);
        let config = RuntimeConfig {
            runtime_id: "codex-test".into(),
            agent_type: AgentType::Codex,
            runtime_name: "Codex",
        };
        let (updates_tx, mut updates_rx) = mpsc::channel(4);
        let mut events = synchronize_runtime(&adapter, &config, &updates_tx)
            .await
            .unwrap();

        for _ in 0..2 {
            match updates_rx.recv().await.unwrap() {
                SourceUpdate::Snapshot {
                    config,
                    projects,
                    sessions,
                } => {
                    assert_eq!(config.runtime_id, "codex-test");
                    assert_eq!(projects.len(), 1);
                    assert_eq!(sessions.len(), 1);
                }
                _ => panic!("synchronization emitted a non-snapshot baseline"),
            }
        }
        assert!(events.next().await.is_none());
    }
}
