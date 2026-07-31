use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// A durable, host-scoped credential assignment change. A bulk operation is
/// deliberately a collection of independent mutations: a successful target
/// is never rolled back merely because another host is unavailable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkSwitchOperation {
    pub operation_id: String,
    pub target_profile_id: String,
    pub created_at_ms: i64,
    pub targets: Vec<BulkSwitchTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkSwitchTarget {
    pub host_id: String,
    pub runtime_id: String,
    pub prior_profile_id: String,
    pub state: BulkSwitchTargetState,
    /// Redacted, user-actionable result text. It must not contain a secret or
    /// provider response body.
    pub detail: Option<String>,
    pub completed_at_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BulkSwitchTargetState {
    Planned,
    Dispatched,
    Succeeded,
    Failed,
    OutcomeUnknown,
}

impl BulkSwitchTargetState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::OutcomeUnknown
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BulkSwitchSummary {
    pub planned: usize,
    pub dispatched: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub outcome_unknown: usize,
}

impl BulkSwitchSummary {
    pub fn is_complete(self) -> bool {
        self.planned == 0 && self.dispatched == 0
    }

    pub fn has_partial_failure(self) -> bool {
        self.failed > 0 || self.outcome_unknown > 0
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BulkSwitchError {
    #[error("bulk switch operation ID must not be empty")]
    EmptyOperationId,
    #[error("bulk switch target profile ID must not be empty")]
    EmptyTargetProfileId,
    #[error("bulk switch requires at least one target")]
    NoTargets,
    #[error("bulk switch target has an empty {0}")]
    EmptyTargetField(&'static str),
    #[error("bulk switch contains duplicate target {host_id}/{runtime_id}")]
    DuplicateTarget { host_id: String, runtime_id: String },
    #[error("bulk switch target does not exist: {host_id}/{runtime_id}")]
    TargetNotFound { host_id: String, runtime_id: String },
    #[error("bulk switch target {host_id}/{runtime_id} cannot transition from {from:?} to {to:?}")]
    InvalidTransition {
        host_id: String,
        runtime_id: String,
        from: BulkSwitchTargetState,
        to: BulkSwitchTargetState,
    },
}

impl BulkSwitchOperation {
    pub fn new(
        operation_id: impl Into<String>,
        target_profile_id: impl Into<String>,
        created_at_ms: i64,
        targets: Vec<BulkSwitchTarget>,
    ) -> Result<Self, BulkSwitchError> {
        let operation_id = operation_id.into();
        let target_profile_id = target_profile_id.into();
        if operation_id.trim().is_empty() {
            return Err(BulkSwitchError::EmptyOperationId);
        }
        if target_profile_id.trim().is_empty() {
            return Err(BulkSwitchError::EmptyTargetProfileId);
        }
        if targets.is_empty() {
            return Err(BulkSwitchError::NoTargets);
        }

        let mut target_keys = HashSet::new();
        for target in &targets {
            for (name, value) in [
                ("host_id", &target.host_id),
                ("runtime_id", &target.runtime_id),
                ("prior_profile_id", &target.prior_profile_id),
            ] {
                if value.trim().is_empty() {
                    return Err(BulkSwitchError::EmptyTargetField(name));
                }
            }
            let key = format!("{}/{}", target.host_id, target.runtime_id);
            if !target_keys.insert(key) {
                return Err(BulkSwitchError::DuplicateTarget {
                    host_id: target.host_id.clone(),
                    runtime_id: target.runtime_id.clone(),
                });
            }
        }

        Ok(Self {
            operation_id,
            target_profile_id,
            created_at_ms,
            targets,
        })
    }

    pub fn summary(&self) -> BulkSwitchSummary {
        let mut summary = BulkSwitchSummary {
            planned: 0,
            dispatched: 0,
            succeeded: 0,
            failed: 0,
            outcome_unknown: 0,
        };
        for target in &self.targets {
            match target.state {
                BulkSwitchTargetState::Planned => summary.planned += 1,
                BulkSwitchTargetState::Dispatched => summary.dispatched += 1,
                BulkSwitchTargetState::Succeeded => summary.succeeded += 1,
                BulkSwitchTargetState::Failed => summary.failed += 1,
                BulkSwitchTargetState::OutcomeUnknown => summary.outcome_unknown += 1,
            }
        }
        summary
    }

    pub fn transition_target(
        &mut self,
        host_id: &str,
        runtime_id: &str,
        to: BulkSwitchTargetState,
        detail: Option<String>,
        completed_at_ms: Option<i64>,
    ) -> Result<(), BulkSwitchError> {
        let target = self
            .targets
            .iter_mut()
            .find(|target| target.host_id == host_id && target.runtime_id == runtime_id)
            .ok_or_else(|| BulkSwitchError::TargetNotFound {
                host_id: host_id.to_owned(),
                runtime_id: runtime_id.to_owned(),
            })?;
        if !valid_transition(target.state, to) {
            return Err(BulkSwitchError::InvalidTransition {
                host_id: host_id.to_owned(),
                runtime_id: runtime_id.to_owned(),
                from: target.state,
                to,
            });
        }
        target.state = to;
        target.detail = detail;
        target.completed_at_ms = if to.is_terminal() {
            completed_at_ms
        } else {
            None
        };
        Ok(())
    }
}

fn valid_transition(from: BulkSwitchTargetState, to: BulkSwitchTargetState) -> bool {
    matches!(
        (from, to),
        (BulkSwitchTargetState::Planned, BulkSwitchTargetState::Dispatched)
            | (BulkSwitchTargetState::Dispatched, BulkSwitchTargetState::Succeeded)
            | (BulkSwitchTargetState::Dispatched, BulkSwitchTargetState::Failed)
            | (BulkSwitchTargetState::Dispatched, BulkSwitchTargetState::OutcomeUnknown)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(host_id: &str, runtime_id: &str) -> BulkSwitchTarget {
        BulkSwitchTarget {
            host_id: host_id.into(),
            runtime_id: runtime_id.into(),
            prior_profile_id: "profile-old".into(),
            state: BulkSwitchTargetState::Planned,
            detail: None,
            completed_at_ms: None,
        }
    }

    #[test]
    fn partial_bulk_switch_is_complete_but_never_claimed_fully_successful() {
        let mut operation = BulkSwitchOperation::new(
            "switch-1",
            "profile-new",
            100,
            vec![target("host-a", "runtime-a"), target("host-b", "runtime-b")],
        )
        .unwrap();
        for (host, runtime, result, detail) in [
            (
                "host-a",
                "runtime-a",
                BulkSwitchTargetState::Succeeded,
                None,
            ),
            (
                "host-b",
                "runtime-b",
                BulkSwitchTargetState::Failed,
                Some("host is offline".into()),
            ),
        ] {
            operation
                .transition_target(
                    host,
                    runtime,
                    BulkSwitchTargetState::Dispatched,
                    None,
                    None,
                )
                .unwrap();
            operation
                .transition_target(host, runtime, result, detail, Some(200))
                .unwrap();
        }

        assert_eq!(
            operation.summary(),
            BulkSwitchSummary {
                planned: 0,
                dispatched: 0,
                succeeded: 1,
                failed: 1,
                outcome_unknown: 0,
            }
        );
        assert!(operation.summary().is_complete());
        assert!(operation.summary().has_partial_failure());
    }

    #[test]
    fn an_unknown_outcome_is_terminal_and_cannot_be_overwritten() {
        let mut operation = BulkSwitchOperation::new(
            "switch-unknown",
            "profile-new",
            100,
            vec![target("host-a", "runtime-a")],
        )
        .unwrap();
        operation
            .transition_target(
                "host-a",
                "runtime-a",
                BulkSwitchTargetState::Dispatched,
                None,
                None,
            )
            .unwrap();
        operation
            .transition_target(
                "host-a",
                "runtime-a",
                BulkSwitchTargetState::OutcomeUnknown,
                Some("connection lost after dispatch".into()),
                Some(200),
            )
            .unwrap();
        assert!(matches!(
            operation.transition_target(
                "host-a",
                "runtime-a",
                BulkSwitchTargetState::Succeeded,
                None,
                Some(201),
            ),
            Err(BulkSwitchError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn duplicate_host_runtime_targets_are_rejected_during_impact_planning() {
        assert!(matches!(
            BulkSwitchOperation::new(
                "switch-duplicate",
                "profile-new",
                100,
                vec![target("host-a", "runtime-a"), target("host-a", "runtime-a")],
            ),
            Err(BulkSwitchError::DuplicateTarget { .. })
        ));
    }
}
