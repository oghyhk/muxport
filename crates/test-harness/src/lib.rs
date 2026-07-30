use adapter_api::{AdapterError, AgentAdapter, CapabilitySet, EventStream, ProjectInfo, SessionSummary};
use async_trait::async_trait;
use futures::stream;
use muxport_protocol::{AgentType, CredentialStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct DeterministicFakeAdapter {
    agent_type: AgentType,
    should_fail: Arc<AtomicBool>,
}

impl DeterministicFakeAdapter {
    pub fn new(agent_type: AgentType) -> Self {
        Self {
            agent_type,
            should_fail: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_fail_mode(&self, fail: bool) {
        self.should_fail.store(fail, Ordering::SeqCst);
    }
}

#[async_trait]
impl AgentAdapter for DeterministicFakeAdapter {
    fn agent_type(&self) -> AgentType {
        self.agent_type
    }

    async fn probe(&self) -> Result<CapabilitySet, AdapterError> {
        if self.should_fail.load(Ordering::SeqCst) {
            return Err(AdapterError::InitFailed("Injected fake probe failure".into()));
        }
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
            path: "/fake/repo".into(),
            name: "fake-repo".into(),
        }])
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError> {
        Ok(vec![SessionSummary {
            session_id: "fake-sess-1".into(),
            project_path: "/fake/repo".into(),
            title: "Fake Test Session".into(),
            status: "idle".into(),
            credential_profile_id: "fake-profile-1".into(),
            created_at_ms: 1700000000000,
        }])
    }

    async fn subscribe_events(&self) -> Result<EventStream, AdapterError> {
        Ok(Box::pin(stream::empty()))
    }

    async fn start_session(&self, _project_path: &str, _prompt: &str, _profile_id: &str) -> Result<String, AdapterError> {
        Ok("fake-sess-1".into())
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

    async fn respond_approval(
        &self,
        _session_id: &str,
        _approval_id: &str,
        _approved: bool,
        _reason: &str,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn validate_credential(&self, secret_payload: &str) -> Result<CredentialStatus, AdapterError> {
        if secret_payload == "invalid" {
            Ok(CredentialStatus::Invalid)
        } else {
            Ok(CredentialStatus::Active)
        }
    }

    async fn activate_credential(&self, _profile_id: &str, _secret_payload: &str) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn shutdown_gracefully(&self) -> Result<(), AdapterError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fake_adapter_failure_injection() {
        let adapter = DeterministicFakeAdapter::new(AgentType::Opencode);
        assert!(adapter.probe().await.is_ok());

        let adapter2 = DeterministicFakeAdapter::new(AgentType::Opencode);
        adapter2.set_fail_mode(true);
        assert!(adapter2.probe().await.is_err());
    }
}
