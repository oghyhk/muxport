use adapter_api::{AdapterError, AgentAdapter, CapabilitySet, EventStream, ProjectInfo, SessionSummary};
use async_trait::async_trait;
use futures::stream;
use muxport_protocol::{
    event::Inner, AgentType, CredentialStatus, Event, SessionUpdatedEvent,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct OpenCodeAdapter {
    base_url: String,
    auth_token: Option<String>,
    client: Client,
    active_profile_id: Arc<Mutex<String>>,
}

impl OpenCodeAdapter {
    pub fn new(base_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self {
            base_url: base_url.into(),
            auth_token,
            client: Client::new(),
            active_profile_id: Arc::new(Mutex::new("default".to_string())),
        }
    }
}

#[async_trait]
impl AgentAdapter for OpenCodeAdapter {
    fn agent_type(&self) -> AgentType {
        AgentType::Opencode
    }

    async fn probe(&self) -> Result<CapabilitySet, AdapterError> {
        Ok(CapabilitySet {
            can_stream_deltas: true,
            can_approve_commands: true,
            can_approve_edits: true,
            can_interrupt: true,
            can_switch_credentials_live: true,
            can_read_usage: true,
        })
    }

    async fn discover_projects(&self) -> Result<Vec<ProjectInfo>, AdapterError> {
        Ok(vec![ProjectInfo {
            path: "/workspace".into(),
            name: "default-workspace".into(),
        }])
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError> {
        let profile = self.active_profile_id.lock().await.clone();
        Ok(vec![SessionSummary {
            session_id: "opencode-sess-1".into(),
            title: "OpenCode Refactoring Task".into(),
            status: "idle".into(),
            credential_profile_id: profile,
            created_at_ms: 1700000000000,
        }])
    }

    async fn subscribe_events(&self) -> Result<EventStream, AdapterError> {
        let events = vec![
            Ok(Event {
                event_id: "evt-opencode-1".into(),
                timestamp_ms: 1700000000001,
                inner: Some(Inner::SessionUpdated(SessionUpdatedEvent {
                    session_id: "opencode-sess-1".into(),
                    runtime_id: "opencode-runtime".into(),
                    title: "OpenCode Refactoring Task".into(),
                    status: "running".into(),
                    credential_profile_id: "opencode-go-profile-1".into(),
                    updated_at_ms: 1700000000001,
                })),
            }),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    async fn start_session(&self, _project_path: &str, _prompt: &str, _profile_id: &str) -> Result<String, AdapterError> {
        let sess_id = format!("opencode-sess-{}", uuid::Uuid::new_v4());
        Ok(sess_id)
    }

    async fn send_input(&self, _session_id: &str, _text: &str) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn steer(&self, _session_id: &str, _instruction: &str) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn interrupt(&self, _session_id: &str, _reason: &str) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn respond_approval(&self, _approval_id: &str, _approved: bool, _reason: &str) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn validate_credential(&self, secret_payload: &str) -> Result<CredentialStatus, AdapterError> {
        if secret_payload.is_empty() || secret_payload.contains("invalid") {
            Ok(CredentialStatus::Invalid)
        } else {
            Ok(CredentialStatus::Active)
        }
    }

    async fn activate_credential(&self, profile_id: &str, _secret_payload: &str) -> Result<(), AdapterError> {
        let mut prof = self.active_profile_id.lock().await;
        *prof = profile_id.to_string();
        Ok(())
    }

    async fn shutdown_gracefully(&self) -> Result<(), AdapterError> {
        Ok(())
    }
}
