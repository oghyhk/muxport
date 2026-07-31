pub use adapter_api::ProviderFailureClass;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

const ONE_HOUR_MS: i64 = 60 * 60 * 1000;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RotationMode {
    Manual,
    RoundRobin,
    Scheduled,
    ConfirmedFailure,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RotationTrigger {
    Manual,
    RoundRobin,
    Scheduled,
    ConfirmedFailure(ProviderFailureClass),
}

impl RotationTrigger {
    pub fn is_automatic(self) -> bool {
        !matches!(self, Self::Manual)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RotationPool {
    pub pool_id: String,
    pub provider_id: String,
    pub ordered_profile_ids: Vec<String>,
    pub mode: RotationMode,
    pub cooldown_ms: i64,
    pub max_switches_per_hour: u32,
    pub allowed_host_ids: Vec<String>,
    pub quota_failover_enabled: bool,
    pub last_selection_cursor: Option<usize>,
    pub last_selected_at_ms: Option<i64>,
    pub recent_switches_ms: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationEligibility {
    pub profile_id: String,
    pub provider_id: String,
    pub eligible: bool,
    pub cooldown_until_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationRequest<'a> {
    pub host_id: &'a str,
    pub current_profile_id: &'a str,
    pub now_ms: i64,
    pub trigger: RotationTrigger,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RotationDecision {
    pub pool_id: String,
    pub selected_profile_id: String,
    pub selected_cursor: usize,
    pub automatic: bool,
    pub reason_code: String,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum RotationError {
    #[error("rotation pool field {0} must not be empty")]
    EmptyField(&'static str),
    #[error("rotation pool must contain at least two unique credential profiles")]
    InsufficientProfiles,
    #[error("rotation pool cooldown must not be negative")]
    InvalidCooldown,
    #[error("rotation pool max switches per hour must be positive")]
    InvalidHourlyLimit,
    #[error("host is not allowed by this rotation pool")]
    HostRestricted,
    #[error("rotation trigger is incompatible with pool mode")]
    TriggerNotAllowed,
    #[error("automatic failover is forbidden for this failure class")]
    UnsafeFailureClass,
    #[error("quota-triggered rotation is feature-gated for this pool")]
    QuotaFeatureDisabled,
    #[error("rotation pool cooldown has not elapsed")]
    PoolCoolingDown,
    #[error("rotation pool hourly switch limit has been reached")]
    HourlyLimitReached,
    #[error("no compatible eligible credential profile is available")]
    NoEligibleProfile,
}

impl RotationPool {
    pub fn validate(&self) -> Result<(), RotationError> {
        if self.pool_id.trim().is_empty() {
            return Err(RotationError::EmptyField("pool_id"));
        }
        if self.provider_id.trim().is_empty() {
            return Err(RotationError::EmptyField("provider_id"));
        }
        if self.cooldown_ms < 0 {
            return Err(RotationError::InvalidCooldown);
        }
        if self.max_switches_per_hour == 0 {
            return Err(RotationError::InvalidHourlyLimit);
        }
        let profiles = self
            .ordered_profile_ids
            .iter()
            .filter(|profile| !profile.trim().is_empty())
            .collect::<HashSet<_>>();
        if profiles.len() != self.ordered_profile_ids.len() || profiles.len() < 2 {
            return Err(RotationError::InsufficientProfiles);
        }
        if self
            .allowed_host_ids
            .iter()
            .any(|host| host.trim().is_empty())
        {
            return Err(RotationError::EmptyField("allowed_host_id"));
        }
        if self
            .last_selection_cursor
            .is_some_and(|cursor| cursor >= self.ordered_profile_ids.len())
        {
            return Err(RotationError::NoEligibleProfile);
        }
        Ok(())
    }

    pub fn select(
        &mut self,
        request: RotationRequest<'_>,
        eligibility: &[RotationEligibility],
    ) -> Result<RotationDecision, RotationError> {
        self.validate()?;
        if !self.allowed_host_ids.is_empty()
            && !self
                .allowed_host_ids
                .iter()
                .any(|host| host == request.host_id)
        {
            return Err(RotationError::HostRestricted);
        }
        self.validate_trigger(request.trigger)?;
        if self
            .last_selected_at_ms
            .is_some_and(|last| request.now_ms.saturating_sub(last) < self.cooldown_ms)
        {
            return Err(RotationError::PoolCoolingDown);
        }
        self.recent_switches_ms.retain(|timestamp| {
            *timestamp <= request.now_ms
                && request.now_ms.saturating_sub(*timestamp) < ONE_HOUR_MS
        });
        if self.recent_switches_ms.len() >= self.max_switches_per_hour as usize {
            return Err(RotationError::HourlyLimitReached);
        }

        let start = self
            .last_selection_cursor
            .map(|cursor| (cursor + 1) % self.ordered_profile_ids.len())
            .unwrap_or(0);
        let selected = (0..self.ordered_profile_ids.len())
            .map(|offset| (start + offset) % self.ordered_profile_ids.len())
            .find(|cursor| {
                let profile_id = &self.ordered_profile_ids[*cursor];
                profile_id != request.current_profile_id
                    && eligibility.iter().any(|candidate| {
                        candidate.profile_id == *profile_id
                            && candidate.provider_id == self.provider_id
                            && candidate.eligible
                            && candidate
                                .cooldown_until_ms
                                .is_none_or(|until| until <= request.now_ms)
                    })
            })
            .ok_or(RotationError::NoEligibleProfile)?;

        self.last_selection_cursor = Some(selected);
        self.last_selected_at_ms = Some(request.now_ms);
        self.recent_switches_ms.push(request.now_ms);
        Ok(RotationDecision {
            pool_id: self.pool_id.clone(),
            selected_profile_id: self.ordered_profile_ids[selected].clone(),
            selected_cursor: selected,
            automatic: request.trigger.is_automatic(),
            reason_code: trigger_reason(request.trigger).to_owned(),
        })
    }

    fn validate_trigger(&self, trigger: RotationTrigger) -> Result<(), RotationError> {
        if matches!(trigger, RotationTrigger::Manual) {
            return Ok(());
        }
        let mode_matches = matches!(
            (self.mode, trigger),
            (RotationMode::RoundRobin, RotationTrigger::RoundRobin)
                | (RotationMode::Scheduled, RotationTrigger::Scheduled)
                | (
                    RotationMode::ConfirmedFailure,
                    RotationTrigger::ConfirmedFailure(_)
                )
        );
        if !mode_matches {
            return Err(RotationError::TriggerNotAllowed);
        }
        if let RotationTrigger::ConfirmedFailure(failure) = trigger {
            match failure {
                ProviderFailureClass::Authentication
                | ProviderFailureClass::Permission
                | ProviderFailureClass::RateLimit => {}
                ProviderFailureClass::Quota if self.quota_failover_enabled => {}
                ProviderFailureClass::Quota => {
                    return Err(RotationError::QuotaFeatureDisabled);
                }
                ProviderFailureClass::Network
                | ProviderFailureClass::RuntimeCrash
                | ProviderFailureClass::MalformedResponse
                | ProviderFailureClass::ConnectorRestart
                | ProviderFailureClass::Unknown => {
                    return Err(RotationError::UnsafeFailureClass);
                }
            }
        }
        Ok(())
    }
}

fn trigger_reason(trigger: RotationTrigger) -> &'static str {
    match trigger {
        RotationTrigger::Manual => "manual",
        RotationTrigger::RoundRobin => "round_robin",
        RotationTrigger::Scheduled => "scheduled",
        RotationTrigger::ConfirmedFailure(ProviderFailureClass::Authentication) => {
            "confirmed_authentication_failure"
        }
        RotationTrigger::ConfirmedFailure(ProviderFailureClass::Permission) => {
            "confirmed_permission_failure"
        }
        RotationTrigger::ConfirmedFailure(ProviderFailureClass::RateLimit) => {
            "confirmed_rate_limit"
        }
        RotationTrigger::ConfirmedFailure(ProviderFailureClass::Quota) => "confirmed_quota",
        RotationTrigger::ConfirmedFailure(_) => "forbidden_failure_class",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(mode: RotationMode) -> RotationPool {
        RotationPool {
            pool_id: "go-pool".into(),
            provider_id: "opencode-go".into(),
            ordered_profile_ids: vec!["account-a".into(), "account-b".into(), "account-c".into()],
            mode,
            cooldown_ms: 1_000,
            max_switches_per_hour: 2,
            allowed_host_ids: vec!["host-a".into()],
            quota_failover_enabled: false,
            last_selection_cursor: None,
            last_selected_at_ms: None,
            recent_switches_ms: Vec::new(),
        }
    }

    fn eligible() -> Vec<RotationEligibility> {
        ["account-a", "account-b", "account-c"]
            .into_iter()
            .map(|profile_id| RotationEligibility {
                profile_id: profile_id.into(),
                provider_id: "opencode-go".into(),
                eligible: true,
                cooldown_until_ms: None,
            })
            .collect()
    }

    #[test]
    fn round_robin_is_ordered_and_skips_current_profile() {
        let mut pool = pool(RotationMode::RoundRobin);
        let first = pool
            .select(
                RotationRequest {
                    host_id: "host-a",
                    current_profile_id: "account-a",
                    now_ms: 1_000,
                    trigger: RotationTrigger::RoundRobin,
                },
                &eligible(),
            )
            .unwrap();
        assert_eq!(first.selected_profile_id, "account-b");
        let second = pool
            .select(
                RotationRequest {
                    host_id: "host-a",
                    current_profile_id: "account-b",
                    now_ms: 2_000,
                    trigger: RotationTrigger::RoundRobin,
                },
                &eligible(),
            )
            .unwrap();
        assert_eq!(second.selected_profile_id, "account-c");
    }

    #[test]
    fn cooldown_and_hourly_limit_prevent_rotation_storms() {
        let mut pool = pool(RotationMode::RoundRobin);
        pool.select(
            RotationRequest {
                host_id: "host-a",
                current_profile_id: "account-a",
                now_ms: 1_000,
                trigger: RotationTrigger::RoundRobin,
            },
            &eligible(),
        )
        .unwrap();
        assert_eq!(
            pool.select(
                RotationRequest {
                    host_id: "host-a",
                    current_profile_id: "account-b",
                    now_ms: 1_500,
                    trigger: RotationTrigger::RoundRobin,
                },
                &eligible(),
            ),
            Err(RotationError::PoolCoolingDown)
        );
        pool.select(
            RotationRequest {
                host_id: "host-a",
                current_profile_id: "account-b",
                now_ms: 2_000,
                trigger: RotationTrigger::RoundRobin,
            },
            &eligible(),
        )
        .unwrap();
        assert_eq!(
            pool.select(
                RotationRequest {
                    host_id: "host-a",
                    current_profile_id: "account-c",
                    now_ms: 3_000,
                    trigger: RotationTrigger::RoundRobin,
                },
                &eligible(),
            ),
            Err(RotationError::HourlyLimitReached)
        );
    }

    #[test]
    fn automatic_failure_rotation_accepts_only_typed_safe_signals() {
        for failure in [
            ProviderFailureClass::Network,
            ProviderFailureClass::RuntimeCrash,
            ProviderFailureClass::MalformedResponse,
            ProviderFailureClass::ConnectorRestart,
            ProviderFailureClass::Unknown,
        ] {
            let mut pool = pool(RotationMode::ConfirmedFailure);
            assert_eq!(
                pool.select(
                    RotationRequest {
                        host_id: "host-a",
                        current_profile_id: "account-a",
                        now_ms: 1_000,
                        trigger: RotationTrigger::ConfirmedFailure(failure),
                    },
                    &eligible(),
                ),
                Err(RotationError::UnsafeFailureClass)
            );
        }
        let mut quota = pool(RotationMode::ConfirmedFailure);
        assert_eq!(
            quota.select(
                RotationRequest {
                    host_id: "host-a",
                    current_profile_id: "account-a",
                    now_ms: 1_000,
                    trigger: RotationTrigger::ConfirmedFailure(ProviderFailureClass::Quota),
                },
                &eligible(),
            ),
            Err(RotationError::QuotaFeatureDisabled)
        );
    }

    #[test]
    fn host_restrictions_and_profile_eligibility_fail_closed() {
        let mut restricted = pool(RotationMode::Manual);
        assert_eq!(
            restricted.select(
                RotationRequest {
                    host_id: "host-b",
                    current_profile_id: "account-a",
                    now_ms: 1_000,
                    trigger: RotationTrigger::Manual,
                },
                &eligible(),
            ),
            Err(RotationError::HostRestricted)
        );
        let mut no_candidate = pool(RotationMode::Manual);
        let candidates = eligible()
            .into_iter()
            .map(|mut candidate| {
                candidate.eligible = candidate.profile_id == "account-a";
                candidate
            })
            .collect::<Vec<_>>();
        assert_eq!(
            no_candidate.select(
                RotationRequest {
                    host_id: "host-a",
                    current_profile_id: "account-a",
                    now_ms: 1_000,
                    trigger: RotationTrigger::Manual,
                },
                &candidates,
            ),
            Err(RotationError::NoEligibleProfile)
        );
    }
}
