mod bulk_switch;
mod credential_switch;
mod rotation;

pub use bulk_switch::{
    BulkSwitchError, BulkSwitchOperation, BulkSwitchSummary, BulkSwitchTarget,
    BulkSwitchTargetState,
};
pub use credential_switch::{
    activate_staged_credential, switch_runtime_assignment, CredentialRuntime,
    CredentialSwitchError, CredentialSwitchResult,
};
pub use rotation::{
    ProviderFailureClass, RotationDecision, RotationEligibility, RotationError,
    RotationMode, RotationPool, RotationRequest, RotationTrigger,
};

use muxport_protocol::{ConnectorState, RemoteOpState, RuntimeState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    #[error("Command idempotency conflict or duplicate key: {0}")]
    DuplicateCommand(String),
    #[error("Command expired (deadline passed)")]
    CommandExpired,
    #[error("Idempotency key must not be empty")]
    EmptyIdempotencyKey,
    #[error("Command fingerprint must not be empty")]
    EmptyCommandFingerprint,
    #[error("Command was not reserved before recording a result: {0}")]
    UnknownCommand(String),
    #[error("Bulk switch operation does not exist: {0}")]
    UnknownBulkSwitchOperation(String),
    #[error("Command ledger contains an invalid operation state: {0}")]
    InvalidStoredState(i32),
    #[error("Command ledger integrity check failed: {0}")]
    IntegrityCheckFailed(String),
    #[error("Command ledger schema version {found} is newer than supported version {supported}")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },
    #[error("Assignment field {0} must not be empty")]
    EmptyAssignmentField(&'static str),
    #[error("Conflicting credential assignments exist at precedence {0}")]
    AmbiguousAssignment(u8),
    #[error(
        "Session {session_id} in runtime {runtime_id} is already assigned to {existing}, not {requested}"
    )]
    SessionAssignmentConflict {
        runtime_id: String,
        session_id: String,
        existing: String,
        requested: String,
    },
    #[error("Rotation pool does not exist: {0}")]
    RotationPoolNotFound(String),
    #[error("Rotation pool row id {row} does not match policy id {policy}")]
    RotationPoolIdMismatch { row: String, policy: String },
    #[error("Rotation policy serialization failed: {0}")]
    RotationSerialization(#[from] serde_json::Error),
    #[error("Audit record field {0} is empty or exceeds the redacted limit")]
    InvalidAuditRecord(&'static str),
    #[error("Bulk switch operation row id {row} does not match operation id {operation}")]
    BulkSwitchOperationIdMismatch { row: String, operation: String },
    #[error(transparent)]
    BulkSwitch(#[from] BulkSwitchError),
    #[error(transparent)]
    Rotation(#[from] RotationError),
    #[error("Command ledger database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

pub fn validate_connector_transition(from: ConnectorState, to: ConnectorState) -> Result<(), CoreError> {
    use ConnectorState::*;
    let valid = match (from, to) {
        (Starting, VaultLocked) | (Starting, Recovering) | (Starting, FatalError) => true,
        (VaultLocked, Recovering) | (VaultLocked, Ready) => true,
        (Recovering, Ready) | (Recovering, Degraded) => true,
        (Ready, Degraded) | (Ready, VaultLocked) => true,
        (Degraded, Ready) | (Degraded, FatalError) => true,
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(CoreError::InvalidTransition {
            from: format!("{:?}", from),
            to: format!("{:?}", to),
        })
    }
}

pub fn validate_runtime_transition(from: RuntimeState, to: RuntimeState) -> Result<(), CoreError> {
    use RuntimeState::*;
    let valid = match (from, to) {
        (Unknown, Discovering) => true,
        (Discovering, Starting) | (Discovering, Degraded) | (Discovering, Stopped) => true,
        (Starting, Synchronizing) | (Starting, Crashed) | (Starting, CredentialLocked) => true,
        (Synchronizing, OnlineIdle)
        | (Synchronizing, OnlineRunning)
        | (Synchronizing, OnlineWaitingApproval)
        | (Synchronizing, Degraded)
        | (Synchronizing, Crashed)
        | (Synchronizing, CredentialLocked)
        | (Synchronizing, Stopped) => true,
        (OnlineIdle, OnlineRunning)
        | (OnlineIdle, OnlineWaitingApproval)
        | (OnlineIdle, Synchronizing)
        | (OnlineIdle, Degraded)
        | (OnlineIdle, Crashed)
        | (OnlineIdle, Stopped) => true,
        (OnlineRunning, OnlineIdle)
        | (OnlineRunning, OnlineWaitingApproval)
        | (OnlineRunning, Synchronizing)
        | (OnlineRunning, Degraded)
        | (OnlineRunning, Crashed) => true,
        (OnlineWaitingApproval, OnlineRunning)
        | (OnlineWaitingApproval, OnlineIdle)
        | (OnlineWaitingApproval, Synchronizing)
        | (OnlineWaitingApproval, Degraded)
        | (OnlineWaitingApproval, Crashed) => true,
        (Crashed, Starting) | (Crashed, CrashLoop) => true,
        (CrashLoop, Starting) | (CrashLoop, Stopped) => true,
        (Stopped, Starting) => true,
        (Degraded, Discovering) | (Degraded, Starting) | (Degraded, Synchronizing) => true,
        (CredentialLocked, Starting) | (CredentialLocked, Stopped) => true,
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(CoreError::InvalidTransition {
            from: format!("{:?}", from),
            to: format!("{:?}", to),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DesiredRuntimeConfig {
    pub runtime_id: String,
    pub assigned_profile_id: String,
    pub autostart: bool,
    pub restart_on_crash: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObservedRuntimeStatus {
    pub runtime_id: String,
    pub pid: Option<u32>,
    pub current_state: RuntimeState,
    pub active_profile_id: String,
}

pub struct DesiredObservedReconciler;

impl DesiredObservedReconciler {
    pub fn reconcile(
        desired: &DesiredRuntimeConfig,
        observed: &ObservedRuntimeStatus,
    ) -> Vec<String> {
        let mut actions = Vec::new();

        if desired.autostart && observed.current_state == RuntimeState::Stopped {
            actions.push(format!("start_runtime:{}", desired.runtime_id));
        }

        if desired.assigned_profile_id != observed.active_profile_id {
            actions.push(format!(
                "switch_profile:{}:{}",
                desired.runtime_id, desired.assigned_profile_id
            ));
        }

        actions
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AssignmentScope {
    Global,
    Host {
        host_id: String,
    },
    Runtime {
        host_id: String,
        runtime_id: String,
    },
    Project {
        host_id: String,
        runtime_id: String,
        project_path: String,
    },
}

impl AssignmentScope {
    fn precedence(&self) -> u8 {
        match self {
            Self::Global => 0,
            Self::Host { .. } => 1,
            Self::Runtime { .. } => 2,
            Self::Project { .. } => 3,
        }
    }

    fn matches(&self, target: &AssignmentTarget<'_>) -> bool {
        match self {
            Self::Global => true,
            Self::Host { host_id } => host_id == target.host_id,
            Self::Runtime {
                host_id,
                runtime_id,
            } => host_id == target.host_id && runtime_id == target.runtime_id,
            Self::Project {
                host_id,
                runtime_id,
                project_path,
            } => {
                host_id == target.host_id
                    && runtime_id == target.runtime_id
                    && target.project_path == Some(project_path.as_str())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CredentialAssignment {
    pub profile_id: String,
    pub scope: AssignmentScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignmentTarget<'a> {
    pub host_id: &'a str,
    pub runtime_id: &'a str,
    pub project_path: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAssignment {
    pub profile_id: String,
    pub scope: AssignmentScope,
}

pub fn resolve_assignment(
    assignments: &[CredentialAssignment],
    target: AssignmentTarget<'_>,
) -> Result<Option<ResolvedAssignment>, CoreError> {
    let mut selected: Option<&CredentialAssignment> = None;
    for assignment in assignments
        .iter()
        .filter(|assignment| assignment.scope.matches(&target))
    {
        if assignment.profile_id.trim().is_empty() {
            return Err(CoreError::EmptyAssignmentField("profile_id"));
        }
        match selected {
            None => selected = Some(assignment),
            Some(current) if assignment.scope.precedence() > current.scope.precedence() => {
                selected = Some(assignment);
            }
            Some(current)
                if assignment.scope.precedence() == current.scope.precedence()
                    && assignment.profile_id != current.profile_id =>
            {
                return Err(CoreError::AmbiguousAssignment(
                    assignment.scope.precedence(),
                ));
            }
            _ => {}
        }
    }
    Ok(selected.map(|assignment| ResolvedAssignment {
        profile_id: assignment.profile_id.clone(),
        scope: assignment.scope.clone(),
    }))
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SwitchImpactTarget {
    pub host_id: String,
    pub runtime_id: String,
    pub provider_compatible: bool,
    pub online: bool,
    pub credential_unlocked: bool,
    pub managed: bool,
    pub active_turns: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct SwitchImpactPlan {
    pub ready: Vec<String>,
    pub incompatible: Vec<String>,
    pub offline: Vec<String>,
    pub locked: Vec<String>,
    pub busy: Vec<String>,
    pub unmanaged: Vec<String>,
}

impl SwitchImpactPlan {
    pub fn build(targets: &[SwitchImpactTarget]) -> Self {
        let mut plan = Self::default();
        for target in targets {
            let id = format!("{}/{}", target.host_id, target.runtime_id);
            if !target.provider_compatible {
                plan.incompatible.push(id.clone());
            }
            if !target.online {
                plan.offline.push(id.clone());
            }
            if !target.credential_unlocked {
                plan.locked.push(id.clone());
            }
            if target.active_turns > 0 {
                plan.busy.push(id.clone());
            }
            if !target.managed {
                plan.unmanaged.push(id.clone());
            }
            if target.provider_compatible
                && target.online
                && target.credential_unlocked
                && target.managed
                && target.active_turns == 0
            {
                plan.ready.push(id);
            }
        }
        for group in [
            &mut plan.ready,
            &mut plan.incompatible,
            &mut plan.offline,
            &mut plan.locked,
            &mut plan.busy,
            &mut plan.unmanaged,
        ] {
            group.sort();
            group.dedup();
        }
        plan
    }
}

pub struct CommandLedger {
    executed_commands: HashMap<String, CommandRecord>,
    runtime_assignments: HashMap<String, String>,
    session_assignments: HashMap<(String, String), String>,
    rotation_pools: HashMap<String, RotationPool>,
    bulk_switch_operations: HashMap<String, BulkSwitchOperation>,
    command_audit: Vec<CommandAuditRecord>,
    conn: Option<rusqlite::Connection>,
}

pub fn validate_operation_transition(
    from: RemoteOpState,
    to: RemoteOpState,
) -> Result<(), CoreError> {
    use RemoteOpState::*;
    let valid = matches!(
        (from, to),
        (Created, Persisted)
            | (Persisted, Dispatched)
            | (Dispatched, SourceAcknowledged)
            | (Dispatched, Succeeded)
            | (Dispatched, Failed)
            | (Dispatched, Expired)
            | (Dispatched, Cancelled)
            | (Dispatched, OutcomeUnknown)
            | (Dispatched, ReconciliationRequired)
            | (SourceAcknowledged, Reconciled)
            | (SourceAcknowledged, Failed)
            | (SourceAcknowledged, OutcomeUnknown)
            | (Reconciled, Succeeded)
            | (Reconciled, Failed)
            | (OutcomeUnknown, ReconciliationRequired)
    );
    if valid {
        Ok(())
    } else {
        Err(CoreError::InvalidTransition {
            from: format!("{from:?}"),
            to: format!("{to:?}"),
        })
    }
}

#[derive(Clone)]
struct CommandRecord {
    state: RemoteOpState,
    result_json: String,
    fingerprint: String,
}

/// A redacted control-plane audit record. It deliberately excludes
/// command payloads, provider secrets, filesystem paths, and source error text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandAuditRecord {
    pub idempotency_key: String,
    pub command_id: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub outcome: String,
    pub completed_at_ms: i64,
}

impl Default for CommandLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandLedger {
    const SCHEMA_VERSION: u32 = 7;

    pub fn new() -> Self {
        Self {
            executed_commands: HashMap::new(),
            runtime_assignments: HashMap::new(),
            session_assignments: HashMap::new(),
            rotation_pools: HashMap::new(),
            bulk_switch_operations: HashMap::new(),
            command_audit: Vec::new(),
            conn: None,
        }
    }

    pub fn open_sqlite(path: impl AsRef<std::path::Path>) -> Result<Self, CoreError> {
        let path = path.as_ref();
        if path.exists() {
            let preflight = rusqlite::Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            let integrity: String =
                preflight.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            if integrity != "ok" {
                return Err(CoreError::IntegrityCheckFailed(integrity));
            }
            let schema_version: u32 =
                preflight.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            if schema_version > Self::SCHEMA_VERSION {
                return Err(CoreError::UnsupportedSchemaVersion {
                    found: schema_version,
                    supported: Self::SCHEMA_VERSION,
                });
            }
        }
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let schema_version: u32 =
            conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if schema_version > Self::SCHEMA_VERSION {
            return Err(CoreError::UnsupportedSchemaVersion {
                found: schema_version,
                supported: Self::SCHEMA_VERSION,
            });
        }
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let transaction = conn.unchecked_transaction()?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS command_ledger (
                idempotency_key TEXT PRIMARY KEY,
                state_code INTEGER NOT NULL,
                result_json TEXT NOT NULL,
                command_fingerprint TEXT NOT NULL DEFAULT '',
                updated_at_ms INTEGER NOT NULL
            )",
            [],
        )?;
        let has_fingerprint = {
            let mut stmt = transaction.prepare("PRAGMA table_info(command_ledger)")?;
            let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for column in columns {
                if column? == "command_fingerprint" {
                    found = true;
                }
            }
            found
        };
        if !has_fingerprint {
            transaction.execute(
                "ALTER TABLE command_ledger
                 ADD COLUMN command_fingerprint TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS runtime_assignments (
                runtime_id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                CHECK (length(trim(runtime_id)) > 0),
                CHECK (length(trim(profile_id)) > 0)
            )",
            [],
        )?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS rotation_pools (
                pool_id TEXT PRIMARY KEY,
                policy_json TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                CHECK (length(trim(pool_id)) > 0)
            )",
            [],
        )?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS session_assignments (
                runtime_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                PRIMARY KEY (runtime_id, session_id),
                CHECK (length(trim(runtime_id)) > 0),
                CHECK (length(trim(session_id)) > 0),
                CHECK (length(trim(profile_id)) > 0)
            )",
            [],
        )?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS bulk_switch_operations (
                operation_id TEXT PRIMARY KEY,
                operation_json TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                CHECK (length(trim(operation_id)) > 0)
            )",
            [],
        )?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS command_audit (
                idempotency_key TEXT PRIMARY KEY,
                command_id TEXT NOT NULL,
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT NOT NULL,
                outcome TEXT NOT NULL,
                completed_at_ms INTEGER NOT NULL,
                CHECK (length(trim(idempotency_key)) > 0),
                CHECK (length(trim(command_id)) > 0),
                CHECK (length(trim(actor)) > 0),
                CHECK (length(trim(action)) > 0),
                CHECK (length(trim(target)) > 0),
                CHECK (length(trim(outcome)) > 0)
            )",
            [],
        )?;
        transaction.pragma_update(None, "user_version", Self::SCHEMA_VERSION)?;
        transaction.commit()?;

        let mut ledger = Self {
            executed_commands: HashMap::new(),
            runtime_assignments: HashMap::new(),
            session_assignments: HashMap::new(),
            rotation_pools: HashMap::new(),
            bulk_switch_operations: HashMap::new(),
            command_audit: Vec::new(),
            conn: Some(conn),
        };
        ledger.load_from_db()?;
        Ok(ledger)
    }

    fn load_from_db(&mut self) -> Result<(), CoreError> {
        if let Some(ref conn) = self.conn {
            let mut stmt = conn.prepare(
                "SELECT idempotency_key, state_code, result_json, command_fingerprint
                 FROM command_ledger",
            )?;
            let rows = stmt.query_map([], |row| {
                let key: String = row.get(0)?;
                let state_code: i32 = row.get(1)?;
                let result_json: String = row.get(2)?;
                let fingerprint: String = row.get(3)?;
                Ok((key, state_code, result_json, fingerprint))
            })?;
            for r in rows {
                let (key, state_code, result_json, fingerprint) = r?;
                let state = RemoteOpState::try_from(state_code)
                    .map_err(|_| CoreError::InvalidStoredState(state_code))?;
                self.executed_commands.insert(
                    key,
                    CommandRecord {
                        state,
                        result_json,
                        fingerprint,
                    },
                );
            }
            drop(stmt);
            let mut assignment_stmt = conn.prepare(
                "SELECT runtime_id, profile_id
                 FROM runtime_assignments",
            )?;
            let assignments = assignment_stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for assignment in assignments {
                let (runtime_id, profile_id) = assignment?;
                self.runtime_assignments.insert(runtime_id, profile_id);
            }
            drop(assignment_stmt);
            let mut session_assignment_stmt = conn.prepare(
                "SELECT runtime_id, session_id, profile_id
                 FROM session_assignments",
            )?;
            let session_assignments = session_assignment_stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for assignment in session_assignments {
                let (runtime_id, session_id, profile_id) = assignment?;
                self.session_assignments
                    .insert((runtime_id, session_id), profile_id);
            }
            drop(session_assignment_stmt);
            let mut pool_stmt = conn.prepare(
                "SELECT pool_id, policy_json
                 FROM rotation_pools",
            )?;
            let pools = pool_stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for pool in pools {
                let (pool_id, policy_json) = pool?;
                let policy: RotationPool = serde_json::from_str(&policy_json)?;
                policy.validate()?;
                if policy.pool_id != pool_id {
                    return Err(CoreError::RotationPoolIdMismatch {
                        row: pool_id,
                        policy: policy.pool_id,
                    });
                }
                self.rotation_pools.insert(pool_id, policy);
            }
            drop(pool_stmt);
            let mut bulk_switch_stmt = conn.prepare(
                "SELECT operation_id, operation_json
                 FROM bulk_switch_operations",
            )?;
            let operations = bulk_switch_stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for operation in operations {
                let (operation_id, operation_json) = operation?;
                let operation: BulkSwitchOperation = serde_json::from_str(&operation_json)?;
                operation.validate()?;
                if operation.operation_id != operation_id {
                    return Err(CoreError::BulkSwitchOperationIdMismatch {
                        row: operation_id,
                        operation: operation.operation_id,
                    });
                }
                self.bulk_switch_operations.insert(operation_id, operation);
            }
            drop(bulk_switch_stmt);
            let mut audit_stmt = conn.prepare(
                "SELECT idempotency_key, command_id, actor, action, target, outcome, completed_at_ms
                 FROM command_audit
                 ORDER BY completed_at_ms ASC, idempotency_key ASC",
            )?;
            let audit_rows = audit_stmt.query_map([], |row| {
                Ok(CommandAuditRecord {
                    idempotency_key: row.get(0)?,
                    command_id: row.get(1)?,
                    actor: row.get(2)?,
                    action: row.get(3)?,
                    target: row.get(4)?,
                    outcome: row.get(5)?,
                    completed_at_ms: row.get(6)?,
                })
            })?;
            for audit in audit_rows {
                self.command_audit.push(audit?);
            }
        }
        Ok(())
    }

    /// Returns the prior result for a duplicate command, or reserves a new key
    /// in `Created` state before the caller performs any side effect.
    pub fn check_or_record(
        &mut self,
        idempotency_key: &str,
        deadline_ms: i64,
    ) -> Result<Option<(RemoteOpState, String)>, CoreError> {
        self.reserve_command_inner(idempotency_key, deadline_ms, "")
    }

    /// Reserves an idempotency key and binds it to an immutable command
    /// fingerprint before the caller performs a side effect. Reusing the key
    /// for different command bytes is a conflict, never a cache hit.
    pub fn reserve_command(
        &mut self,
        idempotency_key: &str,
        deadline_ms: i64,
        command_fingerprint: &str,
    ) -> Result<Option<(RemoteOpState, String)>, CoreError> {
        if command_fingerprint.is_empty() {
            return Err(CoreError::EmptyCommandFingerprint);
        }
        self.reserve_command_inner(idempotency_key, deadline_ms, command_fingerprint)
    }

    fn reserve_command_inner(
        &mut self,
        idempotency_key: &str,
        deadline_ms: i64,
        command_fingerprint: &str,
    ) -> Result<Option<(RemoteOpState, String)>, CoreError> {
        if idempotency_key.trim().is_empty() {
            return Err(CoreError::EmptyIdempotencyKey);
        }
        if let Some(prev) = self.executed_commands.get(idempotency_key) {
            if prev.fingerprint != command_fingerprint {
                return Err(CoreError::DuplicateCommand(idempotency_key.to_owned()));
            }
            Ok(Some((prev.state, prev.result_json.clone())))
        } else {
            let now = chrono::Utc::now().timestamp_millis();
            if deadline_ms > 0 && now > deadline_ms {
                return Err(CoreError::CommandExpired);
            }
            if let Some(ref conn) = self.conn {
                conn.execute(
                    "INSERT INTO command_ledger
                        (idempotency_key, state_code, result_json, command_fingerprint, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        idempotency_key,
                        RemoteOpState::Created as i32,
                        "",
                        command_fingerprint,
                        now
                    ],
                )?;
            }
            self.executed_commands.insert(
                idempotency_key.to_owned(),
                CommandRecord {
                    state: RemoteOpState::Created,
                    result_json: String::new(),
                    fingerprint: command_fingerprint.to_owned(),
                },
            );
            Ok(None)
        }
    }

    pub fn record_result(
        &mut self,
        idempotency_key: String,
        state: RemoteOpState,
        result_json: String,
    ) -> Result<(), CoreError> {
        let prior_state = self
            .executed_commands
            .get(&idempotency_key)
            .map(|record| record.state)
            .ok_or_else(|| CoreError::UnknownCommand(idempotency_key.clone()))?;
        validate_operation_transition(prior_state, state)?;
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(ref conn) = self.conn {
            let changed = conn.execute(
                "UPDATE command_ledger
                 SET state_code = ?2, result_json = ?3, updated_at_ms = ?4
                 WHERE idempotency_key = ?1 AND state_code = ?5",
                rusqlite::params![
                    &idempotency_key,
                    state as i32,
                    &result_json,
                    now,
                    prior_state as i32
                ],
            )?;
            if changed != 1 {
                return Err(CoreError::InvalidTransition {
                    from: format!("{prior_state:?}"),
                    to: format!("{state:?}"),
                });
            }
        }
        self.executed_commands
            .entry(idempotency_key)
            .and_modify(|record| {
                record.state = state;
                record.result_json = result_json;
            });
        Ok(())
    }

    pub fn lookup_command(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<(RemoteOpState, String)>, CoreError> {
        if idempotency_key.trim().is_empty() {
            return Err(CoreError::EmptyIdempotencyKey);
        }
        Ok(self
            .executed_commands
            .get(idempotency_key)
            .map(|record| (record.state, record.result_json.clone())))
    }

    pub fn set_runtime_assignment(
        &mut self,
        runtime_id: &str,
        profile_id: &str,
    ) -> Result<(), CoreError> {
        let runtime_id = runtime_id.trim();
        let profile_id = profile_id.trim();
        if runtime_id.is_empty() {
            return Err(CoreError::EmptyAssignmentField("runtime_id"));
        }
        if profile_id.is_empty() {
            return Err(CoreError::EmptyAssignmentField("profile_id"));
        }
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(ref conn) = self.conn {
            let transaction = conn.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO runtime_assignments (runtime_id, profile_id, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(runtime_id) DO UPDATE SET
                    profile_id = excluded.profile_id,
                    updated_at_ms = excluded.updated_at_ms",
                rusqlite::params![runtime_id, profile_id, now],
            )?;
            transaction.commit()?;
        }
        self.runtime_assignments
            .insert(runtime_id.to_owned(), profile_id.to_owned());
        Ok(())
    }

    pub fn ensure_runtime_assignment(
        &mut self,
        runtime_id: &str,
        default_profile_id: &str,
    ) -> Result<String, CoreError> {
        if let Some(existing) = self.runtime_assignments.get(runtime_id) {
            return Ok(existing.clone());
        }
        self.set_runtime_assignment(runtime_id, default_profile_id)?;
        Ok(default_profile_id.trim().to_owned())
    }

    pub fn runtime_assignment(&self, runtime_id: &str) -> Option<&str> {
        self.runtime_assignments
            .get(runtime_id)
            .map(String::as_str)
    }

    pub fn set_session_assignment(
        &mut self,
        runtime_id: &str,
        session_id: &str,
        profile_id: &str,
    ) -> Result<(), CoreError> {
        let runtime_id = runtime_id.trim();
        let session_id = session_id.trim();
        let profile_id = profile_id.trim();
        if runtime_id.is_empty() {
            return Err(CoreError::EmptyAssignmentField("runtime_id"));
        }
        if session_id.is_empty() {
            return Err(CoreError::EmptyAssignmentField("session_id"));
        }
        if profile_id.is_empty() {
            return Err(CoreError::EmptyAssignmentField("profile_id"));
        }
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(ref conn) = self.conn {
            let transaction = conn.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO session_assignments
                    (runtime_id, session_id, profile_id, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(runtime_id, session_id) DO NOTHING",
                rusqlite::params![runtime_id, session_id, profile_id, now],
            )?;
            let stored: String = transaction.query_row(
                "SELECT profile_id FROM session_assignments
                 WHERE runtime_id = ?1 AND session_id = ?2",
                rusqlite::params![runtime_id, session_id],
                |row| row.get(0),
            )?;
            if stored != profile_id {
                return Err(CoreError::SessionAssignmentConflict {
                    runtime_id: runtime_id.to_owned(),
                    session_id: session_id.to_owned(),
                    existing: stored,
                    requested: profile_id.to_owned(),
                });
            }
            transaction.commit()?;
        } else if let Some(existing) = self
            .session_assignments
            .get(&(runtime_id.to_owned(), session_id.to_owned()))
        {
            if existing != profile_id {
                return Err(CoreError::SessionAssignmentConflict {
                    runtime_id: runtime_id.to_owned(),
                    session_id: session_id.to_owned(),
                    existing: existing.clone(),
                    requested: profile_id.to_owned(),
                });
            }
        }
        self.session_assignments.insert(
            (runtime_id.to_owned(), session_id.to_owned()),
            profile_id.to_owned(),
        );
        Ok(())
    }

    pub fn session_assignment(&self, runtime_id: &str, session_id: &str) -> Option<&str> {
        self.session_assignments
            .get(&(runtime_id.to_owned(), session_id.to_owned()))
            .map(String::as_str)
    }

    pub fn session_assignments_for_runtime(
        &self,
        runtime_id: &str,
    ) -> Vec<(String, String)> {
        self.session_assignments
            .iter()
            .filter_map(|((assigned_runtime_id, session_id), profile_id)| {
                (assigned_runtime_id == runtime_id)
                    .then(|| (session_id.clone(), profile_id.clone()))
            })
            .collect()
    }

    pub fn upsert_rotation_pool(&mut self, pool: RotationPool) -> Result<(), CoreError> {
        pool.validate()?;
        let policy_json = serde_json::to_string(&pool)?;
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(ref conn) = self.conn {
            let transaction = conn.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO rotation_pools (pool_id, policy_json, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(pool_id) DO UPDATE SET
                    policy_json = excluded.policy_json,
                    updated_at_ms = excluded.updated_at_ms",
                rusqlite::params![&pool.pool_id, &policy_json, now],
            )?;
            transaction.commit()?;
        }
        self.rotation_pools.insert(pool.pool_id.clone(), pool);
        Ok(())
    }

    pub fn rotation_pool(&self, pool_id: &str) -> Option<&RotationPool> {
        self.rotation_pools.get(pool_id)
    }

    /// Returns only non-secret pool policy metadata in a deterministic order.
    /// The profile references are safe to show to an authenticated client;
    /// vault material is deliberately not reachable from this API.
    pub fn rotation_pools(&self) -> Vec<&RotationPool> {
        let mut pools = self.rotation_pools.values().collect::<Vec<_>>();
        pools.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
        pools
    }

    /// Writes one redacted terminal command audit record. Repeated delivery of
    /// the same idempotency key is intentionally represented once.
    pub fn record_command_audit(
        &mut self,
        audit: CommandAuditRecord,
    ) -> Result<(), CoreError> {
        for (field, value) in [
            ("idempotency_key", audit.idempotency_key.as_str()),
            ("command_id", audit.command_id.as_str()),
            ("actor", audit.actor.as_str()),
            ("action", audit.action.as_str()),
            ("target", audit.target.as_str()),
            ("outcome", audit.outcome.as_str()),
        ] {
            if value.trim().is_empty() || value.len() > 256 {
                return Err(CoreError::InvalidAuditRecord(field));
            }
        }
        if let Some(ref conn) = self.conn {
            let transaction = conn.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO command_audit (
                    idempotency_key, command_id, actor, action, target, outcome, completed_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(idempotency_key) DO NOTHING",
                rusqlite::params![
                    &audit.idempotency_key,
                    &audit.command_id,
                    &audit.actor,
                    &audit.action,
                    &audit.target,
                    &audit.outcome,
                    audit.completed_at_ms,
                ],
            )?;
            transaction.commit()?;
        }
        if !self
            .command_audit
            .iter()
            .any(|existing| existing.idempotency_key == audit.idempotency_key)
        {
            self.command_audit.push(audit);
        }
        Ok(())
    }

    /// Provides only already-redacted records in deterministic completion order.
    pub fn command_audit_records(&self) -> &[CommandAuditRecord] {
        &self.command_audit
    }

    pub fn select_rotation(
        &mut self,
        pool_id: &str,
        request: RotationRequest<'_>,
        eligibility: &[RotationEligibility],
    ) -> Result<RotationDecision, CoreError> {
        let mut updated = self
            .rotation_pools
            .get(pool_id)
            .cloned()
            .ok_or_else(|| CoreError::RotationPoolNotFound(pool_id.to_owned()))?;
        let decision = updated.select(request, eligibility)?;
        self.upsert_rotation_pool(updated)?;
        Ok(decision)
    }

    /// Persists the full per-target result set before it is reported to a
    /// caller. The operation is intentionally resumable; it does not imply an
    /// all-or-nothing transaction across hosts.
    pub fn upsert_bulk_switch_operation(
        &mut self,
        operation: BulkSwitchOperation,
    ) -> Result<(), CoreError> {
        operation.validate()?;
        let operation_json = serde_json::to_string(&operation)?;
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(ref conn) = self.conn {
            let transaction = conn.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO bulk_switch_operations
                    (operation_id, operation_json, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(operation_id) DO UPDATE SET
                    operation_json = excluded.operation_json,
                    updated_at_ms = excluded.updated_at_ms",
                rusqlite::params![&operation.operation_id, &operation_json, now],
            )?;
            transaction.commit()?;
        }
        self.bulk_switch_operations
            .insert(operation.operation_id.clone(), operation);
        Ok(())
    }

    pub fn bulk_switch_operation(
        &self,
        operation_id: &str,
    ) -> Option<&BulkSwitchOperation> {
        self.bulk_switch_operations.get(operation_id)
    }

    pub fn transition_bulk_switch_target(
        &mut self,
        operation_id: &str,
        host_id: &str,
        runtime_id: &str,
        to: BulkSwitchTargetState,
        detail: Option<String>,
        completed_at_ms: Option<i64>,
    ) -> Result<(), CoreError> {
        let mut operation = self
            .bulk_switch_operations
            .get(operation_id)
            .cloned()
            .ok_or_else(|| CoreError::UnknownBulkSwitchOperation(operation_id.to_owned()))?;
        operation.transition_target(
            host_id,
            runtime_id,
            to,
            detail,
            completed_at_ms,
        )?;
        self.upsert_bulk_switch_operation(operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advance_to_dispatched(ledger: &mut CommandLedger, key: &str) {
        ledger
            .record_result(
                key.to_owned(),
                RemoteOpState::Persisted,
                String::new(),
            )
            .unwrap();
        ledger
            .record_result(
                key.to_owned(),
                RemoteOpState::Dispatched,
                String::new(),
            )
            .unwrap();
    }

    #[test]
    fn test_connector_state_transitions() {
        assert!(validate_connector_transition(ConnectorState::Starting, ConnectorState::Recovering).is_ok());
        assert!(validate_connector_transition(ConnectorState::Recovering, ConnectorState::Ready).is_ok());
        assert!(validate_connector_transition(ConnectorState::Ready, ConnectorState::Degraded).is_ok());
        assert!(validate_connector_transition(ConnectorState::FatalError, ConnectorState::Ready).is_err());
    }

    #[test]
    fn test_reconciliation() {
        let desired = DesiredRuntimeConfig {
            runtime_id: "opencode-1".into(),
            assigned_profile_id: "profile-b".into(),
            autostart: true,
            restart_on_crash: true,
        };
        let observed = ObservedRuntimeStatus {
            runtime_id: "opencode-1".into(),
            pid: Some(1234),
            current_state: RuntimeState::OnlineIdle,
            active_profile_id: "profile-a".into(),
        };

        let actions = DesiredObservedReconciler::reconcile(&desired, &observed);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], "switch_profile:opencode-1:profile-b");
    }

    #[test]
    fn assignment_resolution_uses_project_runtime_host_global_precedence() {
        let assignments = vec![
            CredentialAssignment {
                profile_id: "global".into(),
                scope: AssignmentScope::Global,
            },
            CredentialAssignment {
                profile_id: "host".into(),
                scope: AssignmentScope::Host {
                    host_id: "host-a".into(),
                },
            },
            CredentialAssignment {
                profile_id: "runtime".into(),
                scope: AssignmentScope::Runtime {
                    host_id: "host-a".into(),
                    runtime_id: "runtime-a".into(),
                },
            },
            CredentialAssignment {
                profile_id: "project".into(),
                scope: AssignmentScope::Project {
                    host_id: "host-a".into(),
                    runtime_id: "runtime-a".into(),
                    project_path: "/repo".into(),
                },
            },
        ];

        let resolved = resolve_assignment(
            &assignments,
            AssignmentTarget {
                host_id: "host-a",
                runtime_id: "runtime-a",
                project_path: Some("/repo"),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.profile_id, "project");
        assert!(matches!(resolved.scope, AssignmentScope::Project { .. }));

        let runtime = resolve_assignment(
            &assignments,
            AssignmentTarget {
                host_id: "host-a",
                runtime_id: "runtime-a",
                project_path: Some("/other"),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(runtime.profile_id, "runtime");
    }

    #[test]
    fn conflicting_assignments_at_one_precedence_fail_closed() {
        let assignments = vec![
            CredentialAssignment {
                profile_id: "profile-a".into(),
                scope: AssignmentScope::Runtime {
                    host_id: "host-a".into(),
                    runtime_id: "runtime-a".into(),
                },
            },
            CredentialAssignment {
                profile_id: "profile-b".into(),
                scope: AssignmentScope::Runtime {
                    host_id: "host-a".into(),
                    runtime_id: "runtime-a".into(),
                },
            },
        ];
        assert!(matches!(
            resolve_assignment(
                &assignments,
                AssignmentTarget {
                    host_id: "host-a",
                    runtime_id: "runtime-a",
                    project_path: None,
                }
            ),
            Err(CoreError::AmbiguousAssignment(2))
        ));
    }

    #[test]
    fn switch_impact_plan_separates_every_unsafe_dimension() {
        let plan = SwitchImpactPlan::build(&[
            SwitchImpactTarget {
                host_id: "host-a".into(),
                runtime_id: "ready".into(),
                provider_compatible: true,
                online: true,
                credential_unlocked: true,
                managed: true,
                active_turns: 0,
            },
            SwitchImpactTarget {
                host_id: "host-b".into(),
                runtime_id: "multi-risk".into(),
                provider_compatible: false,
                online: false,
                credential_unlocked: false,
                managed: false,
                active_turns: 2,
            },
        ]);
        assert_eq!(plan.ready, ["host-a/ready"]);
        assert_eq!(plan.incompatible, ["host-b/multi-risk"]);
        assert_eq!(plan.offline, ["host-b/multi-risk"]);
        assert_eq!(plan.locked, ["host-b/multi-risk"]);
        assert_eq!(plan.busy, ["host-b/multi-risk"]);
        assert_eq!(plan.unmanaged, ["host-b/multi-risk"]);
    }

    #[test]
    fn switch_impact_plan_keeps_same_named_runtimes_on_different_hosts_distinct() {
        let plan = SwitchImpactPlan::build(&[
            SwitchImpactTarget {
                host_id: "host-a".into(),
                runtime_id: "opencode".into(),
                provider_compatible: true,
                online: true,
                credential_unlocked: true,
                managed: true,
                active_turns: 0,
            },
            SwitchImpactTarget {
                host_id: "host-b".into(),
                runtime_id: "opencode".into(),
                provider_compatible: true,
                online: true,
                credential_unlocked: true,
                managed: true,
                active_turns: 0,
            },
        ]);
        assert_eq!(plan.ready, ["host-a/opencode", "host-b/opencode"]);
    }

    #[test]
    fn runtime_state_transitions_reject_impossible_jumps() {
        assert!(
            validate_runtime_transition(RuntimeState::Unknown, RuntimeState::Discovering)
                .is_ok()
        );
        assert!(
            validate_runtime_transition(RuntimeState::Synchronizing, RuntimeState::OnlineIdle)
                .is_ok()
        );
        assert!(
            validate_runtime_transition(RuntimeState::OnlineIdle, RuntimeState::CrashLoop)
                .is_err()
        );
        assert!(
            validate_runtime_transition(
                RuntimeState::CredentialLocked,
                RuntimeState::OnlineRunning
            )
            .is_err()
        );
    }

    #[test]
    fn command_ledger_reserves_before_execution_and_returns_prior_result() {
        let mut ledger = CommandLedger::new();

        assert_eq!(ledger.check_or_record("command-1", 0).unwrap(), None);
        assert_eq!(
            ledger.check_or_record("command-1", 0).unwrap(),
            Some((RemoteOpState::Created, String::new()))
        );

        advance_to_dispatched(&mut ledger, "command-1");
        ledger.record_result(
            "command-1".into(),
            RemoteOpState::Succeeded,
            r#"{"ok":true}"#.into(),
        )
        .unwrap();
        assert_eq!(
            ledger.check_or_record("command-1", 0).unwrap(),
            Some((RemoteOpState::Succeeded, r#"{"ok":true}"#.into()))
        );
    }

    #[test]
    fn command_ledger_rejects_expired_commands_without_reserving_them() {
        let mut ledger = CommandLedger::new();
        let expired = chrono::Utc::now().timestamp_millis() - 1;

        assert!(matches!(
            ledger.check_or_record("expired-command", expired),
            Err(CoreError::CommandExpired)
        ));
        assert_eq!(
            ledger.check_or_record("expired-command", 0).unwrap(),
            None
        );
    }

    #[test]
    fn command_ledger_lookup_is_read_only_and_validated() {
        let mut ledger = CommandLedger::new();
        assert_eq!(ledger.lookup_command("missing").unwrap(), None);
        assert!(matches!(
            ledger.lookup_command(""),
            Err(CoreError::EmptyIdempotencyKey)
        ));
        ledger
            .reserve_command("operation-1", 0, "fingerprint")
            .unwrap();
        advance_to_dispatched(&mut ledger, "operation-1");
        ledger
            .record_result(
                "operation-1".into(),
                RemoteOpState::Succeeded,
                r#"{"success":true}"#.into(),
            )
            .unwrap();
        assert_eq!(
            ledger.lookup_command("operation-1").unwrap(),
            Some((
                RemoteOpState::Succeeded,
                r#"{"success":true}"#.into()
            ))
        );
    }

    #[test]
    fn command_fingerprint_prevents_idempotency_key_reuse() {
        let mut ledger = CommandLedger::new();
        assert_eq!(
            ledger
                .reserve_command("command-1", 0, "fingerprint-a")
                .unwrap(),
            None
        );
        assert_eq!(
            ledger
                .reserve_command("command-1", 0, "fingerprint-a")
                .unwrap(),
            Some((RemoteOpState::Created, String::new()))
        );
        assert!(matches!(
            ledger.reserve_command("command-1", 0, "fingerprint-b"),
            Err(CoreError::DuplicateCommand(key)) if key == "command-1"
        ));
        assert!(matches!(
            ledger.reserve_command("command-2", 0, ""),
            Err(CoreError::EmptyCommandFingerprint)
        ));
    }

    #[test]
    fn completed_duplicate_is_returned_after_original_deadline() {
        let mut ledger = CommandLedger::new();
        ledger
            .reserve_command("command-1", 0, "fingerprint-a")
            .unwrap();
        advance_to_dispatched(&mut ledger, "command-1");
        ledger
            .record_result(
                "command-1".into(),
                RemoteOpState::Succeeded,
                r#"{"ok":true}"#.into(),
            )
            .unwrap();
        let expired = chrono::Utc::now().timestamp_millis() - 1;
        assert_eq!(
            ledger
                .reserve_command("command-1", expired, "fingerprint-a")
                .unwrap(),
            Some((
                RemoteOpState::Succeeded,
                r#"{"ok":true}"#.into()
            ))
        );
    }

    #[test]
    fn sqlite_command_ledger_persists_and_recovers_state() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("cmd_ledger_{}.db", std::process::id()));

        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            assert_eq!(ledger.check_or_record("cmd-persisted", 0).unwrap(), None);
            advance_to_dispatched(&mut ledger, "cmd-persisted");
            ledger.record_result(
                "cmd-persisted".into(),
                RemoteOpState::Succeeded,
                r#"{"persisted":true}"#.into(),
            )
            .unwrap();
        }

        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            assert_eq!(
                ledger.check_or_record("cmd-persisted", 0).unwrap(),
                Some((RemoteOpState::Succeeded, r#"{"persisted":true}"#.into()))
            );
        }

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn runtime_assignment_is_atomic_and_survives_restart() {
        let db_path = std::env::temp_dir().join(format!(
            "assignment-ledger-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            ledger
                .set_runtime_assignment("codex-work", "profile-work")
                .unwrap();
            assert_eq!(
                ledger.runtime_assignment("codex-work"),
                Some("profile-work")
            );
        }
        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            assert_eq!(
                ledger.runtime_assignment("codex-work"),
                Some("profile-work")
            );
            assert_eq!(
                ledger
                    .ensure_runtime_assignment("codex-work", "stale-manifest-default")
                    .unwrap(),
                "profile-work"
            );
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn bulk_switch_target_results_survive_restart_without_implicit_rollback() {
        let db_path = std::env::temp_dir().join(format!(
            "bulk-switch-ledger-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let operation = BulkSwitchOperation::new(
            "bulk-1",
            "profile-new",
            100,
            vec![
                BulkSwitchTarget {
                    host_id: "host-a".into(),
                    runtime_id: "runtime-a".into(),
                    prior_profile_id: "profile-old".into(),
                    state: BulkSwitchTargetState::Planned,
                    detail: None,
                    completed_at_ms: None,
                },
                BulkSwitchTarget {
                    host_id: "host-b".into(),
                    runtime_id: "runtime-b".into(),
                    prior_profile_id: "profile-old".into(),
                    state: BulkSwitchTargetState::Planned,
                    detail: None,
                    completed_at_ms: None,
                },
            ],
        )
        .unwrap();
        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            ledger.upsert_bulk_switch_operation(operation).unwrap();
            for (host_id, runtime_id, final_state, detail) in [
                (
                    "host-a",
                    "runtime-a",
                    BulkSwitchTargetState::Succeeded,
                    None,
                ),
                (
                    "host-b",
                    "runtime-b",
                    BulkSwitchTargetState::OutcomeUnknown,
                    Some("connection lost after dispatch".into()),
                ),
            ] {
                ledger
                    .transition_bulk_switch_target(
                        "bulk-1",
                        host_id,
                        runtime_id,
                        BulkSwitchTargetState::Dispatched,
                        None,
                        None,
                    )
                    .unwrap();
                ledger
                    .transition_bulk_switch_target(
                        "bulk-1",
                        host_id,
                        runtime_id,
                        final_state,
                        detail,
                        Some(200),
                    )
                    .unwrap();
            }
        }
        {
            let ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            let restored = ledger.bulk_switch_operation("bulk-1").unwrap();
            assert_eq!(restored.summary().succeeded, 1);
            assert_eq!(restored.summary().outcome_unknown, 1);
            assert!(restored.summary().is_complete());
            assert!(restored.summary().has_partial_failure());
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn rotation_policy_cursor_and_storm_history_survive_restart() {
        let db_path = std::env::temp_dir().join(format!(
            "rotation-ledger-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let pool = RotationPool {
            pool_id: "pool-a".into(),
            provider_id: "opencode-go".into(),
            ordered_profile_ids: vec!["profile-a".into(), "profile-b".into()],
            mode: RotationMode::RoundRobin,
            cooldown_ms: 500,
            max_switches_per_hour: 3,
            allowed_host_ids: vec!["host-a".into()],
            quota_failover_enabled: false,
            last_selection_cursor: None,
            last_selected_at_ms: None,
            recent_switches_ms: Vec::new(),
        };
        let candidates = vec![
            RotationEligibility {
                profile_id: "profile-a".into(),
                provider_id: "opencode-go".into(),
                eligible: true,
                cooldown_until_ms: None,
            },
            RotationEligibility {
                profile_id: "profile-b".into(),
                provider_id: "opencode-go".into(),
                eligible: true,
                cooldown_until_ms: None,
            },
        ];
        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            ledger.upsert_rotation_pool(pool).unwrap();
            let decision = ledger
                .select_rotation(
                    "pool-a",
                    RotationRequest {
                        host_id: "host-a",
                        current_profile_id: "profile-a",
                        now_ms: 1_000,
                        trigger: RotationTrigger::RoundRobin,
                    },
                    &candidates,
                )
                .unwrap();
            assert_eq!(decision.selected_profile_id, "profile-b");
        }
        {
            let ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            let restored = ledger.rotation_pool("pool-a").unwrap();
            assert_eq!(restored.last_selection_cursor, Some(1));
            assert_eq!(restored.last_selected_at_ms, Some(1_000));
            assert_eq!(restored.recent_switches_ms, [1_000]);
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn redacted_command_audit_survives_restart_and_deduplicates() {
        let db_path = std::env::temp_dir().join(format!(
            "command-audit-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let audit = CommandAuditRecord {
            idempotency_key: "audit-key-1".into(),
            command_id: "command-1".into(),
            actor: "device-public-id".into(),
            action: "change_assignment".into(),
            target: "runtime:codex-1".into(),
            outcome: "succeeded".into(),
            completed_at_ms: 123,
        };
        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            ledger.record_command_audit(audit.clone()).unwrap();
            ledger.record_command_audit(audit.clone()).unwrap();
            assert_eq!(ledger.command_audit_records(), &[audit.clone()]);
            assert!(matches!(
                ledger.record_command_audit(CommandAuditRecord {
                    target: "secret-value-that-must-not-fit".repeat(20),
                    ..audit.clone()
                }),
                Err(CoreError::InvalidAuditRecord("target"))
            ));
        }
        {
            let ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            assert_eq!(ledger.command_audit_records(), &[audit]);
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn session_credential_identity_survives_restart_and_cannot_be_relabelled() {
        let db_path = std::env::temp_dir().join(format!(
            "session-assignment-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            ledger
                .set_session_assignment("runtime-a", "session-1", "profile-a")
                .unwrap();
            assert!(matches!(
                ledger.set_session_assignment("runtime-a", "session-1", "profile-b"),
                Err(CoreError::SessionAssignmentConflict { .. })
            ));
        }
        {
            let ledger = CommandLedger::open_sqlite(&db_path).unwrap();
            assert_eq!(
                ledger.session_assignment("runtime-a", "session-1"),
                Some("profile-a")
            );
            assert_eq!(
                ledger.session_assignments_for_runtime("runtime-a"),
                vec![("session-1".into(), "profile-a".into())]
            );
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn sqlite_reservation_failure_is_reported_and_not_cached() {
        let db_path = std::env::temp_dir().join(format!(
            "cmd-ledger-failure-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
        ledger
            .conn
            .as_ref()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_command_insert
                 BEFORE INSERT ON command_ledger
                 BEGIN
                   SELECT RAISE(ABORT, 'injected persistence failure');
                 END;",
            )
            .unwrap();

        assert!(matches!(
            ledger.check_or_record("must-persist", 0),
            Err(CoreError::Sql(_))
        ));

        ledger
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("DROP TRIGGER reject_command_insert")
            .unwrap();
        assert_eq!(ledger.check_or_record("must-persist", 0).unwrap(), None);

        drop(ledger);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn result_requires_a_reserved_command() {
        let mut ledger = CommandLedger::new();
        assert!(matches!(
            ledger.record_result(
                "never-reserved".into(),
                RemoteOpState::Succeeded,
                "{}".into()
            ),
            Err(CoreError::UnknownCommand(key)) if key == "never-reserved"
        ));
    }

    #[test]
    fn impossible_operation_transition_is_rejected_without_mutation() {
        let mut ledger = CommandLedger::new();
        ledger
            .reserve_command("operation-1", 0, "fingerprint")
            .unwrap();

        assert!(matches!(
            ledger.record_result(
                "operation-1".into(),
                RemoteOpState::Succeeded,
                "{}".into()
            ),
            Err(CoreError::InvalidTransition { .. })
        ));
        assert_eq!(
            ledger.lookup_command("operation-1").unwrap(),
            Some((RemoteOpState::Created, String::new()))
        );
    }

    #[test]
    fn sqlite_open_migrates_legacy_command_ledger() {
        let db_path = std::env::temp_dir().join(format!(
            "cmd-ledger-migration-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE command_ledger (
                    idempotency_key TEXT PRIMARY KEY,
                    state_code INTEGER NOT NULL,
                    result_json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                INSERT INTO command_ledger
                    (idempotency_key, state_code, result_json, updated_at_ms)
                VALUES ('legacy', 6, '{\"legacy\":true}', 1);",
            )
            .unwrap();
        }

        let mut ledger = CommandLedger::open_sqlite(&db_path).unwrap();
        assert_eq!(
            ledger
                .conn
                .as_ref()
                .unwrap()
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            CommandLedger::SCHEMA_VERSION
        );
        assert_eq!(
            ledger.check_or_record("legacy", 0).unwrap(),
            Some((
                RemoteOpState::Succeeded,
                r#"{"legacy":true}"#.into()
            ))
        );
        assert_eq!(
            ledger
                .reserve_command("new-command", 0, "new-fingerprint")
                .unwrap(),
            None
        );

        drop(ledger);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn sqlite_rejects_future_schema_without_modifying_it() {
        let db_path = std::env::temp_dir().join(format!(
            "cmd-ledger-future-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "PRAGMA user_version = 999;
                 CREATE TABLE future_marker (value TEXT NOT NULL);
                 INSERT INTO future_marker VALUES ('preserve-me');",
            )
            .unwrap();
        }

        assert!(matches!(
            CommandLedger::open_sqlite(&db_path),
            Err(CoreError::UnsupportedSchemaVersion {
                found: 999,
                supported
            }) if supported == CommandLedger::SCHEMA_VERSION
        ));
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        assert_eq!(
            conn.query_row("SELECT value FROM future_marker", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "preserve-me"
        );
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            999
        );

        drop(conn);
        let _ = std::fs::remove_file(&db_path);
    }
}
