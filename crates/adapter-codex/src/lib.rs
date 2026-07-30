use adapter_api::{
    AdapterError, AgentAdapter, CapabilitySet, EventStream, ProjectInfo, SessionSummary,
};
use async_trait::async_trait;
use muxport_protocol::{AgentType, CredentialStatus};
use tokio::process::Command;

/// Conservative Codex App Server adapter scaffold.
///
/// `probe` verifies that the configured CLI exposes `app-server`; all protocol
/// operations fail closed until the generated, version-matched JSON-RPC schema
/// is wired into this crate.
pub struct CodexAdapter {
    cmd_path: String,
}

impl CodexAdapter {
    pub fn new(cmd_path: impl Into<String>) -> Self {
        Self {
            cmd_path: cmd_path.into(),
        }
    }

    fn unsupported(operation: &str) -> AdapterError {
        AdapterError::Unsupported(format!(
            "Codex App Server {operation} is not implemented by this connector build"
        ))
    }
}

#[async_trait]
impl AgentAdapter for CodexAdapter {
    fn agent_type(&self) -> AgentType {
        AgentType::Codex
    }

    async fn probe(&self) -> Result<CapabilitySet, AdapterError> {
        let output = Command::new(&self.cmd_path)
            .args(["app-server", "--help"])
            .output()
            .await
            .map_err(|error| AdapterError::InitFailed(error.to_string()))?;
        if !output.status.success() {
            return Err(AdapterError::InitFailed(format!(
                "`{} app-server --help` exited with {}",
                self.cmd_path, output.status
            )));
        }

        Ok(CapabilitySet {
            can_stream_deltas: false,
            can_approve_commands: false,
            can_approve_edits: false,
            can_interrupt: false,
            can_switch_credentials_live: false,
            can_read_usage: false,
        })
    }

    async fn discover_projects(&self) -> Result<Vec<ProjectInfo>, AdapterError> {
        Err(Self::unsupported("project discovery"))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError> {
        Err(Self::unsupported("session listing"))
    }

    async fn subscribe_events(&self) -> Result<EventStream, AdapterError> {
        Err(Self::unsupported("event streaming"))
    }

    async fn start_session(
        &self,
        _project_path: &str,
        _prompt: &str,
        _profile_id: &str,
    ) -> Result<String, AdapterError> {
        Err(Self::unsupported("session creation"))
    }

    async fn send_input(&self, _session_id: &str, _text: &str) -> Result<(), AdapterError> {
        Err(Self::unsupported("send input"))
    }

    async fn steer(&self, _session_id: &str, _instruction: &str) -> Result<(), AdapterError> {
        Err(Self::unsupported("steering"))
    }

    async fn interrupt(&self, _session_id: &str, _reason: &str) -> Result<(), AdapterError> {
        Err(Self::unsupported("interrupt"))
    }

    async fn respond_approval(
        &self,
        _session_id: &str,
        _approval_id: &str,
        _approved: bool,
        _reason: &str,
    ) -> Result<(), AdapterError> {
        Err(Self::unsupported("approval response"))
    }

    async fn validate_credential(
        &self,
        _secret_payload: &str,
    ) -> Result<CredentialStatus, AdapterError> {
        Err(Self::unsupported("credential validation"))
    }

    async fn activate_credential(
        &self,
        _profile_id: &str,
        _secret_payload: &str,
    ) -> Result<(), AdapterError> {
        Err(Self::unsupported("credential activation"))
    }

    async fn shutdown_gracefully(&self) -> Result<(), AdapterError> {
        Err(Self::unsupported("graceful shutdown"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn protocol_operations_fail_closed() {
        let adapter = CodexAdapter::new("codex");
        assert!(matches!(
            adapter.list_sessions().await,
            Err(AdapterError::Unsupported(_))
        ));
        assert!(matches!(
            adapter.activate_credential("profile", "secret").await,
            Err(AdapterError::Unsupported(_))
        ));
    }
}
