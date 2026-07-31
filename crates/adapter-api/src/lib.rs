use async_trait::async_trait;
use futures::Stream;
use muxport_protocol::{AgentType, CredentialStatus, Event};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::pin::Pin;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Adapter initialization error: {0}")]
    InitFailed(String),
    #[error("Connection lost to agent runtime")]
    ConnectionLost,
    #[error("Connection lost to agent runtime: {0}")]
    ConnectionLostWithDetail(String),
    #[error("Approval target not found: {0}")]
    ApprovalNotFound(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Credential validation failed: {0}")]
    CredentialInvalid(String),
    #[error("Invalid adapter input: {0}")]
    InvalidInput(String),
    #[error("Agent runtime protocol error: {0}")]
    Protocol(String),
    #[error("Agent runtime operation outcome is unknown: {0}")]
    OutcomeUnknown(String),
    #[error("Operation unsupported by runtime: {0}")]
    Unsupported(String),
    #[error("Internal adapter error: {0}")]
    Internal(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    pub can_stream_deltas: bool,
    pub can_approve_commands: bool,
    pub can_approve_edits: bool,
    pub can_interrupt: bool,
    pub can_switch_credentials_live: bool,
    pub can_read_usage: bool,
}

pub const ADAPTER_CAPABILITY_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterHealth {
    Healthy,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AdapterProbe {
    pub executable_version: String,
    pub source_api_version: String,
    pub capability_version: u32,
    pub capabilities: CapabilitySet,
    pub health: AdapterHealth,
}

const MAX_DIAGNOSTIC_EVENT_TYPE_CHARS: usize = 128;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityDiagnostic {
    pub source_event_type: String,
    pub observed_at_ms: i64,
}

impl CompatibilityDiagnostic {
    pub fn unknown_event(source_event_type: &str, observed_at_ms: i64) -> Self {
        let source_event_type = source_event_type.trim();
        let safe = !source_event_type.is_empty()
            && source_event_type.len() <= MAX_DIAGNOSTIC_EVENT_TYPE_CHARS
            && source_event_type
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || b"./_-".contains(&value));
        Self {
            source_event_type: if safe {
                source_event_type.to_owned()
            } else {
                "<invalid-event-type>".into()
            },
            observed_at_ms,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub project_path: String,
    pub title: String,
    pub status: String,
    pub credential_profile_id: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialKind {
    ApiKey,
}

/// Secret material passed to a built-in adapter for one scoped operation.
///
/// The secret is zeroized on drop, cannot be serialized, and its `Debug`
/// implementation is deliberately redacted.
pub struct CredentialMaterial {
    provider_id: String,
    kind: CredentialKind,
    secret: Zeroizing<Vec<u8>>,
}

impl CredentialMaterial {
    pub fn api_key(
        provider_id: impl Into<String>,
        secret: impl AsRef<[u8]>,
    ) -> Result<Self, AdapterError> {
        let provider_id = provider_id.into();
        if provider_id.trim().is_empty() {
            return Err(AdapterError::InvalidInput(
                "credential provider id must not be empty".into(),
            ));
        }
        let secret = secret.as_ref();
        if secret.is_empty() {
            return Err(AdapterError::InvalidInput(
                "credential secret must not be empty".into(),
            ));
        }
        Ok(Self {
            provider_id,
            kind: CredentialKind::ApiKey,
            secret: Zeroizing::new(secret.to_vec()),
        })
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    pub fn secret_utf8(&self) -> Result<&str, AdapterError> {
        std::str::from_utf8(self.secret.as_slice()).map_err(|_| {
            AdapterError::InvalidInput("credential secret must be valid UTF-8".into())
        })
    }
}

impl fmt::Debug for CredentialMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialMaterial")
            .field("provider_id", &self.provider_id)
            .field("kind", &self.kind)
            .field("secret", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialValidation {
    pub status: CredentialStatus,
    pub provider_id: String,
    pub account_fingerprint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountState {
    pub provider_id: String,
    pub connected: bool,
    pub account_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageWindow {
    pub used_percent: i32,
    pub resets_at_unix_seconds: Option<i64>,
    pub duration_minutes: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageBucket {
    pub bucket_id: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
}

/// Provider-independent usage data safe to project to an untrusted client.
///
/// Adapters must discard unknown provider fields rather than forwarding raw
/// payloads, and must bound the number and size of returned buckets.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageSnapshot {
    pub provider_id: String,
    pub observed_at_ms: i64,
    pub buckets: Vec<UsageBucket>,
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, AdapterError>> + Send>>;

#[async_trait]
pub trait AgentAdapter: Send + Sync {
    fn agent_type(&self) -> AgentType;
    async fn probe(&self) -> Result<AdapterProbe, AdapterError>;
    async fn discover_projects(&self) -> Result<Vec<ProjectInfo>, AdapterError>;
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError>;
    async fn read_session(&self, session_id: &str) -> Result<SessionSummary, AdapterError>;
    async fn subscribe_events(&self) -> Result<EventStream, AdapterError>;
    async fn start_session(&self, project_path: &str, prompt: &str, profile_id: &str) -> Result<String, AdapterError>;
    async fn send_input(&self, session_id: &str, text: &str) -> Result<(), AdapterError>;
    async fn steer(&self, session_id: &str, instruction: &str) -> Result<(), AdapterError>;
    async fn interrupt(&self, session_id: &str, reason: &str) -> Result<(), AdapterError>;
    async fn respond_approval(
        &self,
        session_id: &str,
        approval_id: &str,
        approved: bool,
        reason: &str,
    ) -> Result<(), AdapterError>;
    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<CredentialValidation, AdapterError>;
    async fn activate_credential(
        &self,
        profile_id: &str,
        credential: &CredentialMaterial,
    ) -> Result<(), AdapterError>;
    async fn read_account_state(&self, provider_id: &str) -> Result<AccountState, AdapterError>;
    async fn read_usage(&self) -> Result<UsageSnapshot, AdapterError>;
    fn compatibility_diagnostics(&self) -> Result<Vec<CompatibilityDiagnostic>, AdapterError>;
    async fn shutdown_gracefully(&self) -> Result<(), AdapterError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_diagnostic_never_retains_untrusted_payload_text() {
        let safe = CompatibilityDiagnostic::unknown_event("thread/new-kind", 10);
        assert_eq!(safe.source_event_type, "thread/new-kind");

        let unsafe_value =
            CompatibilityDiagnostic::unknown_event("new-kind\nsecret=bearer-value", 11);
        assert_eq!(unsafe_value.source_event_type, "<invalid-event-type>");
        assert!(!format!("{unsafe_value:?}").contains("bearer-value"));
    }
}
