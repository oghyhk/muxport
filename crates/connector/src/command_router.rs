use adapter_api::{AdapterError, AgentAdapter};
use connector_core::{activate_staged_credential, CommandLedger, CoreError, CredentialSwitchError};
use credential_vault::PersistentVault;
use muxport_protocol::{
    command, Command, CommandResult, RemoteOpState,
};
use prost::Message;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex as StdMutex};
use thiserror::Error;
use tokio::sync::{Mutex, Notify};

#[derive(Error, Debug)]
pub enum CommandDispatchError {
    #[error(transparent)]
    Ledger(#[from] CoreError),
    #[error("Stored command result is corrupt: {0}")]
    CorruptResult(String),
    #[error("Command result serialization failed: {0}")]
    ResultSerialization(#[from] serde_json::Error),
    #[error("Command router state lock was poisoned")]
    StateLockPoisoned,
}

/// Durable, idempotent command boundary shared by direct and relay transports.
///
/// The ledger reservation and dispatched marker are persisted before an
/// adapter is invoked. A crash or cancellation after that point is never
/// retried blindly; the next duplicate receives `ReconciliationRequired`.
pub struct CommandRouter {
    ledger: Mutex<CommandLedger>,
    adapters: HashMap<String, Arc<dyn AgentAdapter>>,
    vault: Option<Arc<Mutex<PersistentVault>>>,
    in_flight: StdMutex<HashMap<String, Arc<Notify>>>,
}

impl CommandRouter {
    pub fn new(
        ledger: CommandLedger,
        adapters: HashMap<String, Arc<dyn AgentAdapter>>,
    ) -> Self {
        Self {
            ledger: Mutex::new(ledger),
            adapters,
            vault: None,
            in_flight: StdMutex::new(HashMap::new()),
        }
    }

    pub fn open_sqlite(
        path: impl AsRef<Path>,
        adapters: HashMap<String, Arc<dyn AgentAdapter>>,
    ) -> Result<Self, CommandDispatchError> {
        Ok(Self::new(CommandLedger::open_sqlite(path)?, adapters))
    }

    pub fn open_sqlite_with_vault(
        path: impl AsRef<Path>,
        adapters: HashMap<String, Arc<dyn AgentAdapter>>,
        vault: Arc<Mutex<PersistentVault>>,
    ) -> Result<Self, CommandDispatchError> {
        Ok(Self {
            ledger: Mutex::new(CommandLedger::open_sqlite(path)?),
            adapters,
            vault: Some(vault),
            in_flight: StdMutex::new(HashMap::new()),
        })
    }

    pub async fn dispatch(
        &self,
        idempotency_key: &str,
        command: &Command,
    ) -> Result<CommandResult, CommandDispatchError> {
        let fingerprint = command_fingerprint(command);
        let _dispatch_guard = 'reserve: loop {
            let prior = {
                let mut ledger = self.ledger.lock().await;
                match ledger.reserve_command(
                    idempotency_key,
                    command.deadline_ms,
                    &fingerprint,
                ) {
                    Ok(None) => {
                        ledger.record_result(
                            idempotency_key.to_owned(),
                            RemoteOpState::Dispatched,
                            String::new(),
                        )?;
                        break 'reserve self.begin_in_flight(idempotency_key)?;
                    }
                    Ok(prior) => prior,
                    Err(CoreError::CommandExpired) => {
                        return Ok(command_result(
                            command,
                            RemoteOpState::Expired,
                            false,
                            "command deadline has passed",
                            String::new(),
                        ));
                    }
                    Err(error) => return Err(error.into()),
                }
            };

            let Some((state, stored_result)) = prior else {
                return Err(CommandDispatchError::CorruptResult(
                    "ledger returned an empty prior reservation".into(),
                ));
            };
            if stored_result.is_empty() {
                if !is_reconcilable_incomplete(state) {
                    return Err(CommandDispatchError::CorruptResult(format!(
                        "idempotency key {idempotency_key} has terminal state {state:?} without a result"
                    )));
                }
                if let Some(notify) = self.in_flight_notification(idempotency_key)? {
                    notify.notified().await;
                    continue 'reserve;
                }
                let result = command_result(
                    command,
                    RemoteOpState::ReconciliationRequired,
                    false,
                    "a prior dispatch did not persist a terminal result; reconcile source state before retrying",
                    String::new(),
                );
                self.persist_result(idempotency_key, &result).await?;
                return Ok(result);
            }
            return restore_stored_result(command, state, &stored_result);
        };

        let result = match self.execute(command).await {
            Ok(value) => command_result(
                command,
                RemoteOpState::Succeeded,
                true,
                "",
                serde_json::to_string(&value)?,
            ),
            Err(error) => {
                let state = if outcome_requires_reconciliation(&error) {
                    RemoteOpState::ReconciliationRequired
                } else {
                    RemoteOpState::Failed
                };
                command_result(command, state, false, &error.to_string(), String::new())
            }
        };
        self.persist_result(idempotency_key, &result).await?;
        Ok(result)
    }

    fn begin_in_flight(
        &self,
        idempotency_key: &str,
    ) -> Result<InFlightGuard<'_>, CommandDispatchError> {
        let mut in_flight = self
            .in_flight
            .lock()
            .map_err(|_| CommandDispatchError::StateLockPoisoned)?;
        if in_flight.contains_key(idempotency_key) {
            return Err(CommandDispatchError::CorruptResult(format!(
                "new reservation {idempotency_key} was already in flight"
            )));
        }
        in_flight.insert(idempotency_key.to_owned(), Arc::new(Notify::new()));
        Ok(InFlightGuard {
            router: self,
            idempotency_key: idempotency_key.to_owned(),
        })
    }

    fn in_flight_notification(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<Arc<Notify>>, CommandDispatchError> {
        self.in_flight
            .lock()
            .map_err(|_| CommandDispatchError::StateLockPoisoned)
            .map(|in_flight| in_flight.get(idempotency_key).cloned())
    }

    async fn execute(&self, command: &Command) -> Result<Value, AdapterError> {
        if command.command_id.trim().is_empty() {
            return Err(AdapterError::InvalidInput(
                "command id must not be empty".into(),
            ));
        }
        match command.inner.as_ref() {
            Some(command::Inner::StartSession(request)) => {
                let adapter = self.adapter(&request.runtime_id)?;
                let session_id = adapter
                    .start_session(
                        &request.project_path,
                        &request.prompt,
                        &request.credential_profile_id,
                    )
                    .await?;
                Ok(json!({"session_id": session_id}))
            }
            Some(command::Inner::SendInput(request)) => {
                let adapter = self.adapter(&request.runtime_id)?;
                adapter
                    .send_input(&request.session_id, &request.text)
                    .await?;
                Ok(json!({}))
            }
            Some(command::Inner::Steer(request)) => {
                let adapter = self.adapter(&request.runtime_id)?;
                adapter
                    .steer(&request.session_id, &request.instruction)
                    .await?;
                Ok(json!({}))
            }
            Some(command::Inner::Interrupt(request)) => {
                let adapter = self.adapter(&request.runtime_id)?;
                adapter
                    .interrupt(&request.session_id, &request.reason)
                    .await?;
                Ok(json!({}))
            }
            Some(command::Inner::ApproveAction(request)) => {
                let adapter = self.adapter(&request.runtime_id)?;
                adapter
                    .respond_approval(
                        &request.session_id,
                        &request.approval_id,
                        request.approved,
                        &request.decision_reason,
                    )
                    .await?;
                Ok(json!({}))
            }
            Some(command::Inner::ProbeHost(_)) => Ok(json!({"reachable": true})),
            Some(command::Inner::ChangeAssignment(request)) => {
                if request.target_type != "runtime" {
                    return Err(AdapterError::Unsupported(
                        "project credential assignments require the desired-state registry"
                            .into(),
                    ));
                }
                let adapter = self.adapter(&request.target_id)?;
                let profile_id = request.new_credential_profile_id.trim();
                if profile_id.is_empty() {
                    return Err(AdapterError::InvalidInput(
                        "new credential profile id must not be empty".into(),
                    ));
                }
                let vault = self.vault.as_ref().ok_or_else(|| {
                    AdapterError::Unsupported("credential vault is unavailable or locked".into())
                })?;
                let mut vault = vault.lock().await;
                let provider = vault
                    .get_profile(profile_id)
                    .map(|profile| profile.provider)
                    .ok_or_else(|| {
                        AdapterError::InvalidInput(format!(
                            "credential profile {profile_id:?} does not exist"
                        ))
                    })?;
                let result = activate_staged_credential(
                    adapter.as_ref(),
                    &mut vault,
                    profile_id,
                    &provider,
                )
                .await
                .map_err(map_credential_switch_error)?;
                Ok(json!({
                    "profile_id": result.profile_id,
                    "provider_id": result.provider_id,
                    "account_fingerprint": result.account_fingerprint,
                    "activated_at_ms": result.activated_at_ms
                }))
            }
            Some(command::Inner::RotateCredential(_)) => Err(AdapterError::Unsupported(
                "credential rotation is not connected to managed runtimes".into(),
            )),
            None => Err(AdapterError::InvalidInput(
                "command payload must not be empty".into(),
            )),
        }
    }

    fn adapter(&self, runtime_id: &str) -> Result<Arc<dyn AgentAdapter>, AdapterError> {
        if runtime_id.trim().is_empty() {
            return Err(AdapterError::InvalidInput(
                "runtime id must not be empty".into(),
            ));
        }
        self.adapters.get(runtime_id).cloned().ok_or_else(|| {
            AdapterError::InvalidInput(format!("runtime {runtime_id:?} is not registered"))
        })
    }

    async fn persist_result(
        &self,
        idempotency_key: &str,
        result: &CommandResult,
    ) -> Result<(), CommandDispatchError> {
        let state = RemoteOpState::try_from(result.state).map_err(|_| {
            CommandDispatchError::CorruptResult(format!(
                "result has unknown state {}",
                result.state
            ))
        })?;
        let serialized = serde_json::to_string(result)?;
        self.ledger.lock().await.record_result(
            idempotency_key.to_owned(),
            state,
            serialized,
        )?;
        Ok(())
    }
}

struct InFlightGuard<'a> {
    router: &'a CommandRouter,
    idempotency_key: String,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        let notify = self
            .router
            .in_flight
            .lock()
            .ok()
            .and_then(|mut in_flight| in_flight.remove(&self.idempotency_key));
        if let Some(notify) = notify {
            notify.notify_waiters();
        }
    }
}

fn restore_stored_result(
    command: &Command,
    state: RemoteOpState,
    stored_result: &str,
) -> Result<CommandResult, CommandDispatchError> {
    let result: CommandResult = serde_json::from_str(stored_result)?;
    if result.command_id != command.command_id {
        return Err(CommandDispatchError::CorruptResult(format!(
            "stored command id {:?} does not match {:?}",
            result.command_id, command.command_id
        )));
    }
    let stored_state = RemoteOpState::try_from(result.state).map_err(|_| {
        CommandDispatchError::CorruptResult(format!(
            "stored result has unknown state {}",
            result.state
        ))
    })?;
    if stored_state != state {
        return Err(CommandDispatchError::CorruptResult(format!(
            "ledger state {state:?} does not match result state {stored_state:?}"
        )));
    }
    Ok(result)
}

fn command_fingerprint(command: &Command) -> String {
    let digest = Sha256::digest(command.encode_to_vec());
    let mut fingerprint = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        fingerprint.push(HEX[(byte >> 4) as usize] as char);
        fingerprint.push(HEX[(byte & 0x0f) as usize] as char);
    }
    fingerprint
}

fn command_result(
    command: &Command,
    state: RemoteOpState,
    success: bool,
    error_message: &str,
    result_json: String,
) -> CommandResult {
    CommandResult {
        command_id: command.command_id.clone(),
        state: state as i32,
        success,
        error_message: error_message.to_owned(),
        completed_at_ms: chrono::Utc::now().timestamp_millis(),
        result_json,
    }
}

fn is_reconcilable_incomplete(state: RemoteOpState) -> bool {
    matches!(
        state,
        RemoteOpState::Created
            | RemoteOpState::Persisted
            | RemoteOpState::Dispatched
            | RemoteOpState::SourceAcknowledged
            | RemoteOpState::OutcomeUnknown
            | RemoteOpState::ReconciliationRequired
    )
}

fn outcome_requires_reconciliation(error: &AdapterError) -> bool {
    matches!(
        error,
        AdapterError::ConnectionLost
            | AdapterError::ConnectionLostWithDetail(_)
            | AdapterError::OutcomeUnknown(_)
    )
}

fn map_credential_switch_error(error: CredentialSwitchError) -> AdapterError {
    let detail = error.to_string();
    match error {
        CredentialSwitchError::RollbackFailed { .. } => {
            AdapterError::OutcomeUnknown(detail)
        }
        CredentialSwitchError::Adapter(AdapterError::ConnectionLost)
        | CredentialSwitchError::Adapter(AdapterError::ConnectionLostWithDetail(_))
        | CredentialSwitchError::Adapter(AdapterError::OutcomeUnknown(_)) => {
            AdapterError::OutcomeUnknown(detail)
        }
        CredentialSwitchError::ActivationRolledBack(_) => {
            AdapterError::CredentialInvalid(detail)
        }
        _ => AdapterError::InvalidInput(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use credential_vault::{CredentialEnrollment, KeyEncryptionKey};
    use muxport_protocol::{AgentType, ChangeAssignmentCmd, Command, StartSessionCmd};
    use std::sync::Arc;
    use test_harness::DeterministicFakeAdapter;

    fn start_command(prompt: &str) -> Command {
        Command {
            command_id: "command-1".into(),
            deadline_ms: chrono::Utc::now().timestamp_millis() + 60_000,
            inner: Some(command::Inner::StartSession(StartSessionCmd {
                runtime_id: "codex-test".into(),
                project_path: "/fake/repo".into(),
                prompt: prompt.into(),
                credential_profile_id: String::new(),
            })),
        }
    }

    fn router(
        adapter: Arc<DeterministicFakeAdapter>,
        ledger: CommandLedger,
    ) -> CommandRouter {
        let mut adapters: HashMap<String, Arc<dyn AgentAdapter>> = HashMap::new();
        adapters.insert("codex-test".into(), adapter);
        CommandRouter::new(ledger, adapters)
    }

    fn assignment_command(profile_id: &str) -> Command {
        Command {
            command_id: "assignment-command-1".into(),
            deadline_ms: chrono::Utc::now().timestamp_millis() + 60_000,
            inner: Some(command::Inner::ChangeAssignment(ChangeAssignmentCmd {
                target_type: "runtime".into(),
                target_id: "codex-test".into(),
                new_credential_profile_id: profile_id.into(),
            })),
        }
    }

    #[tokio::test]
    async fn duplicate_command_returns_persisted_result_without_redispatch() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let router = router(Arc::clone(&adapter), CommandLedger::new());
        let command = start_command("build it");
        let first = router.dispatch("same-key", &command).await.unwrap();
        let second = router.dispatch("same-key", &command).await.unwrap();

        assert!(first.success);
        assert_eq!(first.result_json, second.result_json);
        assert_eq!(adapter.start_session_count(), 1);
    }

    #[tokio::test]
    async fn concurrent_duplicate_waits_for_original_dispatch() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        adapter.set_start_session_delay(std::time::Duration::from_millis(25));
        let router = router(Arc::clone(&adapter), CommandLedger::new());
        let command = start_command("build it");

        let (first, second) = tokio::join!(
            router.dispatch("same-key", &command),
            router.dispatch("same-key", &command)
        );
        assert!(first.unwrap().success);
        assert!(second.unwrap().success);
        assert_eq!(adapter.start_session_count(), 1);
    }

    #[tokio::test]
    async fn idempotency_key_reuse_with_different_payload_is_rejected() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let router = router(adapter, CommandLedger::new());
        router
            .dispatch("same-key", &start_command("first"))
            .await
            .unwrap();
        assert!(matches!(
            router.dispatch("same-key", &start_command("different")).await,
            Err(CommandDispatchError::Ledger(CoreError::DuplicateCommand(key)))
                if key == "same-key"
        ));
    }

    #[tokio::test]
    async fn incomplete_prior_dispatch_requires_reconciliation() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let command = start_command("build it");
        let mut ledger = CommandLedger::new();
        ledger
            .reserve_command("crashed-key", command.deadline_ms, &command_fingerprint(&command))
            .unwrap();
        ledger
            .record_result(
                "crashed-key".into(),
                RemoteOpState::Dispatched,
                String::new(),
            )
            .unwrap();
        let router = router(Arc::clone(&adapter), ledger);

        let result = router.dispatch("crashed-key", &command).await.unwrap();
        assert_eq!(
            RemoteOpState::try_from(result.state).unwrap(),
            RemoteOpState::ReconciliationRequired
        );
        assert!(!result.success);
        assert_eq!(adapter.start_session_count(), 0);
    }

    #[tokio::test]
    async fn terminal_result_survives_router_restart() {
        let db_path = std::env::temp_dir().join(format!(
            "muxport-router-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let command = start_command("build it");
        {
            let mut adapters: HashMap<String, Arc<dyn AgentAdapter>> = HashMap::new();
            adapters.insert("codex-test".into(), adapter.clone());
            let router = CommandRouter::open_sqlite(&db_path, adapters).unwrap();
            assert!(
                router
                    .dispatch("persistent-key", &command)
                    .await
                    .unwrap()
                    .success
            );
        }
        {
            let mut adapters: HashMap<String, Arc<dyn AgentAdapter>> = HashMap::new();
            adapters.insert("codex-test".into(), adapter.clone());
            let router = CommandRouter::open_sqlite(&db_path, adapters).unwrap();
            assert!(
                router
                    .dispatch("persistent-key", &command)
                    .await
                    .unwrap()
                    .success
            );
        }
        assert_eq!(adapter.start_session_count(), 1);

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[tokio::test]
    async fn adapter_unknown_outcome_is_persisted_for_reconciliation() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        adapter.set_start_session_outcome_unknown(true);
        let router = router(Arc::clone(&adapter), CommandLedger::new());
        let command = start_command("build it");

        let first = router.dispatch("unknown-key", &command).await.unwrap();
        let second = router.dispatch("unknown-key", &command).await.unwrap();
        assert_eq!(
            RemoteOpState::try_from(first.state).unwrap(),
            RemoteOpState::ReconciliationRequired
        );
        assert_eq!(first.state, second.state);
        assert_eq!(adapter.start_session_count(), 1);
    }

    #[tokio::test]
    async fn expired_command_never_reaches_adapter() {
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let router = router(Arc::clone(&adapter), CommandLedger::new());
        let mut command = start_command("build it");
        command.deadline_ms = chrono::Utc::now().timestamp_millis() - 1;

        let result = router.dispatch("expired-key", &command).await.unwrap();
        assert_eq!(
            RemoteOpState::try_from(result.state).unwrap(),
            RemoteOpState::Expired
        );
        assert_eq!(adapter.start_session_count(), 0);
    }

    #[tokio::test]
    async fn runtime_assignment_activates_staged_vault_secret_transactionally() {
        let db_path = std::env::temp_dir().join(format!(
            "muxport-router-assignment-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let vault_path = db_path.with_extension("vault");
        let mut vault = PersistentVault::open_or_create(
            &vault_path,
            "host-test",
            KeyEncryptionKey::derive_from_passphrase(
                b"test-only-vault-passphrase",
                &[7_u8; 16],
            )
            .unwrap(),
        )
        .unwrap();
        vault
            .enroll_credential(
                CredentialEnrollment {
                    profile_id: "profile-a".into(),
                    display_name: "Profile A".into(),
                    provider: "openai".into(),
                    credential_type: "api_key".into(),
                    account_fingerprint: "account-a".into(),
                    created_at_ms: 1,
                    last_validated_at_ms: 1,
                },
                b"old-secret-key",
            )
            .unwrap();
        vault
            .stage_credential("profile-a", b"new-secret-key")
            .unwrap();
        let vault = Arc::new(Mutex::new(vault));
        let adapter = Arc::new(DeterministicFakeAdapter::new(AgentType::Codex));
        let mut adapters: HashMap<String, Arc<dyn AgentAdapter>> = HashMap::new();
        adapters.insert("codex-test".into(), adapter);
        let router =
            CommandRouter::open_sqlite_with_vault(&db_path, adapters, Arc::clone(&vault))
                .unwrap();

        let result = router
            .dispatch(
                "assignment-idempotency-key",
                &assignment_command("profile-a"),
            )
            .await
            .unwrap();
        assert!(result.success, "{}", result.error_message);
        assert_eq!(
            vault
                .lock()
                .await
                .decrypt_active_secret("profile-a")
                .unwrap()
                .expose_secret(),
            b"new-secret-key"
        );

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
        let _ = std::fs::remove_file(vault_path);
    }
}
