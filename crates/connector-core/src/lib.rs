mod credential_switch;

pub use credential_switch::{
    activate_staged_credential, CredentialRuntime, CredentialSwitchError,
    CredentialSwitchResult,
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
    #[error("Command ledger contains an invalid operation state: {0}")]
    InvalidStoredState(i32),
    #[error("Command ledger integrity check failed: {0}")]
    IntegrityCheckFailed(String),
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
        (Discovering, Starting) | (Discovering, Degraded) => true,
        (Starting, Synchronizing) | (Starting, Crashed) => true,
        (Synchronizing, OnlineIdle) | (Synchronizing, OnlineRunning) => true,
        (OnlineIdle, OnlineRunning) | (OnlineIdle, OnlineWaitingApproval) | (OnlineIdle, Stopped) => true,
        (OnlineRunning, OnlineIdle) | (OnlineRunning, OnlineWaitingApproval) | (OnlineRunning, Crashed) => true,
        (OnlineWaitingApproval, OnlineRunning) | (OnlineWaitingApproval, OnlineIdle) => true,
        (Crashed, Starting) | (Crashed, CrashLoop) => true,
        (CrashLoop, Starting) | (CrashLoop, Stopped) => true,
        (Stopped, Starting) => true,
        (Degraded, Discovering) | (Degraded, Starting) => true,
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

pub struct CommandLedger {
    executed_commands: HashMap<String, CommandRecord>,
    conn: Option<rusqlite::Connection>,
}

#[derive(Clone)]
struct CommandRecord {
    state: RemoteOpState,
    result_json: String,
    fingerprint: String,
}

impl Default for CommandLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandLedger {
    pub fn new() -> Self {
        Self {
            executed_commands: HashMap::new(),
            conn: None,
        }
    }

    pub fn open_sqlite(path: impl AsRef<std::path::Path>) -> Result<Self, CoreError> {
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let integrity: String =
            conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(CoreError::IntegrityCheckFailed(integrity));
        }
        conn.execute(
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
            let mut stmt = conn.prepare("PRAGMA table_info(command_ledger)")?;
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
            conn.execute(
                "ALTER TABLE command_ledger
                 ADD COLUMN command_fingerprint TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }

        let mut ledger = Self {
            executed_commands: HashMap::new(),
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
        if !self.executed_commands.contains_key(&idempotency_key) {
            return Err(CoreError::UnknownCommand(idempotency_key));
        }
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(ref conn) = self.conn {
            let changed = conn.execute(
                "UPDATE command_ledger SET state_code = ?2, result_json = ?3, updated_at_ms = ?4 WHERE idempotency_key = ?1",
                rusqlite::params![&idempotency_key, state as i32, &result_json, now],
            )?;
            if changed != 1 {
                return Err(CoreError::UnknownCommand(idempotency_key));
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn command_ledger_reserves_before_execution_and_returns_prior_result() {
        let mut ledger = CommandLedger::new();

        assert_eq!(ledger.check_or_record("command-1", 0).unwrap(), None);
        assert_eq!(
            ledger.check_or_record("command-1", 0).unwrap(),
            Some((RemoteOpState::Created, String::new()))
        );

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
}
