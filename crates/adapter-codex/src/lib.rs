use adapter_api::{AdapterError, AgentAdapter, CapabilitySet, EventStream, ProjectInfo, SessionSummary};
use async_trait::async_trait;
use futures::stream;
use muxport_protocol::{
    event::Inner, AgentType, CredentialStatus, Event, StreamDeltaEvent,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct CodexAdapter {
    cmd_path: String,
    active_profile_id: Arc<Mutex<String>>,
}

impl CodexAdapter {
    pub fn new(cmd_path: impl Into<String>) -> Self {
        Self {
            cmd_path: cmd_path.into(),
            active_profile_id: Arc::new(Mutex::new("default-codex-profile".to_string())),
        }
    }
}

#[async_trait]
impl AgentAdapter for CodexAdapter {
    fn agent_type(&self) -> AgentType {
        AgentType::Codex
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
            path: "/workspace/muxport".into(),
            name: "muxport".into(),
        }])
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError> {
        let profile = self.active_profile_id.lock().await.clone();
        Ok(vec![SessionSummary {
            session_id: "codex-thread-1".into(),
            title: "Codex Code Generation".into(),
            status: "idle".into(),
            credential_profile_id: profile,
            created_at_ms: 1700000000000,
        }])
    }

    async fn subscribe_events(&self) -> Result<EventStream, AdapterError> {
        let events = vec![
            Ok(Event {
                event_id: "evt-codex-1".into(),
                timestamp_ms: 1700000000002,
                inner: Some(Inner::StreamDelta(StreamDeltaEvent {
                    session_id: "codex-thread-1".into(),
                    turn_id: "turn-1".into(),
                    delta_text: "Processing prompt in Codex...".into(),
                    is_final: false,
                })),
            }),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    async fn start_session(&self, _project_path: &str, _prompt: &str, _profile_id: &str) -> Result<String, AdapterError> {
        let thread_id = format!("codex-thread-{}", uuid::Uuid::new_v4());
        Ok(thread_id)
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
