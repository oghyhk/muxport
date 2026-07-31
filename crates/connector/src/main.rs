use adapter_api::{
    AdapterError, AgentAdapter, EventStream, ProjectInfo, SessionSummary,
};
use adapter_codex::{CodexAdapter, CodexLoginStart, ManagedCodexProfile};
use adapter_opencode::{ManagedOpenCodeProfile, OpenCodeAdapter};
use connector::{
    journal_runtime_event, replay_events_after_snapshot, CommandRouter,
    ConfirmingParty, DirectTransportService, InstanceLock,
    PairingCoordinator, PairingStore, PairingStoreError, RuntimeMirror,
};
use credential_vault::{
    HostIdentityManager, OsHostIdentityStore, OsVaultKeyStore, PersistentVault,
    VaultKeyManager,
};
use event_journal::EventJournal;
use futures::StreamExt;
use muxport_protocol::{
    event, AgentType, ConnectorState, CredentialProfileInfo, Event, RuntimeState,
    RuntimeStateEvent,
};
use serde::Deserialize;
use rand::Rng;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
const MAX_CONFIGURED_RECONNECT_DELAY: Duration = Duration::from_secs(5 * 60);
const HEALTH_INTERVAL: Duration = Duration::from_secs(30);
const DELTAS_PER_SNAPSHOT: usize = 100;
const SOURCE_UPDATE_CAPACITY: usize = 512;
const MANAGED_RESTART_WINDOW: Duration = Duration::from_secs(10 * 60);
const MAX_MANAGED_FAILURES_PER_WINDOW: usize = 5;
const RUNTIME_MANIFEST_VERSION: u32 = 1;
const MAX_RUNTIME_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_MANAGED_RUNTIMES: usize = 64;

type DynError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct RuntimeConfig {
    runtime_id: String,
    agent_type: AgentType,
    runtime_name: String,
    credential_profile_id: String,
    managed_opencode: Option<Arc<tokio::sync::Mutex<ManagedOpenCodeChild>>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeManifest {
    version: u32,
    profiles_root: PathBuf,
    runtimes: Vec<RuntimeManifestEntry>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
enum ManifestAgentType {
    Opencode,
    Codex,
}

impl ManifestAgentType {
    fn protocol_type(self) -> AgentType {
        match self {
            Self::Opencode => AgentType::Opencode,
            Self::Codex => AgentType::Codex,
        }
    }

    fn default_name(self) -> &'static str {
        match self {
            Self::Opencode => "OpenCode",
            Self::Codex => "Codex",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeManifestEntry {
    runtime_id: String,
    agent_type: ManifestAgentType,
    profile_id: String,
    executable: PathBuf,
    project_directory: PathBuf,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    display_name: Option<String>,
}

struct ManagedOpenCodeChild {
    profile: ManagedOpenCodeProfile,
    server_password: String,
    child: Option<tokio::process::Child>,
    restart_attempts: VecDeque<Instant>,
    crash_loop_tripped: bool,
}

impl ManagedOpenCodeChild {
    fn start(
        profile: ManagedOpenCodeProfile,
        server_password: String,
    ) -> Result<Self, adapter_opencode::ManagedOpenCodeError> {
        let child = profile.spawn(&server_password)?;
        Ok(Self {
            profile,
            server_password,
            child: Some(child),
            restart_attempts: VecDeque::new(),
            crash_loop_tripped: false,
        })
    }

    fn ensure_running(&mut self) -> Result<bool, AdapterError> {
        if self.crash_loop_tripped {
            return Err(AdapterError::InitFailed(
                "managed OpenCode crash loop is latched; operator restart is required".into(),
            ));
        }
        let restart = match self.child.as_mut() {
            Some(child) => child.try_wait().map_err(|error| {
                AdapterError::InitFailed(format!(
                    "managed OpenCode process status failed: {error}"
                ))
            })?.is_some(),
            None => true,
        };
        if !restart {
            return Ok(false);
        }
        let now = Instant::now();
        while self
            .restart_attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= MANAGED_RESTART_WINDOW)
        {
            self.restart_attempts.pop_front();
        }
        self.restart_attempts.push_back(now);
        if self.restart_attempts.len() >= MAX_MANAGED_FAILURES_PER_WINDOW {
            self.child = None;
            self.crash_loop_tripped = true;
            return Err(AdapterError::InitFailed(format!(
                "managed OpenCode crash loop: {} failures within {} seconds; automatic restart stopped",
                MAX_MANAGED_FAILURES_PER_WINDOW,
                MANAGED_RESTART_WINDOW.as_secs()
            )));
        }
        self.child = Some(self.profile.spawn(&self.server_password).map_err(|error| {
            AdapterError::InitFailed(format!(
                "managed OpenCode process restart failed: {error}"
            ))
        })?);
        Ok(true)
    }

    async fn stop(&mut self) -> Result<(), AdapterError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        if child.try_wait().map_err(|error| {
            AdapterError::Internal(format!("managed OpenCode process status failed: {error}"))
        })?.is_none()
        {
            child.kill().await.map_err(|error| {
                AdapterError::Internal(format!("managed OpenCode process stop failed: {error}"))
            })?;
            let _ = child.wait().await;
        }
        Ok(())
    }
}

enum SourceUpdate {
    Snapshot {
        config: RuntimeConfig,
        projects: Vec<ProjectInfo>,
        sessions: Vec<SessionSummary>,
        persisted: oneshot::Sender<Result<(), String>>,
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
    if run_local_subcommand().await? {
        return Ok(());
    }
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("starting Muxport connector host daemon");

    let state_db =
        std::env::var("MUXPORT_STATE_DB").unwrap_or_else(|_| "muxport-state.db".into());
    let lock_path = nonempty_env("MUXPORT_LOCK_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| InstanceLock::default_path_for(&state_db));
    let _instance_lock = InstanceLock::acquire(&lock_path)?;
    info!(path = %lock_path.display(), "exclusive connector instance lock acquired");

    // SQLite INTEGER is signed 64-bit; keep the random epoch positive and
    // representable so persistence cannot fail during the first snapshot.
    let boot_epoch = new_boot_epoch();
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
    mirror.begin_new_boot();
    mirror.save_snapshot(&journal)?;

    let runtimes = load_configured_runtimes().await?;
    let reconnect_delay_cap = configured_reconnect_delay_cap()?;
    let configured_runtime_ids = runtimes
        .iter()
        .map(|(config, _)| config.runtime_id.clone())
        .collect::<Vec<_>>();
    if mirror.retain_configured_runtimes(&configured_runtime_ids) {
        mirror.save_snapshot(&journal)?;
        info!("removed stale runtime projections absent from current configuration");
    }
    let pairing_db = std::env::var("MUXPORT_PAIRING_DB")
        .unwrap_or_else(|_| "muxport-pairing.db".into());
    let mut pairing_store = PairingStore::open_sqlite(&pairing_db)?;
    info!(path = %pairing_db, "persistent pairing store initialized");
    let host_identity =
        HostIdentityManager::new(OsHostIdentityStore::new()).load_or_create(&host_id);
    let direct_transport_security = match host_identity {
        Ok(identity) => {
            let binding = pairing_store.bind_host_identity(
                &host_id,
                &identity.signing_key().verifying_key(),
            );
            match binding {
                Ok(_) => {
                    info!(
                        created = identity.was_created(),
                        "OS-protected host identity is available"
                    );
                    pairing_store
                        .load_registry(&host_id, identity.signing_key())?;
                    Some((
                        Arc::new(identity.into_signing_key()),
                        pairing_store,
                    ))
                }
                Err(PairingStoreError::HostIdentityMismatch) => {
                    warn!(
                        "protected host identity does not match its persisted pin; authenticated pairing remains disabled pending explicit recovery"
                    );
                    None
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(error) => {
            warn!(
                %error,
                "host identity is locked; authenticated pairing remains disabled"
            );
            None
        }
    };
    let vault_file =
        std::env::var("MUXPORT_VAULT_FILE").unwrap_or_else(|_| "vault.sealed".into());
    let vault_key =
        VaultKeyManager::new(OsVaultKeyStore::new()).load_or_create(&host_id);
    let credential_vault = match vault_key {
        Ok(vault_key) => {
            let key_created = vault_key.was_created();
            match PersistentVault::open_or_create(
                &vault_file,
                &host_id,
                vault_key.into_key_encryption_key(),
            ) {
                Ok(vault) => {
                    info!(
                        path = %vault_file,
                        key_created,
                        "OS-protected credential vault is available"
                    );
                    Some(Arc::new(tokio::sync::Mutex::new(vault)))
                }
                Err(error) => {
                    warn!(
                        %error,
                        path = %vault_file,
                        "credential vault is locked or invalid; credential operations remain disabled"
                    );
                    None
                }
            }
        }
        Err(error) => {
            warn!(
                %error,
                "credential vault key is locked; credential operations remain disabled"
            );
            None
        }
    };
    let vault_available = credential_vault.is_some();
    if vault_available {
        mirror.transition_connector_state(ConnectorState::Recovering)?;
    } else {
        mirror.transition_connector_state(ConnectorState::VaultLocked)?;
        warn!(
            "connector entered vault_locked; non-credential recovery may continue but credential mutations are disabled"
        );
    }
    mirror.save_snapshot(&journal)?;
    let command_db = std::env::var("MUXPORT_COMMAND_DB")
        .unwrap_or_else(|_| "muxport-commands.db".into());
    let adapter_registry = runtimes
        .iter()
        .map(|(config, adapter)| {
            (config.runtime_id.clone(), Arc::clone(adapter))
        })
        .collect();
    let command_router = match credential_vault.as_ref() {
        Some(vault) => Arc::new(CommandRouter::open_sqlite_with_vault(
            &command_db,
            adapter_registry,
            Arc::clone(vault),
        )?),
        None => Arc::new(CommandRouter::open_sqlite(
            &command_db,
            adapter_registry,
        )?),
    };
    info!(path = %command_db, "persistent command ledger initialized");
    if let Some(vault) = credential_vault.as_ref() {
        let vault = vault.lock().await;
        mirror.replace_credential_profiles(vault_profile_projection(&vault));
        mirror.save_snapshot(&journal)?;
    }

    let (updates_tx, mut updates_rx) = mpsc::channel(SOURCE_UPDATE_CAPACITY);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let direct_transport_task =
        match (
            nonempty_env("MUXPORT_DIRECT_BIND"),
            direct_transport_security,
        ) {
            (Some(bind_address), Some((host_identity, pairing_store))) => {
                let allow_remote =
                    std::env::var("MUXPORT_ALLOW_REMOTE_DIRECT").as_deref()
                        == Ok("1");
                let bind_address =
                    validate_direct_bind(&bind_address, allow_remote)?;
                let listener = TcpListener::bind(bind_address).await?;
                let local_address = listener.local_addr()?;
                let advertised_endpoint =
                    nonempty_env("MUXPORT_PAIRING_ENDPOINT")
                        .unwrap_or_else(|| local_address.to_string());
                let pairing = Arc::new(Mutex::new(
                    PairingCoordinator::new(
                        pairing_store,
                        Arc::clone(&host_identity),
                        host_id.clone(),
                        hostname.clone(),
                        advertised_endpoint,
                    )?,
                ));
                if let Some(ttl) = pairing_offer_ttl()? {
                    let offer = pairing
                        .lock()
                        .map_err(|_| "pairing coordinator lock is unavailable")?
                        .create_offer(ttl)?;
                    info!(
                        pairing_offer_json = %serde_json::to_string(&offer)?,
                        expires_at_ms = offer.expires_at_ms,
                        "signed pairing offer created; keep this one-time code local until it is scanned"
                    );
                }
                let service =
                    DirectTransportService::new_with_pairing(
                        host_id.clone(),
                        boot_epoch,
                        host_identity,
                        pairing,
                        Arc::clone(&command_router),
                    )?
                    .with_event_journal(&state_db);
                info!(
                    bind_address = %local_address,
                    "authenticated direct command transport listening"
                );
                if vault_available {
                    mirror.transition_connector_state(ConnectorState::Ready)?;
                }
                mirror.save_snapshot(&journal)?;
                Some(tokio::spawn(service.serve(
                    listener,
                    shutdown_rx.clone(),
                )))
            }
            (Some(bind_address), None) => {
                warn!(
                    %bind_address,
                    "direct transport requested but the protected host identity is unavailable"
                );
                None
            }
            (None, _) => {
                warn!(
                    "direct transport is disabled; set MUXPORT_DIRECT_BIND to an explicit listen address"
                );
                None
            }
        };
    let mut monitor_tasks = Vec::new();
    for (config, adapter) in &runtimes {
        monitor_tasks.push(tokio::spawn(monitor_runtime(
            Arc::clone(adapter),
            config.clone(),
            updates_tx.clone(),
            shutdown_rx.clone(),
            reconnect_delay_cap,
        )));
    }
    drop(updates_tx);

    if direct_transport_task.is_some() {
        info!(
            "authenticated snapshot replay and live event polling are available"
        );
    } else {
        if vault_available {
            mirror.transition_connector_state(ConnectorState::Degraded)?;
        }
        mirror.save_snapshot(&journal)?;
        warn!(
            "mobile sync transport is unavailable; connector remains degraded"
        );
    }

    let mut shutdown = Box::pin(shutdown_signal());
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
                if let Some(vault) = credential_vault.as_ref() {
                    let vault = vault.lock().await;
                    mirror.replace_credential_profiles(
                        vault_profile_projection(&vault),
                    );
                }
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
    for (config, _) in &runtimes {
        if let Some(managed) = config.managed_opencode.as_ref() {
            if let Err(error) = managed.lock().await.stop().await {
                warn!(
                    %error,
                    runtime_id = %config.runtime_id,
                    "managed OpenCode process could not be stopped"
                );
            }
        }
    }
    if let Some(task) = direct_transport_task {
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                warn!(%error, "direct transport stopped with an error");
            }
            Err(error) => {
                warn!(%error, "direct transport task failed");
            }
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
            persisted,
        } => {
            let result: Result<(), event_journal::JournalError> = (|| {
                mirror.reconcile_runtime(
                    &config.runtime_id,
                    config.agent_type,
                    &config.runtime_name,
                    projects,
                    sessions,
                );
                mirror.set_runtime_active_profile(
                    &config.runtime_id,
                    &config.credential_profile_id,
                );
                mirror.save_snapshot(journal)?;
                info!(
                    runtime_id = %config.runtime_id,
                    runtime = %config.runtime_name,
                    "runtime mirror synchronized and durably baselined"
                );
                Ok(())
            })();
            let acknowledgement = result
                .as_ref()
                .map(|_| ())
                .map_err(ToString::to_string);
            let _ = persisted.send(acknowledgement);
            result?;
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
                runtime = %config.runtime_name,
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
    reconnect_delay_cap: Duration,
) {
    let mut reconnect_delay = INITIAL_RECONNECT_DELAY;
    let mut degraded_reported = false;
    loop {
        if *shutdown.borrow() || updates.is_closed() {
            return;
        }

        let synchronized = async {
            if let Some(managed) = config.managed_opencode.as_ref() {
                if managed.lock().await.ensure_running()? {
                    info!(
                        runtime_id = %config.runtime_id,
                        profile_id = %config.credential_profile_id,
                        "managed OpenCode process restarted in the same isolated profile"
                    );
                }
            }
            synchronize_runtime(adapter.as_ref(), &config, &updates).await
        }
        .await;
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
                    runtime = %config.runtime_name,
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
                if wait_for_shutdown(&mut shutdown, jittered_reconnect_delay(reconnect_delay)).await {
                    return;
                }
                reconnect_delay = next_reconnect_delay(reconnect_delay, reconnect_delay_cap);
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
        if wait_for_shutdown(&mut shutdown, jittered_reconnect_delay(reconnect_delay)).await {
            return;
        }
        reconnect_delay = next_reconnect_delay(reconnect_delay, reconnect_delay_cap);
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
    let (persisted, acknowledgement) = oneshot::channel();
    updates
        .send(SourceUpdate::Snapshot {
            config: config.clone(),
            projects,
            sessions,
            persisted,
        })
        .await
        .map_err(|_| AdapterError::Internal("connector update receiver closed".into()))?;
    acknowledgement
        .await
        .map_err(|_| {
            AdapterError::Internal(
                "connector closed before persisting runtime baseline".into(),
            )
        })?
        .map_err(|error| {
            AdapterError::Internal(format!(
                "connector could not persist runtime baseline: {error}"
            ))
        })
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
            active_profile_id: config.credential_profile_id.clone(),
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

async fn shutdown_signal() -> Result<(), io::Error> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

fn next_reconnect_delay(delay: Duration, cap: Duration) -> Duration {
    delay
        .checked_mul(2)
        .unwrap_or(cap)
        .min(cap)
}

fn jittered_reconnect_delay(delay: Duration) -> Duration {
    let percent = rand::thread_rng().gen_range(75_u32..=125);
    reconnect_delay_with_jitter_percent(delay, percent)
}

fn reconnect_delay_with_jitter_percent(delay: Duration, percent: u32) -> Duration {
    delay
        .checked_mul(percent)
        .and_then(|scaled| scaled.checked_div(100))
        .unwrap_or(delay)
}

fn configured_reconnect_delay_cap() -> Result<Duration, io::Error> {
    let Some(raw) = nonempty_env("MUXPORT_RECONNECT_MAX_MS") else {
        return Ok(MAX_RECONNECT_DELAY);
    };
    let milliseconds = raw.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "MUXPORT_RECONNECT_MAX_MS must be an integer number of milliseconds",
        )
    })?;
    let cap = Duration::from_millis(milliseconds);
    if !(INITIAL_RECONNECT_DELAY..=MAX_CONFIGURED_RECONNECT_DELAY).contains(&cap) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "MUXPORT_RECONNECT_MAX_MS must be between {} and {}",
                INITIAL_RECONNECT_DELAY.as_millis(),
                MAX_CONFIGURED_RECONNECT_DELAY.as_millis()
            ),
        ));
    }
    Ok(cap)
}

async fn load_configured_runtimes(
) -> Result<Vec<(RuntimeConfig, Arc<dyn AgentAdapter>)>, DynError> {
    let Some(manifest_path) = nonempty_env("MUXPORT_RUNTIME_MANIFEST") else {
        return load_legacy_runtimes();
    };
    let manifest = read_runtime_manifest(Path::new(&manifest_path))?;
    build_manifest_runtimes(manifest).await
}

fn read_runtime_manifest(path: &Path) -> Result<RuntimeManifest, DynError> {
    if !path.is_absolute() {
        return Err(invalid_manifest(
            "MUXPORT_RUNTIME_MANIFEST must be an absolute path",
        )
        .into());
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid_manifest("runtime manifest must not be a symbolic link").into());
    }
    if !metadata.is_file() {
        return Err(invalid_manifest("runtime manifest must be a regular file").into());
    }
    if metadata.len() > MAX_RUNTIME_MANIFEST_BYTES {
        return Err(invalid_manifest("runtime manifest exceeds the 1 MiB limit").into());
    }
    let bytes = fs::read(path)?;
    parse_runtime_manifest(&bytes).map_err(Into::into)
}

fn parse_runtime_manifest(bytes: &[u8]) -> Result<RuntimeManifest, io::Error> {
    let manifest: RuntimeManifest = serde_json::from_slice(bytes).map_err(|error| {
        invalid_manifest(format!("runtime manifest JSON is invalid: {error}"))
    })?;
    if manifest.version != RUNTIME_MANIFEST_VERSION {
        return Err(invalid_manifest(format!(
            "unsupported runtime manifest version {}; expected {}",
            manifest.version, RUNTIME_MANIFEST_VERSION
        )));
    }
    if !manifest.profiles_root.is_absolute() {
        return Err(invalid_manifest("profiles_root must be an absolute path"));
    }
    if let Ok(metadata) = fs::symlink_metadata(&manifest.profiles_root) {
        if metadata.file_type().is_symlink() {
            return Err(invalid_manifest(
                "profiles_root must not be a symbolic link",
            ));
        }
        if !metadata.is_dir() {
            return Err(invalid_manifest("profiles_root must be a directory"));
        }
    }
    if manifest.runtimes.is_empty() {
        return Err(invalid_manifest(
            "runtime manifest must configure at least one runtime",
        ));
    }
    if manifest.runtimes.len() > MAX_MANAGED_RUNTIMES {
        return Err(invalid_manifest(format!(
            "runtime manifest may configure at most {MAX_MANAGED_RUNTIMES} runtimes"
        )));
    }

    let mut runtime_ids = HashSet::new();
    let mut profiles = HashSet::new();
    let mut opencode_ports = HashSet::new();
    for entry in &manifest.runtimes {
        if !valid_manifest_identifier(&entry.runtime_id) {
            return Err(invalid_manifest(format!(
                "runtime_id {:?} is invalid",
                entry.runtime_id
            )));
        }
        if !valid_manifest_identifier(&entry.profile_id) {
            return Err(invalid_manifest(format!(
                "profile_id for runtime {:?} is invalid",
                entry.runtime_id
            )));
        }
        if !runtime_ids.insert(entry.runtime_id.clone()) {
            return Err(invalid_manifest(format!(
                "duplicate runtime_id {:?}",
                entry.runtime_id
            )));
        }
        if !profiles.insert((entry.agent_type, entry.profile_id.clone())) {
            return Err(invalid_manifest(format!(
                "profile {:?} is assigned to more than one {:?} runtime",
                entry.profile_id, entry.agent_type
            )));
        }
        if !entry.executable.is_absolute() {
            return Err(invalid_manifest(format!(
                "executable for runtime {:?} must be an absolute path",
                entry.runtime_id
            )));
        }
        if !entry.project_directory.is_absolute() {
            return Err(invalid_manifest(format!(
                "project_directory for runtime {:?} must be an absolute path",
                entry.runtime_id
            )));
        }
        if let Some(display_name) = &entry.display_name {
            let display_name = display_name.trim();
            if display_name.is_empty()
                || display_name.len() > 128
                || display_name.chars().any(char::is_control)
            {
                return Err(invalid_manifest(format!(
                    "display_name for runtime {:?} is invalid",
                    entry.runtime_id
                )));
            }
        }
        match entry.agent_type {
            ManifestAgentType::Opencode => {
                let port = entry.port.ok_or_else(|| {
                    invalid_manifest(format!(
                        "OpenCode runtime {:?} requires a port",
                        entry.runtime_id
                    ))
                })?;
                if port == 0 {
                    return Err(invalid_manifest(format!(
                        "OpenCode runtime {:?} port must be from 1 to 65535",
                        entry.runtime_id
                    )));
                }
                if !opencode_ports.insert(port) {
                    return Err(invalid_manifest(format!(
                        "OpenCode port {port} is assigned more than once"
                    )));
                }
            }
            ManifestAgentType::Codex if entry.port.is_some() => {
                return Err(invalid_manifest(format!(
                    "Codex runtime {:?} must not configure a port",
                    entry.runtime_id
                )));
            }
            ManifestAgentType::Codex => {}
        }
    }
    Ok(manifest)
}

async fn build_manifest_runtimes(
    manifest: RuntimeManifest,
) -> Result<Vec<(RuntimeConfig, Arc<dyn AgentAdapter>)>, DynError> {
    validate_runtime_paths(&manifest)?;
    let mut runtimes = Vec::with_capacity(manifest.runtimes.len());
    for entry in manifest.runtimes {
        let runtime_name = entry
            .display_name
            .as_deref()
            .map(str::trim)
            .unwrap_or_else(|| entry.agent_type.default_name())
            .to_owned();
        let built: Result<(RuntimeConfig, Arc<dyn AgentAdapter>), DynError> =
            (|| match entry.agent_type {
                ManifestAgentType::Opencode => {
                    let profile = ManagedOpenCodeProfile::prepare(
                        &manifest.profiles_root,
                        &entry.profile_id,
                        &entry.executable,
                        &entry.project_directory,
                        entry.port.expect("validated OpenCode port"),
                    )?;
                    let password = uuid::Uuid::new_v4().simple().to_string();
                    let adapter = profile.adapter(password.clone())?;
                    let managed = ManagedOpenCodeChild::start(profile.clone(), password)?;
                    info!(
                        runtime_id = %entry.runtime_id,
                        profile_id = %entry.profile_id,
                        profile_root = %profile.profile_root().display(),
                        endpoint = %profile.base_url(),
                        "connector-managed isolated OpenCode runtime started from manifest"
                    );
                    Ok((
                        RuntimeConfig {
                            runtime_id: entry.runtime_id,
                            agent_type: entry.agent_type.protocol_type(),
                            runtime_name,
                            credential_profile_id: entry.profile_id,
                            managed_opencode: Some(Arc::new(tokio::sync::Mutex::new(managed))),
                        },
                        Arc::new(adapter) as Arc<dyn AgentAdapter>,
                    ))
                }
                ManifestAgentType::Codex => {
                    let profile = ManagedCodexProfile::prepare(
                        &manifest.profiles_root,
                        &entry.profile_id,
                        &entry.executable,
                        &entry.project_directory,
                    )?;
                    info!(
                        runtime_id = %entry.runtime_id,
                        profile_id = %entry.profile_id,
                        profile_root = %profile.profile_root().display(),
                        "connector-managed isolated Codex App Server configured from manifest"
                    );
                    Ok((
                        RuntimeConfig {
                            runtime_id: entry.runtime_id,
                            agent_type: entry.agent_type.protocol_type(),
                            runtime_name,
                            credential_profile_id: entry.profile_id,
                            managed_opencode: None,
                        },
                        Arc::new(profile.adapter()) as Arc<dyn AgentAdapter>,
                    ))
                }
            })();
        match built {
            Ok(runtime) => runtimes.push(runtime),
            Err(error) => {
                stop_managed_opencode_children(&runtimes).await;
                return Err(error);
            }
        }
    }
    Ok(runtimes)
}

fn validate_runtime_paths(manifest: &RuntimeManifest) -> Result<(), io::Error> {
    for entry in &manifest.runtimes {
        let executable = fs::metadata(&entry.executable).map_err(|error| {
            invalid_manifest(format!(
                "executable for runtime {:?} is unavailable: {error}",
                entry.runtime_id
            ))
        })?;
        if !executable.is_file() {
            return Err(invalid_manifest(format!(
                "executable for runtime {:?} must be a file",
                entry.runtime_id
            )));
        }
        let project = fs::metadata(&entry.project_directory).map_err(|error| {
            invalid_manifest(format!(
                "project_directory for runtime {:?} is unavailable: {error}",
                entry.runtime_id
            ))
        })?;
        if !project.is_dir() {
            return Err(invalid_manifest(format!(
                "project_directory for runtime {:?} must be a directory",
                entry.runtime_id
            )));
        }
    }
    Ok(())
}

fn load_legacy_runtimes() -> Result<Vec<(RuntimeConfig, Arc<dyn AgentAdapter>)>, DynError> {
    let managed_opencode_profile_id = nonempty_env("MUXPORT_OPENCODE_PROFILE_ID");
    let (opencode, default_opencode_runtime_id, managed_opencode): (
        Arc<dyn AgentAdapter>,
        String,
        Option<Arc<tokio::sync::Mutex<ManagedOpenCodeChild>>>,
    ) = if let Some(profile_id) = managed_opencode_profile_id.as_deref() {
        let profile = managed_opencode_profile(profile_id)?;
        let password = nonempty_env("MUXPORT_OPENCODE_PASSWORD")
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        let adapter = profile.adapter(password.clone())?;
        let managed = ManagedOpenCodeChild::start(profile.clone(), password)?;
        info!(
            profile_id,
            profile_root = %profile.profile_root().display(),
            endpoint = %profile.base_url(),
            "connector-managed isolated OpenCode runtime started"
        );
        (
            Arc::new(adapter),
            format!("opencode-managed-{profile_id}"),
            Some(Arc::new(tokio::sync::Mutex::new(managed))),
        )
    } else {
        let opencode_url = std::env::var("MUXPORT_OPENCODE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:4096".into());
        let opencode_password = nonempty_env("MUXPORT_OPENCODE_PASSWORD");
        (
            Arc::new(OpenCodeAdapter::new(opencode_url, opencode_password)),
            "opencode-local".into(),
            None,
        )
    };
    let opencode_config = RuntimeConfig {
        runtime_id: nonempty_env("MUXPORT_OPENCODE_RUNTIME_ID")
            .unwrap_or(default_opencode_runtime_id),
        agent_type: AgentType::Opencode,
        runtime_name: "OpenCode".into(),
        credential_profile_id: managed_opencode_profile_id.unwrap_or_default(),
        managed_opencode,
    };

    let managed_codex_profile_id = nonempty_env("MUXPORT_CODEX_PROFILE_ID");
    let (codex, default_codex_runtime_id): (Arc<dyn AgentAdapter>, String) =
        if let Some(profile_id) = managed_codex_profile_id.as_deref() {
            let profile = managed_codex_profile(profile_id)?;
            info!(
                profile_id,
                profile_root = %profile.profile_root().display(),
                "connector-managed isolated Codex App Server configured"
            );
            (Arc::new(profile.adapter()), format!("codex-managed-{profile_id}"))
        } else {
            let codex_path =
                nonempty_env("MUXPORT_CODEX_PATH").unwrap_or_else(|| "codex".into());
            (Arc::new(CodexAdapter::new(codex_path)), "codex-local".into())
        };
    let codex_config = RuntimeConfig {
        runtime_id: nonempty_env("MUXPORT_CODEX_RUNTIME_ID")
            .unwrap_or(default_codex_runtime_id),
        agent_type: AgentType::Codex,
        runtime_name: "Codex".into(),
        credential_profile_id: managed_codex_profile_id.unwrap_or_default(),
        managed_opencode: None,
    };
    Ok(vec![(opencode_config, opencode), (codex_config, codex)])
}

async fn stop_managed_opencode_children(
    runtimes: &[(RuntimeConfig, Arc<dyn AgentAdapter>)],
) {
    for (config, _) in runtimes {
        if let Some(managed) = config.managed_opencode.as_ref() {
            let _ = managed.lock().await.stop().await;
        }
    }
}

fn valid_manifest_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn invalid_manifest(message: impl Into<String>) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("invalid runtime manifest: {}", message.into()),
    )
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn pairing_offer_ttl() -> Result<Option<Duration>, io::Error> {
    let Some(value) = nonempty_env("MUXPORT_PAIRING_OFFER_TTL_SECONDS")
    else {
        return Ok(None);
    };
    let seconds = value.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "MUXPORT_PAIRING_OFFER_TTL_SECONDS must be an integer",
        )
    })?;
    if !(30..=600).contains(&seconds) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MUXPORT_PAIRING_OFFER_TTL_SECONDS must be between 30 and 600",
        ));
    }
    Ok(Some(Duration::from_secs(seconds)))
}

async fn run_local_subcommand() -> Result<bool, DynError> {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Ok(false);
    };
    match command.as_str() {
        "pairing-confirm" => run_pairing_confirm(arguments)?,
        "opencode-profile-auth" => {
            let profile_id = arguments
                .next()
                .ok_or("opencode-profile-auth requires PROFILE_ID PROVIDER_ID")?;
            let provider_id = arguments
                .next()
                .ok_or("opencode-profile-auth requires PROFILE_ID PROVIDER_ID")?;
            if arguments.next().is_some() {
                return Err(
                    "opencode-profile-auth accepts exactly PROFILE_ID PROVIDER_ID".into(),
                );
            }
            let profile = managed_opencode_profile(&profile_id)?;
            let status = profile.auth_login(&provider_id).await?;
            if !status.success() {
                return Err(format!(
                    "OpenCode authentication exited unsuccessfully with {status}"
                )
                .into());
            }
            println!(
                "OpenCode provider {provider_id} enrolled in isolated profile {profile_id}"
            );
        }
        "codex-profile-login" => {
            let profile_id = arguments
                .next()
                .ok_or("codex-profile-login requires PROFILE_ID")?;
            if arguments.next().is_some() {
                return Err("codex-profile-login accepts exactly PROFILE_ID".into());
            }
            let profile = managed_codex_profile(&profile_id)?;
            let adapter = profile.adapter();
            let login = adapter.start_device_code_login().await?;
            let (login_id, verification_url, user_code) = match login {
                CodexLoginStart::ChatgptDeviceCode {
                    login_id,
                    verification_url,
                    user_code,
                } => (login_id, verification_url, user_code),
                other => {
                    return Err(format!(
                        "Codex returned an unexpected device login response: {other:?}"
                    )
                    .into())
                }
            };
            println!("Open {verification_url}");
            println!("Enter code: {user_code}");
            println!("Waiting for Codex to confirm the isolated profile login...");
            adapter
                .wait_for_login_completion(&login_id, Duration::from_secs(15 * 60))
                .await?;
            let account = adapter.read_account(true).await?;
            if account.account.is_none() {
                return Err(
                    "Codex reported login completion but account/read remained signed out".into(),
                );
            }
            adapter.shutdown_gracefully().await?;
            println!("Codex account enrolled in isolated profile {profile_id}");
        }
        "runtime-manifest-validate" => {
            let path = arguments
                .next()
                .ok_or("runtime-manifest-validate requires ABSOLUTE_MANIFEST_PATH")?;
            if arguments.next().is_some() {
                return Err(
                    "runtime-manifest-validate accepts exactly ABSOLUTE_MANIFEST_PATH".into(),
                );
            }
            let manifest = read_runtime_manifest(Path::new(&path))?;
            validate_runtime_paths(&manifest)?;
            println!(
                "Runtime manifest version {} is valid for {} managed runtime(s)",
                manifest.version,
                manifest.runtimes.len()
            );
        }
        _ => return Err(format!("unknown connector command {command:?}").into()),
    }
    Ok(true)
}

fn run_pairing_confirm(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(), DynError> {
    let host_id = arguments
        .next()
        .ok_or("pairing-confirm requires HOST_ID PAIRING_ID SAS")?;
    let pairing_id = arguments
        .next()
        .ok_or("pairing-confirm requires HOST_ID PAIRING_ID SAS")?;
    let sas = arguments
        .next()
        .ok_or("pairing-confirm requires HOST_ID PAIRING_ID SAS")?;
    if arguments.next().is_some() {
        return Err(
            "pairing-confirm accepts exactly HOST_ID PAIRING_ID SAS".into(),
        );
    }
    let identity = HostIdentityManager::new(OsHostIdentityStore::new())
        .load_existing(&host_id)?
        .ok_or("no existing protected identity exists for that host id")?;
    let pairing_db = std::env::var("MUXPORT_PAIRING_DB")
        .unwrap_or_else(|_| "muxport-pairing.db".into());
    let mut store = PairingStore::open_sqlite(&pairing_db)?;
    store.bind_host_identity(
        &host_id,
        &identity.signing_key().verifying_key(),
    )?;
    match store.confirm_sas(
        &pairing_id,
        &sas,
        ConfirmingParty::Host,
    ) {
        Ok(true) => {
            let device = store.finalize(
                &pairing_id,
                &host_id,
                identity.signing_key(),
            )?;
            println!(
                "pairing finalized for device {} ({})",
                device.device_id, device.device_name
            );
        }
        Ok(false) => {
            println!(
                "host SAS confirmed; waiting for phone confirmation for {pairing_id}"
            );
        }
        Err(PairingStoreError::InvalidPairingState) => {
            let device = store.finalize(
                &pairing_id,
                &host_id,
                identity.signing_key(),
            )?;
            println!(
                "pairing was already finalized for device {} ({})",
                device.device_id, device.device_name
            );
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn managed_opencode_profile(
    profile_id: &str,
) -> Result<ManagedOpenCodeProfile, DynError> {
    let executable = nonempty_env("MUXPORT_OPENCODE_PATH")
        .map(PathBuf::from)
        .ok_or("MUXPORT_OPENCODE_PATH is required for a managed OpenCode profile")?;
    let project_directory = nonempty_env("MUXPORT_OPENCODE_PROJECT")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let profiles_root = nonempty_env("MUXPORT_PROFILES_DIR")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?.join("profiles"));
    let port = match nonempty_env("MUXPORT_OPENCODE_PORT") {
        Some(value) => value.parse::<u16>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "MUXPORT_OPENCODE_PORT must be an integer from 1 to 65535",
            )
        })?,
        None => 4096,
    };
    if port == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MUXPORT_OPENCODE_PORT must be an integer from 1 to 65535",
        )
        .into());
    }
    Ok(ManagedOpenCodeProfile::prepare(
        profiles_root,
        profile_id,
        executable,
        project_directory,
        port,
    )?)
}

fn managed_codex_profile(profile_id: &str) -> Result<ManagedCodexProfile, DynError> {
    let executable = nonempty_env("MUXPORT_CODEX_PATH")
        .map(PathBuf::from)
        .ok_or("MUXPORT_CODEX_PATH is required for a managed Codex profile")?;
    let project_directory = nonempty_env("MUXPORT_CODEX_PROJECT")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let profiles_root = nonempty_env("MUXPORT_PROFILES_DIR")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?.join("profiles"));
    Ok(ManagedCodexProfile::prepare(
        profiles_root,
        profile_id,
        executable,
        project_directory,
    )?)
}

fn vault_profile_projection(vault: &PersistentVault) -> Vec<CredentialProfileInfo> {
    vault
        .list_profiles()
        .into_iter()
        .map(|profile| CredentialProfileInfo {
            profile_id: profile.profile_id,
            display_name: profile.display_name,
            provider: profile.provider,
            account_fingerprint: profile.account_fingerprint,
            status: profile.status as i32,
            last_validated_at_ms: profile.last_validated_at_ms,
        })
        .collect()
}

fn new_boot_epoch() -> u64 {
    (uuid::Uuid::new_v4().as_u128() & i64::MAX as u128) as u64
}

fn validate_direct_bind(
    value: &str,
    allow_remote: bool,
) -> Result<SocketAddr, io::Error> {
    let address: SocketAddr = value.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "MUXPORT_DIRECT_BIND must be an IP socket address",
        )
    })?;
    if !address.ip().is_loopback() && !allow_remote {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "non-loopback direct transport requires MUXPORT_ALLOW_REMOTE_DIRECT=1",
        ));
    }
    Ok(address)
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
    fn direct_transport_defaults_to_loopback_only() {
        assert!(validate_direct_bind("127.0.0.1:45821", false).is_ok());
        assert!(validate_direct_bind("[::1]:45821", false).is_ok());
        assert_eq!(
            validate_direct_bind("0.0.0.0:45821", false)
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn remote_direct_transport_requires_explicit_opt_in() {
        assert_eq!(
            validate_direct_bind("192.0.2.10:45821", true).unwrap(),
            "192.0.2.10:45821".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            validate_direct_bind("localhost:45821", true)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn reconnect_delay_is_bounded() {
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(1), MAX_RECONNECT_DELAY),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(30), MAX_RECONNECT_DELAY),
            MAX_RECONNECT_DELAY
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(20), Duration::from_secs(45)),
            Duration::from_secs(40)
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(40), Duration::from_secs(45)),
            Duration::from_secs(45)
        );
        assert_eq!(
            reconnect_delay_with_jitter_percent(Duration::from_secs(20), 75),
            Duration::from_secs(15)
        );
        assert_eq!(
            reconnect_delay_with_jitter_percent(Duration::from_secs(20), 125),
            Duration::from_secs(25)
        );
    }

    fn test_runtime_manifest() -> serde_json::Value {
        let root = std::env::temp_dir().join("muxport-manifest-test");
        serde_json::json!({
            "version": 1,
            "profiles_root": root.join("profiles"),
            "runtimes": [
                {
                    "runtime_id": "opencode-work",
                    "agent_type": "opencode",
                    "profile_id": "work",
                    "executable": root.join("opencode"),
                    "project_directory": root.join("work-project"),
                    "port": 43101,
                    "display_name": "Work OpenCode"
                },
                {
                    "runtime_id": "codex-personal",
                    "agent_type": "codex",
                    "profile_id": "personal",
                    "executable": root.join("codex"),
                    "project_directory": root.join("personal-project")
                }
            ]
        })
    }

    #[test]
    fn runtime_manifest_accepts_multiple_isolated_agent_instances() {
        let bytes = serde_json::to_vec(&test_runtime_manifest()).unwrap();
        let manifest = parse_runtime_manifest(&bytes).unwrap();
        assert_eq!(manifest.runtimes.len(), 2);
        assert_eq!(
            manifest.runtimes[0].agent_type,
            ManifestAgentType::Opencode
        );
        assert_eq!(manifest.runtimes[1].agent_type, ManifestAgentType::Codex);
    }

    #[test]
    fn runtime_manifest_rejects_runtime_profile_and_port_collisions() {
        for (field, value, expected) in [
            ("runtime_id", serde_json::json!("opencode-work"), "runtime_id"),
            ("profile_id", serde_json::json!("work"), "profile"),
        ] {
            let mut document = test_runtime_manifest();
            document["runtimes"][1]["agent_type"] = serde_json::json!("opencode");
            document["runtimes"][1]["port"] = serde_json::json!(43102);
            document["runtimes"][1][field] = value;
            let error =
                parse_runtime_manifest(&serde_json::to_vec(&document).unwrap()).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }

        let mut document = test_runtime_manifest();
        document["runtimes"][1]["agent_type"] = serde_json::json!("opencode");
        document["runtimes"][1]["port"] = serde_json::json!(43101);
        let error = parse_runtime_manifest(&serde_json::to_vec(&document).unwrap()).unwrap_err();
        assert!(error.to_string().contains("port 43101"), "{error}");
    }

    #[test]
    fn runtime_manifest_rejects_secrets_and_agent_specific_mistakes() {
        let mut document = test_runtime_manifest();
        document["runtimes"][0]["password"] = serde_json::json!("must-not-be-here");
        let error = parse_runtime_manifest(&serde_json::to_vec(&document).unwrap()).unwrap_err();
        assert!(error.to_string().contains("unknown field"), "{error}");

        let mut document = test_runtime_manifest();
        document["runtimes"][1]["port"] = serde_json::json!(43102);
        let error = parse_runtime_manifest(&serde_json::to_vec(&document).unwrap()).unwrap_err();
        assert!(error.to_string().contains("must not configure a port"), "{error}");
    }

    #[test]
    fn runtime_manifest_is_versioned_and_bounded() {
        let mut document = test_runtime_manifest();
        document["version"] = serde_json::json!(2);
        let error = parse_runtime_manifest(&serde_json::to_vec(&document).unwrap()).unwrap_err();
        assert!(error.to_string().contains("version 2"), "{error}");

        let mut document = test_runtime_manifest();
        document["runtimes"] = serde_json::json!([]);
        let error = parse_runtime_manifest(&serde_json::to_vec(&document).unwrap()).unwrap_err();
        assert!(error.to_string().contains("at least one runtime"), "{error}");
    }

    #[test]
    fn runtime_manifest_preflights_paths_before_process_start() {
        let mut document = test_runtime_manifest();
        document["runtimes"][0]["executable"] = serde_json::json!(
            std::env::temp_dir().join(format!(
                "muxport-missing-executable-{}",
                uuid::Uuid::new_v4()
            ))
        );
        let bytes = serde_json::to_vec(&document).unwrap();
        let manifest = parse_runtime_manifest(&bytes).unwrap();
        let error = validate_runtime_paths(&manifest).unwrap_err();
        assert!(error.to_string().contains("executable"), "{error}");
        assert!(error.to_string().contains("unavailable"), "{error}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runtime_manifest_builds_and_stops_multiple_managed_instances() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "muxport-multi-runtime-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let executable = root.join("fake-agent");
        std::fs::write(&executable, "#!/bin/sh\nwhile :; do sleep 1; done\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let document = serde_json::json!({
            "version": 1,
            "profiles_root": root.join("profiles"),
            "runtimes": [
                {
                    "runtime_id": "opencode-a",
                    "agent_type": "opencode",
                    "profile_id": "account-a",
                    "executable": executable,
                    "project_directory": project,
                    "port": 43111
                },
                {
                    "runtime_id": "opencode-b",
                    "agent_type": "opencode",
                    "profile_id": "account-b",
                    "executable": executable,
                    "project_directory": project,
                    "port": 43112
                },
                {
                    "runtime_id": "codex-a",
                    "agent_type": "codex",
                    "profile_id": "account-a",
                    "executable": executable,
                    "project_directory": project
                },
                {
                    "runtime_id": "codex-b",
                    "agent_type": "codex",
                    "profile_id": "account-b",
                    "executable": executable,
                    "project_directory": project
                }
            ]
        });
        let manifest =
            parse_runtime_manifest(&serde_json::to_vec(&document).unwrap()).unwrap();
        let runtimes = build_manifest_runtimes(manifest).await.unwrap();
        assert_eq!(runtimes.len(), 4);
        assert_eq!(
            runtimes
                .iter()
                .filter(|(config, _)| config.managed_opencode.is_some())
                .count(),
            2
        );
        assert!(root.join("profiles/opencode/account-a").is_dir());
        assert!(root.join("profiles/opencode/account-b").is_dir());
        assert!(root.join("profiles/codex/account-a").is_dir());
        assert!(root.join("profiles/codex/account-b").is_dir());

        stop_managed_opencode_children(&runtimes).await;
        for (config, _) in &runtimes {
            if let Some(managed) = config.managed_opencode.as_ref() {
                assert!(managed.lock().await.child.is_none());
            }
        }
        drop(runtimes);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn synchronization_brackets_subscription_with_snapshots() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let config = RuntimeConfig {
            runtime_id: "codex-test".into(),
            agent_type: AgentType::Codex,
            runtime_name: "Codex".into(),
            credential_profile_id: String::new(),
            managed_opencode: None,
        };
        let (updates_tx, mut updates_rx) = mpsc::channel(4);
        let synchronization = tokio::spawn({
            let adapter = Arc::clone(&adapter);
            let config = config.clone();
            async move {
                synchronize_runtime(adapter.as_ref(), &config, &updates_tx).await
            }
        });

        for _ in 0..2 {
            match updates_rx.recv().await.unwrap() {
                SourceUpdate::Snapshot {
                    config,
                    projects,
                    sessions,
                    persisted,
                } => {
                    assert_eq!(config.runtime_id, "codex-test");
                    assert_eq!(projects.len(), 1);
                    assert_eq!(sessions.len(), 1);
                    persisted.send(Ok(())).unwrap();
                }
                _ => panic!("synchronization emitted a non-snapshot baseline"),
            }
        }
        let mut events = synchronization.await.unwrap().unwrap();
        assert!(events.next().await.is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn managed_opencode_child_restarts_with_the_same_profile() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "muxport-managed-child-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let executable = root.join("fake-opencode");
        std::fs::write(
            &executable,
            "#!/bin/sh\nprintf 'launch\\n' >> launches.txt\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let profile =
            ManagedOpenCodeProfile::prepare(&root, "profile-a", executable, &project, 43119)
                .unwrap();
        let mut managed =
            ManagedOpenCodeChild::start(profile, "test-server-password".into()).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(managed.ensure_running().unwrap());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            std::fs::read_to_string(project.join("launches.txt"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        for _ in 0..3 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert!(managed.ensure_running().unwrap());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(matches!(
            managed.ensure_running(),
            Err(AdapterError::InitFailed(detail)) if detail.contains("crash loop")
        ));
        assert_eq!(
            std::fs::read_to_string(project.join("launches.txt"))
                .unwrap()
                .lines()
                .count(),
            5
        );
        managed.stop().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
