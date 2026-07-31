use adapter_api::{
    AccountState, AdapterError, AgentAdapter, CapabilitySet, CredentialMaterial,
    CredentialValidation, EventStream, ProjectInfo, SessionSummary, UsageBucket, UsageSnapshot,
    UsageWindow,
};
use async_trait::async_trait;
use futures::stream;
use muxport_protocol::{AgentType, CredentialStatus};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

pub struct DeterministicFakeAdapter {
    agent_type: AgentType,
    should_fail: Arc<AtomicBool>,
    start_session_count: Arc<AtomicUsize>,
    start_session_delay_ms: Arc<AtomicU64>,
    start_session_outcome_unknown: Arc<AtomicBool>,
}

impl DeterministicFakeAdapter {
    pub fn new(agent_type: AgentType) -> Self {
        Self {
            agent_type,
            should_fail: Arc::new(AtomicBool::new(false)),
            start_session_count: Arc::new(AtomicUsize::new(0)),
            start_session_delay_ms: Arc::new(AtomicU64::new(0)),
            start_session_outcome_unknown: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_fail_mode(&self, fail: bool) {
        self.should_fail.store(fail, Ordering::SeqCst);
    }

    pub fn start_session_count(&self) -> usize {
        self.start_session_count.load(Ordering::SeqCst)
    }

    pub fn set_start_session_delay(&self, delay: std::time::Duration) {
        let delay_ms = delay.as_millis().try_into().unwrap_or(u64::MAX);
        self.start_session_delay_ms
            .store(delay_ms, Ordering::SeqCst);
    }

    pub fn set_start_session_outcome_unknown(&self, enabled: bool) {
        self.start_session_outcome_unknown
            .store(enabled, Ordering::SeqCst);
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

    async fn read_session(&self, session_id: &str) -> Result<SessionSummary, AdapterError> {
        self.list_sessions()
            .await?
            .into_iter()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| AdapterError::SessionNotFound(session_id.to_owned()))
    }

    async fn subscribe_events(&self) -> Result<EventStream, AdapterError> {
        Ok(Box::pin(stream::empty()))
    }

    async fn start_session(&self, _project_path: &str, _prompt: &str, _profile_id: &str) -> Result<String, AdapterError> {
        self.start_session_count.fetch_add(1, Ordering::SeqCst);
        let delay_ms = self.start_session_delay_ms.load(Ordering::SeqCst);
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        if self.start_session_outcome_unknown.load(Ordering::SeqCst) {
            return Err(AdapterError::OutcomeUnknown(
                "injected fake unknown outcome".into(),
            ));
        }
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

    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<CredentialValidation, AdapterError> {
        let status = if credential.secret_utf8()? == "invalid" {
            CredentialStatus::Invalid
        } else {
            CredentialStatus::Active
        };
        Ok(CredentialValidation {
            status,
            provider_id: credential.provider_id().to_owned(),
            account_fingerprint: None,
        })
    }

    async fn activate_credential(
        &self,
        _profile_id: &str,
        _credential: &CredentialMaterial,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn read_account_state(&self, provider_id: &str) -> Result<AccountState, AdapterError> {
        Ok(AccountState {
            provider_id: provider_id.to_owned(),
            connected: true,
            account_fingerprint: None,
        })
    }

    async fn read_usage(&self) -> Result<UsageSnapshot, AdapterError> {
        Ok(UsageSnapshot {
            provider_id: "fake".into(),
            observed_at_ms: 1700000000000,
            buckets: vec![UsageBucket {
                bucket_id: Some("fake".into()),
                primary: Some(UsageWindow {
                    used_percent: 50,
                    resets_at_unix_seconds: None,
                    duration_minutes: None,
                }),
                secondary: None,
            }],
        })
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

    #[tokio::test]
    async fn fake_adapter_reads_one_authoritative_session_or_not_found() {
        let adapter = DeterministicFakeAdapter::new(AgentType::Codex);
        let session = adapter.read_session("fake-sess-1").await.unwrap();
        assert_eq!(session.project_path, "/fake/repo");
        let usage = adapter.read_usage().await.unwrap();
        assert_eq!(usage.provider_id, "fake");
        assert_eq!(usage.buckets[0].primary.as_ref().unwrap().used_percent, 50);
        assert!(matches!(
            adapter.read_session("missing").await,
            Err(AdapterError::SessionNotFound(id)) if id == "missing"
        ));
    }
}
