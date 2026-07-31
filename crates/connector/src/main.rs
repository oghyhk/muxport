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
    HostIdentityManager, LoadedHostIdentity, OsHostIdentityStore, OsVaultKeyStore,
    KeyEncryptionKey, PersistentVault, VaultKeyManager,
};
use event_journal::EventJournal;
use futures::StreamExt;
use muxport_protocol::{
    event, AgentType, ConnectorState, CredentialProfileInfo, Event, RuntimeState,
    RuntimeStateEvent,
};
use serde::Deserialize;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use zeroize::Zeroize;

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
const MAX_MANAGED_STDERR_TAIL_BYTES: usize = 8 * 1024;
const MAX_HEADLESS_VAULT_PASSPHRASE_BYTES: u64 = 4096;

type DynError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct RuntimeConfig {
    runtime_id: String,
    agent_type: AgentType,
    runtime_name: String,
    credential_profile_id: Arc<RwLock<String>>,
    session_assignments: Arc<RwLock<HashMap<String, String>>>,
    managed_opencode: Option<Arc<tokio::sync::Mutex<ManagedOpenCodeChild>>>,
    connector_managed: bool,
}

impl RuntimeConfig {
    fn credential_profile_id(&self) -> String {
        self.credential_profile_id
            .read()
            .map(|profile| profile.clone())
            .unwrap_or_default()
    }

    fn apply_session_assignments(&self, sessions: &mut [SessionSummary]) {
        let Ok(assignments) = self.session_assignments.read() else {
            return;
        };
        for session in sessions {
            if let Some(profile_id) = assignments.get(&session.session_id) {
                session.credential_profile_id = profile_id.clone();
            }
        }
    }

    fn apply_event_session_assignment(&self, event: &mut Event) {
        let Some(event::Inner::SessionUpdated(session)) = event.inner.as_mut() else {
            return;
        };
        let Ok(assignments) = self.session_assignments.read() else {
            return;
        };
        if let Some(profile_id) = assignments.get(&session.session_id) {
            session.credential_profile_id = profile_id.clone();
        }
    }
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
    #[serde(default = "default_restart_on_failure")]
    restart_on_failure: bool,
}

const fn default_restart_on_failure() -> bool {
    true
}

struct ManagedOpenCodeChild {
    profile: ManagedOpenCodeProfile,
    server_password: String,
    child: Option<tokio::process::Child>,
    restart_attempts: VecDeque<Instant>,
    crash_loop_tripped: bool,
    restart_on_failure: bool,
    stderr_tail: Arc<Mutex<Vec<u8>>>,
    last_exit_diagnostic: Option<String>,
}

impl ManagedOpenCodeChild {
    fn start(
        profile: ManagedOpenCodeProfile,
        server_password: String,
        restart_on_failure: bool,
    ) -> Result<Self, adapter_opencode::ManagedOpenCodeError> {
        let (child, stderr_tail) = capture_managed_stderr(profile.spawn(&server_password)?);
        Ok(Self {
            profile,
            server_password,
            child: Some(child),
            restart_attempts: VecDeque::new(),
            crash_loop_tripped: false,
            restart_on_failure,
            stderr_tail,
            last_exit_diagnostic: None,
        })
    }

    fn ensure_running(&mut self) -> Result<bool, AdapterError> {
        if self.crash_loop_tripped {
            return Err(AdapterError::InitFailed(
                "managed OpenCode crash loop is latched; operator restart is required".into(),
            ));
        }
        let exited = match self.child.as_mut() {
            Some(child) => child.try_wait().map_err(|error| {
                AdapterError::InitFailed(format!(
                    "managed OpenCode process status failed: {error}"
                ))
            })?,
            None => None,
        };
        if self.child.is_some() && exited.is_none() {
            return Ok(false);
        }
        if let Some(status) = exited {
            let diagnostic = managed_exit_diagnostic(&self.profile, &status, &self.stderr_tail);
            warn!(detail = %diagnostic, "managed OpenCode exited; only redacted crash metadata was retained");
            self.last_exit_diagnostic = Some(diagnostic);
        }
        if !self.restart_on_failure {
            self.child = None;
            return Err(AdapterError::InitFailed(format!(
                "connector-managed OpenCode runtime exited and restart_on_failure is disabled ({})",
                self.last_exit_diagnostic.as_deref().unwrap_or("no exit metadata available")
            )));
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
                "managed OpenCode crash loop: {} failures within {} seconds; automatic restart stopped ({})",
                MAX_MANAGED_FAILURES_PER_WINDOW,
                MANAGED_RESTART_WINDOW.as_secs(),
                self.last_exit_diagnostic.as_deref().unwrap_or("no exit metadata available")
            )));
        }
        let (child, stderr_tail) = capture_managed_stderr(self.profile.spawn(&self.server_password).map_err(|error| {
            AdapterError::InitFailed(format!(
                "managed OpenCode process restart failed: {error}"
            ))
        })?);
        self.child = Some(child);
        self.stderr_tail = stderr_tail;
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

/// Captures a small private tail for crash attribution. Raw stderr is never
/// logged or exported: vendor processes may include secrets in arbitrary error
/// text, so diagnostics expose only its byte count and SHA-256 fingerprint.
fn capture_managed_stderr(
    mut child: tokio::process::Child,
) -> (tokio::process::Child, Arc<Mutex<Vec<u8>>>) {
    let tail = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut stderr) = child.stderr.take() {
        let destination = Arc::clone(&tail);
        tokio::spawn(async move {
            let mut chunk = [0_u8; 1024];
            loop {
                let Ok(read) = stderr.read(&mut chunk).await else {
                    break;
                };
                if read == 0 {
                    break;
                }
                let Ok(mut captured) = destination.lock() else {
                    break;
                };
                captured.extend_from_slice(&chunk[..read]);
                if captured.len() > MAX_MANAGED_STDERR_TAIL_BYTES {
                    let excess = captured.len() - MAX_MANAGED_STDERR_TAIL_BYTES;
                    captured.drain(..excess);
                }
            }
        });
    }
    (child, tail)
}

fn managed_exit_diagnostic(
    profile: &ManagedOpenCodeProfile,
    status: &std::process::ExitStatus,
    stderr_tail: &Arc<Mutex<Vec<u8>>>,
) -> String {
    let (stderr_bytes, stderr_sha256) = match stderr_tail.lock() {
        Ok(bytes) => (bytes.len(), format!("{:x}", Sha256::digest(&*bytes))),
        Err(_) => (0, "unavailable".into()),
    };
    let exit_code = status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".into());
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status
            .signal()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".into())
    };
    #[cfg(not(unix))]
    let signal = "unavailable".to_owned();
    format!(
        "profile_id={} exit_code={} signal={} stderr_tail_bytes={} stderr_tail_sha256={}",
        profile.profile_id(), exit_code, signal, stderr_bytes, stderr_sha256
    )
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
    let host_identity_manager = HostIdentityManager::new(OsHostIdentityStore::new());
    let host_identity = host_identity_manager.load_or_create(&host_id);
    let direct_transport_security = match host_identity {
        Ok(identity) => {
            if complete_promoted_host_key_rotation(
                &host_id,
                &host_identity_manager,
                &identity,
                &mut pairing_store,
            )? {
                info!("completed an interrupted host-key rotation; every phone must pair again");
            }
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
    let vault_key = load_connector_vault_key(&host_id);
    let credential_vault = match vault_key {
        Ok((vault_key, key_created, key_source)) => {
            match PersistentVault::open_or_create(
                &vault_file,
                &host_id,
                vault_key,
            ) {
                Ok(vault) => {
                    info!(
                        path = %vault_file,
                        key_created,
                        key_source,
                        "credential vault is available"
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
    let assignment_targets = runtimes
        .iter()
        .map(|(config, _)| {
            (
                config.runtime_id.clone(),
                Arc::clone(&config.credential_profile_id),
            )
        })
        .collect();
    let assignment_defaults = runtimes
        .iter()
        .map(|(config, _)| {
            (
                config.runtime_id.clone(),
                config.credential_profile_id(),
            )
        })
        .collect::<Vec<_>>();
    let session_assignment_targets = runtimes
        .iter()
        .map(|(config, _)| {
            (
                config.runtime_id.clone(),
                Arc::clone(&config.session_assignments),
            )
        })
        .collect();
    let command_router = match credential_vault.as_ref() {
        Some(vault) => CommandRouter::open_sqlite_with_vault(
            &command_db,
            adapter_registry,
            Arc::clone(vault),
        )?,
        None => CommandRouter::open_sqlite(&command_db, adapter_registry)?,
    }
    .with_host_id(host_id.clone())
    .with_assignment_targets(assignment_targets)
    .with_session_assignment_targets(session_assignment_targets);
    command_router
        .initialize_runtime_assignments(&assignment_defaults)
        .await?;
    let command_router = Arc::new(command_router);
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
            mut sessions,
            persisted,
        } => {
            let result: Result<(), event_journal::JournalError> = (|| {
                config.apply_session_assignments(&mut sessions);
                mirror.reconcile_runtime(
                    &config.runtime_id,
                    config.agent_type,
                    &config.runtime_name,
                    projects,
                    sessions,
                );
                mirror.set_runtime_connector_managed(
                    &config.runtime_id,
                    config.connector_managed,
                );
                mirror.set_runtime_active_profile(
                    &config.runtime_id,
                    &config.credential_profile_id(),
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
        SourceUpdate::Event { config, mut event } => {
            config.apply_event_session_assignment(&mut event);
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
                        profile_id = %config.credential_profile_id(),
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
            active_profile_id: config.credential_profile_id(),
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
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "headless vault passphrase file is unavailable",
        )
    })?;
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
                    let managed = ManagedOpenCodeChild::start(
                        profile.clone(),
                        password,
                        entry.restart_on_failure,
                    )?;
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
                            credential_profile_id: Arc::new(RwLock::new(entry.profile_id)),
                            session_assignments: Arc::new(RwLock::new(HashMap::new())),
                            managed_opencode: Some(Arc::new(tokio::sync::Mutex::new(managed))),
                            connector_managed: true,
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
                            credential_profile_id: Arc::new(RwLock::new(entry.profile_id)),
                            session_assignments: Arc::new(RwLock::new(HashMap::new())),
                            managed_opencode: None,
                            connector_managed: true,
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
    let connector_managed_opencode = managed_opencode_profile_id.is_some();
    let (opencode, default_opencode_runtime_id, managed_opencode): (
        Arc<dyn AgentAdapter>,
        String,
        Option<Arc<tokio::sync::Mutex<ManagedOpenCodeChild>>>,
    ) = if let Some(profile_id) = managed_opencode_profile_id.as_deref() {
        let profile = managed_opencode_profile(profile_id)?;
        let password = nonempty_env("MUXPORT_OPENCODE_PASSWORD")
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        let adapter = profile.adapter(password.clone())?;
        let managed = ManagedOpenCodeChild::start(profile.clone(), password, true)?;
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
        credential_profile_id: Arc::new(RwLock::new(
            managed_opencode_profile_id.unwrap_or_default(),
        )),
        session_assignments: Arc::new(RwLock::new(HashMap::new())),
        managed_opencode,
        connector_managed: connector_managed_opencode,
    };

    let managed_codex_profile_id = nonempty_env("MUXPORT_CODEX_PROFILE_ID");
    let connector_managed_codex = managed_codex_profile_id.is_some();
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
        credential_profile_id: Arc::new(RwLock::new(
            managed_codex_profile_id.unwrap_or_default(),
        )),
        session_assignments: Arc::new(RwLock::new(HashMap::new())),
        managed_opencode: None,
        connector_managed: connector_managed_codex,
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
        "pairing-list" => run_pairing_list(arguments)?,
        "pairing-revoke" => run_pairing_revoke(arguments)?,
        "pairing-rotate-host-key" => run_pairing_rotate_host_key(arguments)?,
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
        "state-backup" => {
            let output = arguments
                .next()
                .ok_or(
                    "state-backup requires ABSOLUTE_OUTPUT_DIRECTORY [--include-vault]",
                )?;
            let include_vault = match arguments.next().as_deref() {
                Some("--include-vault") => true,
                Some(_) => {
                    return Err(
                        "state-backup optional argument must be --include-vault".into(),
                    )
                }
                None => false,
            };
            if arguments.next().is_some() {
                return Err(
                    "state-backup accepts ABSOLUTE_OUTPUT_DIRECTORY and optional --include-vault"
                        .into(),
                );
            }
            let output = PathBuf::from(output);
            create_state_backup(&output, include_vault)?;
            println!(
                "Verified state backup written to {}",
                output.display()
            );
        }
        "state-backup-verify" => {
            let input = arguments
                .next()
                .ok_or("state-backup-verify requires ABSOLUTE_BACKUP_DIRECTORY")?;
            if arguments.next().is_some() {
                return Err(
                    "state-backup-verify accepts exactly ABSOLUTE_BACKUP_DIRECTORY"
                        .into(),
                );
            }
            let input = PathBuf::from(input);
            verify_state_backup(&input)?;
            println!("State backup verified: {}", input.display());
        }
        _ => return Err(format!("unknown connector command {command:?}").into()),
    }
    Ok(true)
}

fn create_state_backup(output: &Path, include_vault: bool) -> Result<(), DynError> {
    let sqlite_sources = [
        (
            "state.db",
            PathBuf::from(
                std::env::var("MUXPORT_STATE_DB")
                    .unwrap_or_else(|_| "muxport-state.db".into()),
            ),
        ),
        (
            "commands.db",
            PathBuf::from(
                std::env::var("MUXPORT_COMMAND_DB")
                    .unwrap_or_else(|_| "muxport-commands.db".into()),
            ),
        ),
        (
            "pairing.db",
            PathBuf::from(
                std::env::var("MUXPORT_PAIRING_DB")
                    .unwrap_or_else(|_| "muxport-pairing.db".into()),
            ),
        ),
    ];
    let vault_source = PathBuf::from(
        std::env::var("MUXPORT_VAULT_FILE")
            .unwrap_or_else(|_| "vault.sealed".into()),
    );
    create_state_backup_from_sources(
        output,
        &sqlite_sources,
        &vault_source,
        include_vault,
    )
}

fn create_state_backup_from_sources(
    output: &Path,
    sqlite_sources: &[(&str, PathBuf)],
    vault_source: &Path,
    include_vault: bool,
) -> Result<(), DynError> {
    if !output.is_absolute() {
        return Err("state-backup output directory must be absolute".into());
    }
    if output.exists() {
        return Err("state-backup output directory already exists".into());
    }
    if !sqlite_sources
        .iter()
        .any(|(_, source)| source.exists())
        && !(include_vault && vault_source.exists())
    {
        return Err("no connector state files were found to back up".into());
    }

    fs::create_dir(output)?;
    harden_private_directory(output)?;
    let mut files = Vec::new();
    for (logical_name, source) in sqlite_sources {
        if !source.exists() {
            continue;
        }
        let destination = output.join(logical_name);
        backup_sqlite(source, &destination)?;
        files.push(backup_manifest_entry(logical_name, &destination, "sqlite")?);
    }

    if include_vault && vault_source.exists() {
        let destination = output.join("vault.sealed");
        let bytes = fs::read(&vault_source)?;
        write_new_private_file(&destination, &bytes)?;
        files.push(backup_manifest_entry(
            "vault.sealed",
            &destination,
            "sealed_vault",
        )?);
    }
    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaVersion": 1,
        "createdAtMs": chrono::Utc::now().timestamp_millis(),
        "sqliteConsistency": "online_backup_api",
        "vaultConsistency": "single_atomic_generation",
        "sealedVaultIncluded": include_vault && vault_source.exists(),
        "files": files,
    }))?;
    write_new_private_file(&output.join("manifest.json"), &manifest)?;
    Ok(())
}

fn backup_sqlite(source: &Path, destination: &Path) -> Result<(), DynError> {
    let source = rusqlite::Connection::open_with_flags(
        source,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let source_integrity: String =
        source.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if source_integrity != "ok" {
        return Err(format!(
            "source SQLite integrity check failed: {source_integrity}"
        )
        .into());
    }
    let mut destination = rusqlite::Connection::open_with_flags(
        destination,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let backup = rusqlite::backup::Backup::new(&source, &mut destination)?;
    backup.run_to_completion(16, Duration::from_millis(25), None)?;
    drop(backup);
    let destination_integrity: String =
        destination.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if destination_integrity != "ok" {
        return Err(format!(
            "backup SQLite integrity check failed: {destination_integrity}"
        )
        .into());
    }
    destination.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    Ok(())
}

fn backup_manifest_entry(
    logical_name: &str,
    path: &Path,
    kind: &str,
) -> Result<serde_json::Value, DynError> {
    let bytes = fs::read(path)?;
    let mut entry = serde_json::json!({
        "name": logical_name,
        "kind": kind,
        "bytes": bytes.len(),
        "sha256": format!("{:x}", Sha256::digest(&bytes)),
    });
    if kind == "sqlite" {
        let connection = rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let schema_version: u32 =
            connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        entry["schemaVersion"] = serde_json::json!(schema_version);
    }
    Ok(entry)
}

fn verify_state_backup(input: &Path) -> Result<(), DynError> {
    if !input.is_absolute() || !input.is_dir() {
        return Err(
            "state-backup-verify input must be an existing absolute directory"
                .into(),
        );
    }
    let manifest_path = input.join("manifest.json");
    let manifest_size = fs::metadata(&manifest_path)?.len();
    if manifest_size == 0 || manifest_size > 1024 * 1024 {
        return Err("backup manifest is empty or exceeds 1 MiB".into());
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.get("schemaVersion").and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err("unsupported backup manifest schema version".into());
    }
    let created_at_ms = manifest
        .get("createdAtMs")
        .and_then(serde_json::Value::as_i64)
        .ok_or("backup manifest has no valid creation timestamp")?;
    if created_at_ms <= 0
        || created_at_ms
            > chrono::Utc::now()
                .timestamp_millis()
                .saturating_add(5 * 60 * 1000)
    {
        return Err("backup manifest creation timestamp is invalid".into());
    }
    let files = manifest
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or("backup manifest has no file list")?;
    if files.is_empty() || files.len() > 4 {
        return Err("backup manifest file list is empty or oversized".into());
    }
    let mut seen = HashSet::new();
    for entry in files {
        let name = entry
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or("backup file entry has no name")?;
        if !matches!(
            name,
            "state.db" | "commands.db" | "pairing.db" | "vault.sealed"
        ) || !seen.insert(name)
        {
            return Err("backup manifest contains an invalid or duplicate file".into());
        }
        let expected_size = entry
            .get("bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or("backup file entry has no valid size")?;
        let expected_hash = entry
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .filter(|hash| {
                hash.len() == 64
                    && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or("backup file entry has no valid SHA-256 digest")?;
        let path = input.join(name);
        let bytes = fs::read(&path)?;
        if bytes.len() as u64 != expected_size
            || format!("{:x}", Sha256::digest(&bytes)) != expected_hash
        {
            return Err(format!("backup file integrity mismatch: {name}").into());
        }
        let expected_kind = if name == "vault.sealed" {
            "sealed_vault"
        } else {
            "sqlite"
        };
        if entry.get("kind").and_then(serde_json::Value::as_str)
            != Some(expected_kind)
        {
            return Err(format!("backup file kind mismatch: {name}").into());
        }
        if expected_kind == "sqlite" {
            let connection = rusqlite::Connection::open_with_flags(
                &path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            let integrity: String =
                connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            if integrity != "ok" {
                return Err(
                    format!("backup SQLite integrity check failed: {name}").into(),
                );
            }
            let actual_schema: u32 =
                connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            if entry
                .get("schemaVersion")
                .and_then(serde_json::Value::as_u64)
                != Some(actual_schema as u64)
            {
                return Err(
                    format!("backup SQLite schema mismatch: {name}").into(),
                );
            }
        }
    }
    Ok(())
}

fn write_new_private_file(path: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn harden_private_directory(path: &Path) -> Result<(), io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    {
        let system_root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "Windows SystemRoot is unavailable")
            })?;
        let system32 = system_root.join("System32");
        let identity = std::process::Command::new(system32.join("whoami.exe"))
            .args(["/user", "/fo", "csv", "/nh"])
            .output()?;
        if !identity.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "could not resolve the current Windows user SID",
            ));
        }
        let identity = String::from_utf8(identity.stdout).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows returned a non-UTF-8 user identity",
            )
        })?;
        let sid = identity
            .split(',')
            .map(|field| field.trim().trim_matches('"'))
            .find(|field| field.starts_with("S-1-"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows user identity did not contain a SID",
                )
            })?;
        let icacls = system32.join("icacls.exe");
        for arguments in [
            vec!["/reset".to_owned(), "/Q".to_owned()],
            vec![
                "/inheritance:r".to_owned(),
                "/grant:r".to_owned(),
                format!("*{sid}:(OI)(CI)F"),
                "/grant:r".to_owned(),
                "*S-1-5-18:(OI)(CI)F".to_owned(),
                "/Q".to_owned(),
            ],
        ] {
            let status = std::process::Command::new(&icacls)
                .arg(path)
                .args(arguments)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?;
            if !status.success() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "could not enforce private backup directory permissions",
                ));
            }
        }
    }
    Ok(())
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

fn run_pairing_revoke(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(), DynError> {
    let host_id = arguments
        .next()
        .ok_or("pairing-revoke requires HOST_ID DEVICE_ID")?;
    let device_id = arguments
        .next()
        .ok_or("pairing-revoke requires HOST_ID DEVICE_ID")?;
    if arguments.next().is_some() {
        return Err("pairing-revoke accepts exactly HOST_ID DEVICE_ID".into());
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
    if store.revoke_device(&host_id, identity.signing_key(), &device_id)? {
        println!(
            "device {device_id} is revoked durably; restart the connector now to terminate its existing listener state"
        );
    } else {
        println!("device {device_id} was not present or was already revoked");
    }
    Ok(())
}

/// Rotates the protected host identity only while the connector is stopped.
/// The rotation is staged in the OS credential store, recorded in SQLite, and
/// then finalized transactionally. Existing phone identities are intentionally
/// invalidated; the provider vault is never opened or modified.
fn run_pairing_rotate_host_key(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(), DynError> {
    let host_id = arguments
        .next()
        .ok_or("pairing-rotate-host-key requires HOST_ID VERIFIED_RECOVERY_BACKUP --confirm-host-id HOST_ID")?;
    let recovery_backup = arguments
        .next()
        .ok_or("pairing-rotate-host-key requires HOST_ID VERIFIED_RECOVERY_BACKUP --confirm-host-id HOST_ID")?;
    let confirmation_flag = arguments
        .next()
        .ok_or("pairing-rotate-host-key requires --confirm-host-id HOST_ID")?;
    let confirmed_host_id = arguments
        .next()
        .ok_or("pairing-rotate-host-key requires --confirm-host-id HOST_ID")?;
    if confirmation_flag != "--confirm-host-id"
        || confirmed_host_id != host_id
        || arguments.next().is_some()
    {
        return Err(
            "pairing-rotate-host-key accepts exactly HOST_ID VERIFIED_RECOVERY_BACKUP --confirm-host-id HOST_ID"
                .into(),
        );
    }
    let recovery_backup = PathBuf::from(recovery_backup);
    verify_recovery_backup_for_host_key_rotation(&recovery_backup)?;

    let state_db = std::env::var("MUXPORT_STATE_DB")
        .unwrap_or_else(|_| "muxport-state.db".into());
    let _maintenance_lock = InstanceLock::acquire(InstanceLock::default_path_for(&state_db))?;
    let manager = HostIdentityManager::new(OsHostIdentityStore::new());
    let current = manager
        .load_existing(&host_id)?
        .ok_or("no existing protected identity exists for that host id")?;
    let pairing_db = std::env::var("MUXPORT_PAIRING_DB")
        .unwrap_or_else(|_| "muxport-pairing.db".into());
    let mut store = PairingStore::open_sqlite(&pairing_db)?;
    if complete_promoted_host_key_rotation(&host_id, &manager, &current, &mut store)? {
        println!(
            "host key rotation was completed after an interruption; all phones must be paired again"
        );
        return Ok(());
    }
    store.bind_host_identity(&host_id, &current.signing_key().verifying_key())?;

    let staged = manager.stage_rotation(&host_id)?;
    let prepared = store.prepare_host_key_rotation(
        &host_id,
        current.signing_key(),
        &staged.signing_key().verifying_key(),
    )?;
    let promoted = manager.promote_staged_rotation(&host_id)?;
    store.finalize_host_key_rotation(
        &host_id,
        &current.public_key_hex(),
        &promoted.signing_key().verifying_key(),
    )?;
    manager.clear_staged_rotation(&host_id)?;
    println!(
        "host key rotated{}; all paired phones and pending offers were invalidated, while provider credentials were left unchanged",
        if prepared { "" } else { " after resuming the staged recovery" }
    );
    Ok(())
}

fn run_pairing_list(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(), DynError> {
    let host_id = arguments
        .next()
        .ok_or("pairing-list requires HOST_ID")?;
    if arguments.next().is_some() {
        return Err("pairing-list accepts exactly HOST_ID".into());
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
    for device in store
        .load_registry(&host_id, identity.signing_key())?
        .list_devices()
    {
        println!(
            "{}\t{}\t{}\t{}",
            device.device_id,
            if device.is_revoked { "revoked" } else { "active" },
            device.device_name,
            device.public_key_hex
        );
    }
    Ok(())
}

/// Finalizes only the crash window after the protected identity changed but
/// before the pairing database could atomically invalidate old devices. If the
/// identity is still the old key, normal operation continues and the explicit
/// maintenance command can safely resume it later.
fn complete_promoted_host_key_rotation(
    host_id: &str,
    manager: &HostIdentityManager<OsHostIdentityStore>,
    current: &LoadedHostIdentity,
    store: &mut PairingStore,
) -> Result<bool, DynError> {
    let Some(pending) = store.pending_host_key_rotation(host_id)? else {
        if manager.load_staged_rotation(host_id)?.is_some() {
            manager.clear_staged_rotation(host_id)?;
        }
        return Ok(false);
    };
    let current_public_key_hex = current.public_key_hex();
    if pending.new_public_key_hex == current_public_key_hex {
        store.finalize_host_key_rotation(
            host_id,
            &pending.old_public_key_hex,
            &current.signing_key().verifying_key(),
        )?;
        manager.clear_staged_rotation(host_id)?;
        return Ok(true);
    }
    if pending.old_public_key_hex == current_public_key_hex {
        return Ok(false);
    }
    Err("pending host-key rotation does not match the protected host identity".into())
}

fn verify_recovery_backup_for_host_key_rotation(path: &Path) -> Result<(), DynError> {
    verify_state_backup(path)?;
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(path.join("manifest.json"))?)?;
    let has_pairing_database = manifest
        .get("files")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|files| {
            files.iter().any(|entry| {
                entry.get("name").and_then(serde_json::Value::as_str)
                    == Some("pairing.db")
            })
        });
    if !has_pairing_database {
        return Err(
            "host-key rotation requires a verified recovery backup containing pairing.db"
                .into(),
        );
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

/// Loads the normal OS-protected key first. A headless fallback is available
/// only when the operator explicitly points `MUXPORT_VAULT_PASSPHRASE_FILE` at
/// an owner-private credential file; plaintext environment values are never
/// accepted as a vault key source.
fn load_connector_vault_key(
    host_id: &str,
) -> Result<(KeyEncryptionKey, bool, &'static str), DynError> {
    match VaultKeyManager::new(OsVaultKeyStore::new()).load_or_create(host_id) {
        Ok(key) => {
            let created = key.was_created();
            Ok((key.into_key_encryption_key(), created, "os-protected"))
        }
        Err(os_error) => match headless_vault_key_from_env(host_id)? {
            Some(key) => Ok((key, false, "headless-passphrase-file")),
            None => Err(Box::new(os_error)),
        },
    }
}

fn headless_vault_key_from_env(
    host_id: &str,
) -> Result<Option<KeyEncryptionKey>, DynError> {
    let Some(path) = std::env::var_os("MUXPORT_VAULT_PASSPHRASE_FILE") else {
        return Ok(None);
    };
    headless_vault_key_from_passphrase_file(Path::new(&path), host_id).map(Some)
}

fn headless_vault_key_from_passphrase_file(
    path: &Path,
    host_id: &str,
) -> Result<KeyEncryptionKey, DynError> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "headless vault passphrase file must be an absolute path",
        )
        .into());
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "headless vault passphrase source must be a regular file",
        )
        .into());
    }
    if metadata.len() == 0 || metadata.len() > MAX_HEADLESS_VAULT_PASSPHRASE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "headless vault passphrase file has an invalid size",
        )
        .into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "headless vault passphrase file must be owner-private",
            )
            .into());
        }
    }

    let mut passphrase = fs::read(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "headless vault passphrase file could not be read",
        )
    })?;
    let mut salt = [0_u8; 16];
    let mut hasher = Sha256::new();
    hasher.update(b"muxport-headless-vault-salt-v1");
    hasher.update((host_id.len() as u64).to_be_bytes());
    hasher.update(host_id.as_bytes());
    salt.copy_from_slice(&hasher.finalize()[..16]);
    let key = KeyEncryptionKey::derive_from_passphrase(&passphrase, &salt)
        .map_err(|error| Box::new(error) as DynError);
    passphrase.zeroize();
    salt.zeroize();
    key
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
    fn owner_private_headless_passphrase_reopens_the_same_vault() {
        let root = std::env::temp_dir().join(format!(
            "muxport-headless-vault-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let passphrase_file = root.join("credential");
        fs::write(&passphrase_file, b"headless-test-passphrase").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&passphrase_file, fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        let vault_path = root.join("vault.sealed");
        {
            let mut vault = PersistentVault::open_or_create(
                &vault_path,
                "host-headless",
                headless_vault_key_from_passphrase_file(&passphrase_file, "host-headless")
                    .unwrap(),
            )
            .unwrap();
            vault
                .enroll_credential(
                    credential_vault::CredentialEnrollment {
                        profile_id: "profile-1".into(),
                        display_name: "Headless profile".into(),
                        provider: "provider".into(),
                        credential_type: "api_key".into(),
                        account_fingerprint: "fingerprint".into(),
                        created_at_ms: 1,
                        last_validated_at_ms: 1,
                    },
                    b"secret",
                )
                .unwrap();
        }
        let reopened = PersistentVault::open_or_create(
            &vault_path,
            "host-headless",
            headless_vault_key_from_passphrase_file(&passphrase_file, "host-headless")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            reopened
                .decrypt_active_secret("profile-1")
                .unwrap()
                .expose_secret(),
            b"secret"
        );
        drop(reopened);
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn headless_passphrase_file_rejects_group_or_world_access() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "muxport-headless-permissions-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let passphrase_file = root.join("credential");
        fs::write(&passphrase_file, b"test-passphrase").unwrap();
        fs::set_permissions(&passphrase_file, fs::Permissions::from_mode(0o644))
            .unwrap();
        assert!(headless_vault_key_from_passphrase_file(&passphrase_file, "host-1").is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn online_backup_includes_committed_wal_state_and_passes_integrity_check() {
        let root = std::env::temp_dir().join(format!(
            "muxport-online-backup-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let source_path = root.join("source.db");
        let backup_path = root.join("backup.db");
        let source = rusqlite::Connection::open(&source_path).unwrap();
        source.pragma_update(None, "journal_mode", "WAL").unwrap();
        source
            .execute_batch(
                "CREATE TABLE evidence (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO evidence (value) VALUES ('committed-in-wal');",
            )
            .unwrap();
        assert!(source_path.with_extension("db-wal").exists());

        backup_sqlite(&source_path, &backup_path).unwrap();

        let restored = rusqlite::Connection::open(&backup_path).unwrap();
        let value: String = restored
            .query_row("SELECT value FROM evidence WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let integrity: String = restored
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "committed-in-wal");
        assert_eq!(integrity, "ok");
        drop(restored);

        let vault_path = root.join("vault.sealed");
        fs::write(&vault_path, b"sealed-vault-generation").unwrap();
        let sources = [("state.db", source_path.clone())];
        let default_output = root.join("default-backup");
        create_state_backup_from_sources(
            &default_output,
            &sources,
            &vault_path,
            false,
        )
        .unwrap();
        assert!(!default_output.join("vault.sealed").exists());
        let default_manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(default_output.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(default_manifest["sealedVaultIncluded"], false);
        assert_eq!(
            default_manifest["files"][0]["schemaVersion"],
            serde_json::json!(0)
        );
        verify_state_backup(&default_output).unwrap();

        let recovery_output = root.join("recovery-backup");
        create_state_backup_from_sources(
            &recovery_output,
            &sources,
            &vault_path,
            true,
        )
        .unwrap();
        assert_eq!(
            fs::read(recovery_output.join("vault.sealed")).unwrap(),
            b"sealed-vault-generation"
        );
        let recovery_manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(recovery_output.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(recovery_manifest["sealedVaultIncluded"], true);
        verify_state_backup(&recovery_output).unwrap();
        fs::write(
            recovery_output.join("vault.sealed"),
            b"tampered-vault-generation",
        )
        .unwrap();
        assert!(verify_state_backup(&recovery_output).is_err());
        drop(source);
        fs::remove_dir_all(root).unwrap();
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

    #[test]
    fn durable_session_identity_overrides_adapter_guesses_without_relabelling_unknown_sessions() {
        let config = RuntimeConfig {
            runtime_id: "codex-test".into(),
            agent_type: AgentType::Codex,
            runtime_name: "Codex".into(),
            credential_profile_id: Arc::new(RwLock::new("current-profile".into())),
            session_assignments: Arc::new(RwLock::new(HashMap::from([(
                "known-session".into(),
                "historical-profile".into(),
            )]))),
            managed_opencode: None,
            connector_managed: true,
        };
        let mut sessions = vec![
            SessionSummary {
                session_id: "known-session".into(),
                project_path: "/repo".into(),
                title: "Known".into(),
                status: "idle".into(),
                credential_profile_id: "incorrect-current-profile".into(),
                created_at_ms: 1,
            },
            SessionSummary {
                session_id: "unknown-session".into(),
                project_path: "/repo".into(),
                title: "Unknown".into(),
                status: "idle".into(),
                credential_profile_id: String::new(),
                created_at_ms: 2,
            },
        ];

        config.apply_session_assignments(&mut sessions);

        assert_eq!(
            sessions[0].credential_profile_id,
            "historical-profile"
        );
        assert!(sessions[1].credential_profile_id.is_empty());
    }

    #[tokio::test]
    async fn synchronization_brackets_subscription_with_snapshots() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let config = RuntimeConfig {
            runtime_id: "codex-test".into(),
            agent_type: AgentType::Codex,
            runtime_name: "Codex".into(),
            credential_profile_id: Arc::new(RwLock::new(String::new())),
            session_assignments: Arc::new(RwLock::new(HashMap::new())),
            managed_opencode: None,
            connector_managed: true,
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
            ManagedOpenCodeChild::start(profile, "test-server-password".into(), true).unwrap();
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

    #[cfg(unix)]
    #[tokio::test]
    async fn managed_opencode_restart_policy_can_leave_an_exited_runtime_stopped() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "muxport-managed-no-restart-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let executable = root.join("fake-opencode");
        std::fs::write(&executable, "#!/bin/sh\nprintf 'launch\\n' >> launches.txt\nexit 7\n")
            .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let profile =
            ManagedOpenCodeProfile::prepare(&root, "profile-a", executable, &project, 43120)
                .unwrap();
        let mut managed =
            ManagedOpenCodeChild::start(profile, "test-server-password".into(), false).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(matches!(
            managed.ensure_running(),
            Err(AdapterError::InitFailed(detail)) if detail.contains("restart_on_failure is disabled")
        ));
        assert_eq!(
            std::fs::read_to_string(project.join("launches.txt"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn managed_exit_diagnostic_never_exposes_raw_stderr() {
        let root = std::env::temp_dir().join(format!(
            "muxport-redacted-stderr-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let profile = ManagedOpenCodeProfile::prepare(
            &root,
            "profile-a",
            "/bin/sh",
            &project,
            43120,
        )
        .unwrap();
        let status = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .status()
            .unwrap();
        let tail = Arc::new(Mutex::new(b"provider_token=do-not-log".to_vec()));
        let diagnostic = managed_exit_diagnostic(&profile, &status, &tail);
        assert!(diagnostic.contains("profile_id=profile-a"));
        assert!(diagnostic.contains("exit_code=7"));
        assert!(diagnostic.contains("stderr_tail_sha256="));
        assert!(!diagnostic.contains("do-not-log"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
