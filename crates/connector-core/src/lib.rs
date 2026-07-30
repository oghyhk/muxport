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
    executed_commands: HashMap<String, (RemoteOpState, String)>,
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
        }
    }

    /// Returns the prior result for a duplicate command, or reserves a new key
    /// in `Created` state before the caller performs any side effect.
    pub fn check_or_record(
        &mut self,
        idempotency_key: &str,
        deadline_ms: i64,
    ) -> Result<Option<(RemoteOpState, String)>, CoreError> {
        let now = chrono::Utc::now().timestamp_millis();
        if deadline_ms > 0 && now > deadline_ms {
            return Err(CoreError::CommandExpired);
        }

        if let Some(prev) = self.executed_commands.get(idempotency_key) {
            Ok(Some(prev.clone()))
        } else {
            self.executed_commands.insert(
                idempotency_key.to_owned(),
                (RemoteOpState::Created, String::new()),
            );
            Ok(None)
        }
    }

    pub fn record_result(
        &mut self,
        idempotency_key: String,
        state: RemoteOpState,
        result_json: String,
    ) {
        self.executed_commands
            .insert(idempotency_key, (state, result_json));
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
        );
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
}
