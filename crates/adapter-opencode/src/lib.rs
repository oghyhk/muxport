use adapter_api::{
    AdapterError, AgentAdapter, CapabilitySet, EventStream, ProjectInfo, SessionSummary,
};
use async_trait::async_trait;
use muxport_protocol::{AgentType, CredentialStatus};
use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::Duration;

/// Read-only OpenCode discovery backed by the documented server API.
///
/// Mutating and streaming methods intentionally fail closed until they are
/// implemented against a generated client for the running OpenCode version.
pub struct OpenCodeAdapter {
    base_url: String,
    server_password: Option<String>,
    client: Client,
}

#[derive(Deserialize)]
struct HealthResponse {
    healthy: bool,
    #[allow(dead_code)]
    version: String,
}

#[derive(Deserialize)]
struct OpenCodeProject {
    id: Option<String>,
    name: Option<String>,
    worktree: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct OpenCodeSession {
    id: String,
    title: Option<String>,
    time: Option<OpenCodeSessionTime>,
}

#[derive(Deserialize)]
struct OpenCodeSessionTime {
    created: Option<i64>,
}

impl OpenCodeAdapter {
    pub fn new(base_url: impl Into<String>, server_password: Option<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            server_password,
            client: Client::new(),
        }
    }

    fn authenticated(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.server_password {
            Some(password) => request.basic_auth("opencode", Some(password)),
            None => request,
        }
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, AdapterError> {
        let request = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .timeout(Duration::from_secs(5));
        self.authenticated(request)
            .send()
            .await
            .map_err(|error| AdapterError::ConnectionLostWithDetail(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdapterError::ConnectionLostWithDetail(error.to_string()))?
            .json()
            .await
            .map_err(|error| AdapterError::Internal(format!("invalid OpenCode response: {error}")))
    }

    fn unsupported(operation: &str) -> AdapterError {
        AdapterError::Unsupported(format!(
            "OpenCode {operation} is not implemented by this connector build"
        ))
    }
}

#[async_trait]
impl AgentAdapter for OpenCodeAdapter {
    fn agent_type(&self) -> AgentType {
        AgentType::Opencode
    }

    async fn probe(&self) -> Result<CapabilitySet, AdapterError> {
        let health: HealthResponse = self.get_json("/global/health").await?;
        if !health.healthy {
            return Err(AdapterError::InitFailed(
                "OpenCode health endpoint reported unhealthy".into(),
            ));
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
        let projects: Vec<OpenCodeProject> = self.get_json("/project").await?;
        projects
            .into_iter()
            .map(|project| {
                let path = project
                    .worktree
                    .or(project.path)
                    .ok_or_else(|| AdapterError::Internal("OpenCode project has no path".into()))?;
                let name = project
                    .name
                    .or(project.id)
                    .unwrap_or_else(|| path.clone());
                Ok(ProjectInfo { path, name })
            })
            .collect()
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError> {
        let sessions: Vec<OpenCodeSession> = self.get_json("/session").await?;
        Ok(sessions
            .into_iter()
            .map(|session| SessionSummary {
                session_id: session.id,
                title: session.title.unwrap_or_else(|| "Untitled session".into()),
                status: "unknown".into(),
                credential_profile_id: String::new(),
                created_at_ms: session
                    .time
                    .and_then(|time| time.created)
                    .unwrap_or_default(),
            })
            .collect())
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

    #[test]
    fn parses_documented_project_and_session_shapes() {
        let project: OpenCodeProject = serde_json::from_str(
            r#"{"id":"project-1","name":"Muxport","worktree":"/srv/muxport"}"#,
        )
        .unwrap();
        assert_eq!(project.worktree.as_deref(), Some("/srv/muxport"));

        let session: OpenCodeSession = serde_json::from_str(
            r#"{"id":"session-1","title":"Review","time":{"created":1234}}"#,
        )
        .unwrap();
        assert_eq!(session.time.unwrap().created, Some(1234));
    }

    #[tokio::test]
    async fn mutating_operations_fail_closed() {
        let adapter = OpenCodeAdapter::new("http://127.0.0.1:9", None);
        assert!(matches!(
            adapter.send_input("session", "hello").await,
            Err(AdapterError::Unsupported(_))
        ));
        assert!(matches!(
            adapter.activate_credential("profile", "secret").await,
            Err(AdapterError::Unsupported(_))
        ));
    }
}
