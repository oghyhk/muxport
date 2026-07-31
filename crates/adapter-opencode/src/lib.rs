mod managed;

pub use managed::{ManagedOpenCodeError, ManagedOpenCodeProfile};

use adapter_api::{
    AccountState, AdapterError, AdapterHealth, AdapterProbe, AgentAdapter, CapabilitySet,
    CompatibilityDiagnostic, CredentialKind, CredentialMaterial, CredentialValidation,
    EventStream, ProjectInfo, SessionSummary, UsageSnapshot, ADAPTER_CAPABILITY_VERSION,
};
use async_trait::async_trait;
use futures::StreamExt;
use muxport_protocol::{
    event, AgentType, ApprovalRequestedEvent, ApprovalResolvedEvent, CredentialStatus, Event,
    SessionUpdatedEvent, StreamDeltaEvent,
};
use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client, RequestBuilder, Response, StatusCode, Url,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const MUTATION_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_SSE_EVENT_BYTES: usize = 1024 * 1024;
const SSE_CHANNEL_CAPACITY: usize = 128;
const COMPATIBILITY_DIAGNOSTIC_CAPACITY: usize = 128;

/// OpenCode server adapter backed by the documented HTTP and global SSE APIs.
///
/// Credential mutation intentionally remains unavailable. Muxport must first
/// prove per-profile process/state isolation and rollback for the installed
/// OpenCode version; sending a secret to `/auth` alone would not provide that.
#[derive(Clone)]
pub struct OpenCodeAdapter {
    base_url: String,
    server_password: Option<String>,
    managed_profile_id: Option<String>,
    client: Client,
    observed_version: Arc<RwLock<Option<String>>>,
    sessions: Arc<RwLock<HashMap<String, SessionContext>>>,
    pending_approvals: Arc<RwLock<HashMap<String, PendingApproval>>>,
    compatibility_diagnostics: Arc<RwLock<VecDeque<CompatibilityDiagnostic>>>,
}

#[derive(Clone, Debug)]
struct SessionContext {
    directory: String,
    title: String,
}

#[derive(Clone, Debug)]
struct PendingApproval {
    session_id: String,
    directory: String,
}

#[derive(Deserialize)]
struct HealthResponse {
    healthy: bool,
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
    directory: Option<String>,
    time: Option<OpenCodeSessionTime>,
}

#[derive(Deserialize)]
struct OpenCodeSessionTime {
    created: Option<i64>,
    updated: Option<i64>,
}

#[derive(Deserialize)]
struct OpenCodeSessionStatus {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Serialize)]
struct PromptBody<'a> {
    parts: [TextPartInput<'a>; 1],
}

#[derive(Serialize)]
struct TextPartInput<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    text: &'a str,
}

#[derive(Serialize)]
struct ApprovalBody {
    response: &'static str,
}

#[derive(Deserialize)]
struct OpenCodeAuthMethod {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct OpenCodeProviderState {
    #[serde(default)]
    connected: Vec<String>,
}

#[derive(Serialize)]
struct ApiCredentialBody<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    key: &'a str,
}

impl OpenCodeAdapter {
    pub fn new(base_url: impl Into<String>, server_password: Option<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            server_password,
            managed_profile_id: None,
            client: Client::new(),
            observed_version: Arc::new(RwLock::new(None)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            compatibility_diagnostics: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Creates an adapter for a connector-owned isolated OpenCode runtime.
    ///
    /// Only managed adapters may mutate provider authentication. Adapters
    /// created with [`OpenCodeAdapter::new`] observe external runtimes and
    /// remain fail-closed for credential changes.
    pub fn new_managed(
        base_url: impl Into<String>,
        server_password: Option<String>,
        profile_id: impl Into<String>,
    ) -> Result<Self, AdapterError> {
        let profile_id = profile_id.into();
        Self::validate_nonempty(&profile_id, "managed profile id")?;
        let mut adapter = Self::new(base_url, server_password);
        adapter.managed_profile_id = Some(profile_id);
        Ok(adapter)
    }

    /// Returns the version from the most recent successful health probe.
    pub fn observed_version(&self) -> Result<Option<String>, AdapterError> {
        self.observed_version
            .read()
            .map(|version| version.clone())
            .map_err(|_| AdapterError::Internal("OpenCode version state lock was poisoned".into()))
    }

    fn endpoint(&self, segments: &[&str]) -> Result<Url, AdapterError> {
        let mut url = Url::parse(&format!("{}/", self.base_url)).map_err(|error| {
            AdapterError::InitFailed(format!("invalid OpenCode server URL: {error}"))
        })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AdapterError::InitFailed(
                "OpenCode server URL must use http or https".into(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(AdapterError::InitFailed(
                "OpenCode server credentials must not be embedded in the URL".into(),
            ));
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(AdapterError::InitFailed(
                "OpenCode server base URL must not contain a query or fragment".into(),
            ));
        }
        {
            let mut path = url.path_segments_mut().map_err(|_| {
                AdapterError::InitFailed("OpenCode server URL cannot contain path segments".into())
            })?;
            path.pop_if_empty();
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    fn authenticated(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.server_password {
            Some(password) => request.basic_auth("opencode", Some(password)),
            None => request,
        }
    }

    async fn send_read(
        &self,
        request: RequestBuilder,
        operation: &str,
    ) -> Result<Response, AdapterError> {
        let response = self
            .authenticated(request)
            .send()
            .await
            .map_err(|error| AdapterError::ConnectionLostWithDetail(error.to_string()))?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(AdapterError::Protocol(format!(
                "OpenCode {operation} returned HTTP {}",
                response.status()
            )))
        }
    }

    async fn send_mutation(
        &self,
        request: RequestBuilder,
        operation: &str,
    ) -> Result<Response, AdapterError> {
        let response = self.authenticated(request).send().await.map_err(|error| {
            AdapterError::OutcomeUnknown(format!(
                "OpenCode {operation} transport failed; reconcile before retrying: {error}"
            ))
        })?;
        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else if status.is_server_error() {
            Err(AdapterError::OutcomeUnknown(format!(
                "OpenCode {operation} returned HTTP {status}; reconcile before retrying"
            )))
        } else {
            Err(AdapterError::Protocol(format!(
                "OpenCode {operation} was rejected with HTTP {status}"
            )))
        }
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        operation: &str,
    ) -> Result<T, AdapterError> {
        let request = self
            .client
            .get(self.endpoint(segments)?)
            .timeout(READ_TIMEOUT);
        self.read_json_request(request, operation).await
    }

    async fn get_json_in_directory<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        operation: &str,
        directory: &str,
    ) -> Result<T, AdapterError> {
        let request = self
            .client
            .get(self.endpoint(segments)?)
            .query(&[("directory", directory)])
            .timeout(READ_TIMEOUT);
        self.read_json_request(request, operation).await
    }

    async fn read_json_request<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
        operation: &str,
    ) -> Result<T, AdapterError> {
        self.send_read(request, operation)
            .await?
            .json()
            .await
            .map_err(|error| {
                AdapterError::Protocol(format!(
                    "OpenCode {operation} returned invalid JSON: {error}"
                ))
            })
    }

    async fn known_project_directories(&self) -> Result<Vec<String>, AdapterError> {
        let mut directories = self
            .discover_projects()
            .await?
            .into_iter()
            .map(|project| project.path)
            .collect::<Vec<_>>();
        directories.sort();
        directories.dedup();
        if directories.is_empty() {
            directories.push(String::new());
        }
        Ok(directories)
    }

    async fn auth_methods(
        &self,
    ) -> Result<HashMap<String, Vec<OpenCodeAuthMethod>>, AdapterError> {
        self.get_json(&["provider", "auth"], "provider auth discovery")
            .await
    }

    async fn provider_state(&self) -> Result<OpenCodeProviderState, AdapterError> {
        self.get_json(&["provider"], "provider state readback").await
    }

    fn validate_nonempty(value: &str, field: &str) -> Result<(), AdapterError> {
        if value.trim().is_empty() {
            Err(AdapterError::InvalidInput(format!(
                "{field} must not be empty"
            )))
        } else {
            Ok(())
        }
    }

    fn unsupported(operation: &str) -> AdapterError {
        AdapterError::Unsupported(format!(
            "OpenCode {operation} is not implemented by this connector build"
        ))
    }

    fn bound_profile_id(&self) -> String {
        self.managed_profile_id.clone().unwrap_or_default()
    }

    fn remember_session(
        &self,
        session_id: &str,
        directory: &str,
        title: Option<&str>,
    ) -> Result<(), AdapterError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| AdapterError::Internal("OpenCode session state lock was poisoned".into()))?;
        let prior_title = sessions
            .get(session_id)
            .map(|session| session.title.clone())
            .unwrap_or_default();
        let prior_directory = sessions
            .get(session_id)
            .map(|session| session.directory.clone())
            .unwrap_or_default();
        sessions.insert(
            session_id.to_owned(),
            SessionContext {
                directory: if directory.is_empty() {
                    prior_directory
                } else {
                    directory.to_owned()
                },
                title: title.unwrap_or(&prior_title).to_owned(),
            },
        );
        Ok(())
    }

    fn session_context(&self, session_id: &str) -> Result<Option<SessionContext>, AdapterError> {
        self.sessions
            .read()
            .map(|sessions| sessions.get(session_id).cloned())
            .map_err(|_| AdapterError::Internal("OpenCode session state lock was poisoned".into()))
    }

    async fn resolve_session_context(
        &self,
        session_id: &str,
    ) -> Result<SessionContext, AdapterError> {
        if let Some(context) = self.session_context(session_id)? {
            if !context.directory.is_empty() {
                return Ok(context);
            }
        }

        for directory in self.known_project_directories().await? {
            let sessions: Vec<OpenCodeSession> = if directory.is_empty() {
                self.get_json(&["session"], "session reconciliation").await?
            } else {
                self.get_json_in_directory(
                    &["session"],
                    "session reconciliation",
                    &directory,
                )
                .await?
            };
            for session in sessions {
                let actual_directory = session
                    .directory
                    .clone()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| directory.clone());
                let title = session.title.as_deref().unwrap_or("Untitled session");
                self.remember_session(&session.id, &actual_directory, Some(title))?;
                if session.id == session_id {
                    return self.session_context(session_id)?.ok_or_else(|| {
                        AdapterError::Internal(
                            "reconciled OpenCode session was not cached".into(),
                        )
                    });
                }
            }
        }

        Err(AdapterError::Protocol(format!(
            "OpenCode session {session_id} was not found during reconciliation"
        )))
    }

    fn with_directory(
        &self,
        request: RequestBuilder,
        directory: Option<&str>,
    ) -> RequestBuilder {
        match directory {
            Some(directory) if !directory.is_empty() => {
                request.query(&[("directory", directory)])
            }
            _ => request,
        }
    }

    async fn send_prompt(
        &self,
        session_id: &str,
        text: &str,
        directory: Option<&str>,
    ) -> Result<(), AdapterError> {
        let body = PromptBody {
            parts: [TextPartInput { kind: "text", text }],
        };
        let request = self.client.post(self.endpoint(&[
            "session",
            session_id,
            "prompt_async",
        ])?);
        let request = self
            .with_directory(request, directory)
            .timeout(MUTATION_TIMEOUT)
            .json(&body);
        let response = self.send_mutation(request, "asynchronous prompt").await?;
        if response.status() == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            Err(AdapterError::OutcomeUnknown(format!(
                "OpenCode asynchronous prompt returned unexpected HTTP {}; reconcile session {session_id}",
                response.status()
            )))
        }
    }

    fn normalize_global_event(&self, data: &str) -> Result<Option<Event>, AdapterError> {
        let envelope: Value = serde_json::from_str(data).map_err(|error| {
            AdapterError::Protocol(format!("invalid OpenCode SSE JSON payload: {error}"))
        })?;
        let directory = envelope
            .get("directory")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let payload = envelope
            .get("payload")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                AdapterError::Protocol("OpenCode global event has no payload object".into())
            })?;
        let event_type = payload.get("type").and_then(Value::as_str).ok_or_else(|| {
            AdapterError::Protocol("OpenCode global event has no payload type".into())
        })?;
        let properties = payload.get("properties").unwrap_or(&Value::Null);

        match event_type {
            "session.created" | "session.updated" | "session.deleted" => {
                let info = properties.get("info").ok_or_else(|| {
                    AdapterError::Protocol(format!(
                        "OpenCode {event_type} event has no session info"
                    ))
                })?;
                let session_id = required_string(info, "id", event_type)?;
                let title = info
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Untitled session");
                let session_directory = info
                    .get("directory")
                    .and_then(Value::as_str)
                    .unwrap_or(directory);
                self.remember_session(session_id, session_directory, Some(title))?;
                let updated_at_ms = info
                    .get("time")
                    .and_then(|time| time.get("updated"))
                    .and_then(Value::as_i64)
                    .unwrap_or_else(now_ms);
                let status = if event_type == "session.deleted" {
                    "deleted"
                } else {
                    "unknown"
                };
                Ok(Some(new_event(
                    updated_at_ms,
                    event::Inner::SessionUpdated(SessionUpdatedEvent {
                        session_id: session_id.to_owned(),
                        runtime_id: String::new(),
                        title: title.to_owned(),
                        status: status.to_owned(),
                        credential_profile_id: self.bound_profile_id(),
                        updated_at_ms,
                        project_path: session_directory.to_owned(),
                    }),
                )))
            }
            "session.status" => {
                let session_id = required_string(properties, "sessionID", event_type)?;
                let status = properties
                    .get("status")
                    .and_then(|status| status.get("type"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AdapterError::Protocol(
                            "OpenCode session.status event has no status type".into(),
                        )
                    })?;
                self.remember_session(session_id, directory, None)?;
                let context = self
                    .session_context(session_id)?
                    .unwrap_or(SessionContext {
                        directory: directory.to_owned(),
                        title: String::new(),
                    });
                let timestamp = now_ms();
                Ok(Some(new_event(
                    timestamp,
                    event::Inner::SessionUpdated(SessionUpdatedEvent {
                        session_id: session_id.to_owned(),
                        runtime_id: String::new(),
                        title: context.title,
                        status: status.to_owned(),
                        credential_profile_id: self.bound_profile_id(),
                        updated_at_ms: timestamp,
                        project_path: context.directory,
                    }),
                )))
            }
            "session.idle" => {
                let session_id = required_string(properties, "sessionID", event_type)?;
                self.remember_session(session_id, directory, None)?;
                let context = self
                    .session_context(session_id)?
                    .unwrap_or(SessionContext {
                        directory: directory.to_owned(),
                        title: String::new(),
                    });
                let timestamp = now_ms();
                Ok(Some(new_event(
                    timestamp,
                    event::Inner::SessionUpdated(SessionUpdatedEvent {
                        session_id: session_id.to_owned(),
                        runtime_id: String::new(),
                        title: context.title,
                        status: "idle".into(),
                        credential_profile_id: self.bound_profile_id(),
                        updated_at_ms: timestamp,
                        project_path: context.directory,
                    }),
                )))
            }
            "message.part.updated" => {
                let delta = match properties.get("delta").and_then(Value::as_str) {
                    Some(delta) => delta,
                    None => return Ok(None),
                };
                let part = properties.get("part").ok_or_else(|| {
                    AdapterError::Protocol(
                        "OpenCode message.part.updated event has no part".into(),
                    )
                })?;
                if part.get("type").and_then(Value::as_str) != Some("text") {
                    return Ok(None);
                }
                let session_id = required_string(part, "sessionID", event_type)?;
                let message_id = required_string(part, "messageID", event_type)?;
                self.remember_session(session_id, directory, None)?;
                Ok(Some(new_event(
                    now_ms(),
                    event::Inner::StreamDelta(StreamDeltaEvent {
                        session_id: session_id.to_owned(),
                        turn_id: message_id.to_owned(),
                        delta_text: delta.to_owned(),
                        is_final: false,
                    }),
                )))
            }
            "message.updated" => {
                let info = properties.get("info").ok_or_else(|| {
                    AdapterError::Protocol("OpenCode message.updated event has no info".into())
                })?;
                let completed = info
                    .get("time")
                    .and_then(|time| time.get("completed"))
                    .and_then(Value::as_i64);
                if info.get("role").and_then(Value::as_str) != Some("assistant")
                    || completed.is_none()
                {
                    return Ok(None);
                }
                let session_id = required_string(info, "sessionID", event_type)?;
                let message_id = required_string(info, "id", event_type)?;
                self.remember_session(session_id, directory, None)?;
                Ok(Some(new_event(
                    completed.unwrap_or_else(now_ms),
                    event::Inner::StreamDelta(StreamDeltaEvent {
                        session_id: session_id.to_owned(),
                        turn_id: message_id.to_owned(),
                        delta_text: String::new(),
                        is_final: true,
                    }),
                )))
            }
            "permission.updated" => {
                let approval_id = required_string(properties, "id", event_type)?;
                let session_id = required_string(properties, "sessionID", event_type)?;
                let action_type = required_string(properties, "type", event_type)?;
                let description = required_string(properties, "title", event_type)?;
                let timestamp = properties
                    .get("time")
                    .and_then(|time| time.get("created"))
                    .and_then(Value::as_i64)
                    .unwrap_or_else(now_ms);
                self.remember_session(session_id, directory, None)?;
                self.pending_approvals
                    .write()
                    .map_err(|_| {
                        AdapterError::Internal(
                            "OpenCode approval state lock was poisoned".into(),
                        )
                    })?
                    .insert(
                        approval_id.to_owned(),
                        PendingApproval {
                            session_id: session_id.to_owned(),
                            directory: directory.to_owned(),
                        },
                    );
                let payload_json = serde_json::to_string(&json!({
                    "messageID": properties.get("messageID"),
                    "callID": properties.get("callID"),
                    "pattern": properties.get("pattern"),
                }))
                .map_err(|error| {
                    AdapterError::Internal(format!(
                        "failed to normalize OpenCode permission: {error}"
                    ))
                })?;
                Ok(Some(new_event(
                    timestamp,
                    event::Inner::ApprovalRequested(ApprovalRequestedEvent {
                        approval_id: approval_id.to_owned(),
                        session_id: session_id.to_owned(),
                        action_type: action_type.to_owned(),
                        description: description.to_owned(),
                        payload_json,
                    }),
                )))
            }
            "permission.replied" => {
                let approval_id = required_string(properties, "permissionID", event_type)?;
                let session_id = required_string(properties, "sessionID", event_type)?;
                let response = required_string(properties, "response", event_type)?;
                self.pending_approvals
                    .write()
                    .map_err(|_| {
                        AdapterError::Internal(
                            "OpenCode approval state lock was poisoned".into(),
                        )
                    })?
                    .remove(approval_id);
                Ok(Some(new_event(
                    now_ms(),
                    event::Inner::ApprovalResolved(ApprovalResolvedEvent {
                        approval_id: approval_id.to_owned(),
                        approved: response != "reject",
                        resolved_by: "opencode".into(),
                        session_id: session_id.to_owned(),
                    }),
                )))
            }
            _ => {
                let mut diagnostics = self.compatibility_diagnostics.write().map_err(|_| {
                    AdapterError::Internal(
                        "OpenCode compatibility diagnostic lock was poisoned".into(),
                    )
                })?;
                if diagnostics.len() == COMPATIBILITY_DIAGNOSTIC_CAPACITY {
                    diagnostics.pop_front();
                }
                diagnostics.push_back(CompatibilityDiagnostic::unknown_event(
                    event_type,
                    now_ms(),
                ));
                Ok(None)
            }
        }
    }
}

#[async_trait]
impl AgentAdapter for OpenCodeAdapter {
    fn agent_type(&self) -> AgentType {
        AgentType::Opencode
    }

    async fn probe(&self) -> Result<AdapterProbe, AdapterError> {
        let health: HealthResponse = self
            .get_json(&["global", "health"], "health probe")
            .await?;
        if !health.healthy {
            return Err(AdapterError::InitFailed(
                "OpenCode health endpoint reported unhealthy".into(),
            ));
        }
        if health.version.trim().is_empty() {
            return Err(AdapterError::Protocol(
                "OpenCode health endpoint returned an empty version".into(),
            ));
        }
        let version = health.version;
        *self.observed_version.write().map_err(|_| {
            AdapterError::Internal("OpenCode version state lock was poisoned".into())
        })? = Some(version.clone());

        Ok(AdapterProbe {
            executable_version: version.clone(),
            source_api_version: version,
            capability_version: ADAPTER_CAPABILITY_VERSION,
            capabilities: CapabilitySet {
                can_stream_deltas: true,
                can_approve_commands: true,
                can_approve_edits: true,
                can_interrupt: true,
                can_switch_credentials_live: self.managed_profile_id.is_some(),
                can_read_usage: false,
            },
            health: AdapterHealth::Healthy,
        })
    }

    async fn discover_projects(&self) -> Result<Vec<ProjectInfo>, AdapterError> {
        let projects: Vec<OpenCodeProject> =
            self.get_json(&["project"], "project discovery").await?;
        projects
            .into_iter()
            .map(|project| {
                let path = project.worktree.or(project.path).ok_or_else(|| {
                    AdapterError::Protocol("OpenCode project has no path".into())
                })?;
                let name = project
                    .name
                    .or(project.id)
                    .unwrap_or_else(|| path.clone());
                Ok(ProjectInfo { path, name })
            })
            .collect()
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError> {
        let mut summaries = Vec::new();
        let mut seen = HashSet::new();
        for directory in self.known_project_directories().await? {
            let sessions: Vec<OpenCodeSession> = if directory.is_empty() {
                self.get_json(&["session"], "session listing").await?
            } else {
                self.get_json_in_directory(&["session"], "session listing", &directory)
                    .await?
            };
            let statuses: HashMap<String, OpenCodeSessionStatus> = if directory.is_empty() {
                self.get_json(&["session", "status"], "session status listing")
                    .await?
            } else {
                self.get_json_in_directory(
                    &["session", "status"],
                    "session status listing",
                    &directory,
                )
                .await?
            };

            for session in sessions {
                Self::validate_nonempty(&session.id, "OpenCode session id")?;
                if !seen.insert(session.id.clone()) {
                    continue;
                }
                let title = session
                    .title
                    .unwrap_or_else(|| "Untitled session".into());
                let actual_directory = session
                    .directory
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| directory.clone());
                self.remember_session(&session.id, &actual_directory, Some(&title))?;
                summaries.push(SessionSummary {
                    status: statuses
                        .get(&session.id)
                        .map(|status| status.kind.clone())
                        .unwrap_or_else(|| "unknown".into()),
                    session_id: session.id,
                    project_path: actual_directory,
                    title,
                    credential_profile_id: self.bound_profile_id(),
                    created_at_ms: session
                        .time
                        .and_then(|time| time.created.or(time.updated))
                        .unwrap_or_default(),
                });
            }
        }
        summaries.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(summaries)
    }

    async fn read_session(&self, session_id: &str) -> Result<SessionSummary, AdapterError> {
        Self::validate_nonempty(session_id, "OpenCode session id")?;
        self.list_sessions()
            .await?
            .into_iter()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| AdapterError::SessionNotFound(session_id.to_owned()))
    }

    async fn subscribe_events(&self) -> Result<EventStream, AdapterError> {
        let request = self
            .client
            .get(self.endpoint(&["global", "event"])?)
            .header(ACCEPT, "text/event-stream");
        let response = self.send_read(request, "global event subscription").await?;
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !content_type
            .to_ascii_lowercase()
            .starts_with("text/event-stream")
        {
            return Err(AdapterError::Protocol(format!(
                "OpenCode global event endpoint returned unexpected content type {content_type:?}"
            )));
        }

        let adapter = self.clone();
        let (sender, receiver) = tokio::sync::mpsc::channel(SSE_CHANNEL_CAPACITY);
        tokio::spawn(async move {
            let mut source = response.bytes_stream();
            let mut parser = SseParser::new(MAX_SSE_EVENT_BYTES);
            while let Some(chunk) = source.next().await {
                let frames = match chunk {
                    Ok(chunk) => parser.push(&chunk),
                    Err(error) => {
                        let _ = sender
                            .send(Err(AdapterError::ConnectionLostWithDetail(
                                error.to_string(),
                            )))
                            .await;
                        return;
                    }
                };
                let frames = match frames {
                    Ok(frames) => frames,
                    Err(error) => {
                        let _ = sender.send(Err(error)).await;
                        return;
                    }
                };
                for frame in frames {
                    match adapter.normalize_global_event(&frame) {
                        Ok(Some(event)) => {
                            if sender.send(Ok(event)).await.is_err() {
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            if sender.send(Err(error)).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            match parser.finish() {
                Ok(frames) => {
                    for frame in frames {
                        match adapter.normalize_global_event(&frame) {
                            Ok(Some(event)) => {
                                if sender.send(Ok(event)).await.is_err() {
                                    return;
                                }
                            }
                            Ok(None) => {}
                            Err(error) => {
                                if sender.send(Err(error)).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error)).await;
                }
            }
        });

        Ok(Box::pin(futures::stream::unfold(
            receiver,
            |mut receiver| async move {
                receiver
                    .recv()
                    .await
                    .map(|event| (event, receiver))
            },
        )))
    }

    async fn start_session(
        &self,
        project_path: &str,
        prompt: &str,
        profile_id: &str,
    ) -> Result<String, AdapterError> {
        Self::validate_nonempty(project_path, "project path")?;
        Self::validate_nonempty(prompt, "prompt")?;
        match self.managed_profile_id.as_deref() {
            Some(bound_profile) if profile_id != bound_profile => {
                return Err(AdapterError::InvalidInput(format!(
                    "managed OpenCode runtime is bound to profile {bound_profile}"
                )));
            }
            None if !profile_id.trim().is_empty() => {
                return Err(AdapterError::Unsupported(
                    "OpenCode profile-bound session creation requires a managed isolated runtime"
                        .into(),
                ));
            }
            _ => {}
        }

        let request = self
            .client
            .post(self.endpoint(&["session"])?)
            .query(&[("directory", project_path)])
            .timeout(MUTATION_TIMEOUT)
            .json(&json!({}));
        let response = self.send_mutation(request, "session creation").await?;
        let session: OpenCodeSession = response.json().await.map_err(|error| {
            AdapterError::OutcomeUnknown(format!(
                "OpenCode created a session but its response was invalid; reconcile before retrying: {error}"
            ))
        })?;
        if session.id.trim().is_empty() {
            return Err(AdapterError::OutcomeUnknown(
                "OpenCode created a session but returned an empty id; reconcile before retrying"
                    .into(),
            ));
        }
        let directory = session
            .directory
            .unwrap_or_else(|| project_path.to_owned());
        let title = session.title.unwrap_or_default();
        self.remember_session(&session.id, &directory, Some(&title))?;

        if let Err(error) = self
            .send_prompt(&session.id, prompt, Some(&directory))
            .await
        {
            return Err(AdapterError::OutcomeUnknown(format!(
                "OpenCode session {} was created, but its initial prompt was not confirmed: {error}",
                session.id
            )));
        }
        Ok(session.id)
    }

    async fn send_input(&self, session_id: &str, text: &str) -> Result<(), AdapterError> {
        Self::validate_nonempty(session_id, "session id")?;
        Self::validate_nonempty(text, "input text")?;
        let context = self.resolve_session_context(session_id).await?;
        self.send_prompt(
            session_id,
            text,
            Some(&context.directory),
        )
        .await
    }

    async fn steer(&self, _session_id: &str, _instruction: &str) -> Result<(), AdapterError> {
        Err(Self::unsupported(
            "steering semantics distinct from a new user message",
        ))
    }

    async fn interrupt(&self, session_id: &str, _reason: &str) -> Result<(), AdapterError> {
        Self::validate_nonempty(session_id, "session id")?;
        let context = self.resolve_session_context(session_id).await?;
        let request = self
            .client
            .post(self.endpoint(&["session", session_id, "abort"])?);
        let request = self
            .with_directory(request, Some(&context.directory))
            .timeout(MUTATION_TIMEOUT);
        let response = self.send_mutation(request, "session abort").await?;
        let aborted: bool = response.json().await.map_err(|error| {
            AdapterError::OutcomeUnknown(format!(
                "OpenCode session abort returned invalid JSON; reconcile session {session_id}: {error}"
            ))
        })?;
        if aborted {
            Ok(())
        } else {
            Err(AdapterError::Protocol(format!(
                "OpenCode did not abort session {session_id}"
            )))
        }
    }

    async fn respond_approval(
        &self,
        session_id: &str,
        approval_id: &str,
        approved: bool,
        _reason: &str,
    ) -> Result<(), AdapterError> {
        Self::validate_nonempty(session_id, "session id")?;
        Self::validate_nonempty(approval_id, "approval id")?;
        let pending = self
            .pending_approvals
            .read()
            .map_err(|_| {
                AdapterError::Internal("OpenCode approval state lock was poisoned".into())
            })?
            .get(approval_id)
            .cloned();
        if let Some(pending) = &pending {
            if pending.session_id != session_id {
                return Err(AdapterError::InvalidInput(format!(
                    "approval {approval_id} belongs to session {}, not {session_id}",
                    pending.session_id
                )));
            }
        }
        let context = match pending {
            Some(pending) if !pending.directory.is_empty() => SessionContext {
                directory: pending.directory,
                title: String::new(),
            },
            _ => self.resolve_session_context(session_id).await?,
        };
        let request = self.client.post(self.endpoint(&[
            "session",
            session_id,
            "permissions",
            approval_id,
        ])?);
        let request = self
            .with_directory(request, Some(&context.directory))
            .timeout(MUTATION_TIMEOUT)
            .json(&ApprovalBody {
                response: if approved { "once" } else { "reject" },
            });
        let response = self.send_mutation(request, "permission response").await?;
        let accepted: bool = response.json().await.map_err(|error| {
            AdapterError::OutcomeUnknown(format!(
                "OpenCode permission response returned invalid JSON; reconcile approval {approval_id}: {error}"
            ))
        })?;
        if !accepted {
            return Err(AdapterError::Protocol(format!(
                "OpenCode did not accept permission response for {approval_id}"
            )));
        }
        self.pending_approvals
            .write()
            .map_err(|_| {
                AdapterError::Internal("OpenCode approval state lock was poisoned".into())
            })?
            .remove(approval_id);
        Ok(())
    }

    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<CredentialValidation, AdapterError> {
        if self.managed_profile_id.is_none() {
            return Err(Self::unsupported(
                "credential validation for an externally managed runtime",
            ));
        }
        let methods = self.auth_methods().await?;
        let provider_methods = methods.get(credential.provider_id()).ok_or_else(|| {
            AdapterError::CredentialInvalid(format!(
                "OpenCode provider {} is not exposed by the installed runtime",
                credential.provider_id()
            ))
        })?;
        let expected_kind = match credential.kind() {
            CredentialKind::ApiKey => "api",
        };
        if !provider_methods
            .iter()
            .any(|method| method.kind == expected_kind)
        {
            return Err(AdapterError::CredentialInvalid(format!(
                "OpenCode provider {} does not advertise {expected_kind} authentication",
                credential.provider_id()
            )));
        }
        credential.secret_utf8()?;
        Ok(CredentialValidation {
            status: CredentialStatus::Staged,
            provider_id: credential.provider_id().to_owned(),
            account_fingerprint: None,
        })
    }

    async fn activate_credential(
        &self,
        profile_id: &str,
        credential: &CredentialMaterial,
    ) -> Result<(), AdapterError> {
        let bound_profile = self.managed_profile_id.as_deref().ok_or_else(|| {
            Self::unsupported("credential activation for an externally managed runtime")
        })?;
        if profile_id != bound_profile {
            return Err(AdapterError::InvalidInput(format!(
                "managed OpenCode runtime is bound to profile {bound_profile}"
            )));
        }
        self.validate_credential(credential).await?;
        let secret = credential.secret_utf8()?;
        let request = self
            .client
            .put(self.endpoint(&["auth", credential.provider_id()])?)
            .timeout(MUTATION_TIMEOUT)
            .json(&ApiCredentialBody {
                kind: "api",
                key: secret,
            });
        let accepted: bool = self
            .send_mutation(request, "credential activation")
            .await?
            .json()
            .await
            .map_err(|error| {
                AdapterError::OutcomeUnknown(format!(
                    "OpenCode credential activation returned invalid JSON; readback and rollback are required: {error}"
                ))
            })?;
        if !accepted {
            return Err(AdapterError::CredentialInvalid(
                "OpenCode rejected the credential payload".into(),
            ));
        }
        let account = self.read_account_state(credential.provider_id()).await?;
        if !account.connected {
            return Err(AdapterError::OutcomeUnknown(format!(
                "OpenCode accepted provider {} authentication but readback did not show it connected",
                credential.provider_id()
            )));
        }
        Ok(())
    }

    async fn read_account_state(&self, provider_id: &str) -> Result<AccountState, AdapterError> {
        Self::validate_nonempty(provider_id, "provider id")?;
        let state = self.provider_state().await?;
        Ok(AccountState {
            provider_id: provider_id.to_owned(),
            connected: state
                .connected
                .iter()
                .any(|connected| connected == provider_id),
            account_fingerprint: None,
        })
    }

    async fn read_usage(&self) -> Result<UsageSnapshot, AdapterError> {
        Err(Self::unsupported(
            "usage reads for the installed OpenCode runtime",
        ))
    }

    fn compatibility_diagnostics(&self) -> Result<Vec<CompatibilityDiagnostic>, AdapterError> {
        self.compatibility_diagnostics
            .read()
            .map(|diagnostics| diagnostics.iter().cloned().collect())
            .map_err(|_| {
                AdapterError::Internal(
                    "OpenCode compatibility diagnostic lock was poisoned".into(),
                )
            })
    }

    async fn shutdown_gracefully(&self) -> Result<(), AdapterError> {
        // This adapter observes an externally managed HTTP server and owns no
        // child process. Dropping its event stream and client handles is the
        // complete local shutdown operation.
        Ok(())
    }
}

fn required_string<'a>(
    value: &'a Value,
    field: &str,
    event_type: &str,
) -> Result<&'a str, AdapterError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        AdapterError::Protocol(format!(
            "OpenCode {event_type} event has no string {field}"
        ))
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn new_event(timestamp_ms: i64, inner: event::Inner) -> Event {
    Event {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp_ms,
        inner: Some(inner),
    }
}

struct SseParser {
    max_event_bytes: usize,
    event_bytes: usize,
    line: Vec<u8>,
    data_lines: Vec<String>,
    pending_cr: bool,
}

impl SseParser {
    fn new(max_event_bytes: usize) -> Self {
        Self {
            max_event_bytes,
            event_bytes: 0,
            line: Vec::new(),
            data_lines: Vec::new(),
            pending_cr: false,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, AdapterError> {
        let mut frames = Vec::new();
        for &byte in chunk {
            if self.pending_cr {
                self.pending_cr = false;
                self.finish_line(&mut frames)?;
                if byte == b'\n' {
                    continue;
                }
            }

            self.event_bytes = self.event_bytes.checked_add(1).ok_or_else(|| {
                AdapterError::Protocol("OpenCode SSE event size overflowed".into())
            })?;
            if self.event_bytes > self.max_event_bytes {
                return Err(AdapterError::Protocol(format!(
                    "OpenCode SSE event exceeded {} bytes",
                    self.max_event_bytes
                )));
            }

            match byte {
                b'\r' => self.pending_cr = true,
                b'\n' => self.finish_line(&mut frames)?,
                _ => self.line.push(byte),
            }
        }
        Ok(frames)
    }

    fn finish(mut self) -> Result<Vec<String>, AdapterError> {
        let mut frames = Vec::new();
        if self.pending_cr {
            self.pending_cr = false;
            self.finish_line(&mut frames)?;
        }
        if !self.line.is_empty() {
            self.finish_line(&mut frames)?;
        }
        if !self.data_lines.is_empty() {
            frames.push(self.data_lines.join("\n"));
            self.data_lines.clear();
        }
        Ok(frames)
    }

    fn finish_line(&mut self, frames: &mut Vec<String>) -> Result<(), AdapterError> {
        let bytes = std::mem::take(&mut self.line);
        let line = String::from_utf8(bytes)
            .map_err(|_| AdapterError::Protocol("OpenCode SSE contained invalid UTF-8".into()))?;
        if line.is_empty() {
            if !self.data_lines.is_empty() {
                frames.push(self.data_lines.join("\n"));
                self.data_lines.clear();
            }
            self.event_bytes = 0;
            return Ok(());
        }
        if line.starts_with(':') {
            return Ok(());
        }
        let (field, value) = line
            .split_once(':')
            .map(|(field, value)| (field, value.strip_prefix(' ').unwrap_or(value)))
            .unwrap_or((line.as_str(), ""));
        if field == "data" {
            self.data_lines.push(value.to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn parses_sse_across_chunks_and_line_endings() {
        let mut parser = SseParser::new(1024);
        assert!(parser.push(b": heartbeat\r").unwrap().is_empty());
        assert!(parser
            .push(b"\ndata: {\"first\":\r\n")
            .unwrap()
            .is_empty());
        let frames = parser
            .push(b"data: \"second\"}\n\n")
            .expect("second SSE chunk");
        assert_eq!(frames, vec!["{\"first\":\n\"second\"}"]);
    }

    #[test]
    fn sse_parser_bounds_unterminated_events() {
        let mut parser = SseParser::new(8);
        assert!(matches!(
            parser.push(b"data: 123"),
            Err(AdapterError::Protocol(_))
        ));
    }

    #[test]
    fn normalizes_session_delta_and_permission_events() {
        let adapter = OpenCodeAdapter::new("http://127.0.0.1:9", None);
        let session = adapter
            .normalize_global_event(
                r#"{"directory":"/srv/app","payload":{"type":"session.updated","properties":{"info":{"id":"s1","directory":"/srv/app","title":"Build","time":{"updated":12}}}}}"#,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            session.inner,
            Some(event::Inner::SessionUpdated(SessionUpdatedEvent {
                session_id,
                title,
                ..
            })) if session_id == "s1" && title == "Build"
        ));

        let delta = adapter
            .normalize_global_event(
                r#"{"directory":"/srv/app","payload":{"type":"message.part.updated","properties":{"part":{"type":"text","sessionID":"s1","messageID":"m1"},"delta":"hi"}}}"#,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            delta.inner,
            Some(event::Inner::StreamDelta(StreamDeltaEvent {
                delta_text,
                is_final: false,
                ..
            })) if delta_text == "hi"
        ));

        let approval = adapter
            .normalize_global_event(
                r#"{"directory":"/srv/app","payload":{"type":"permission.updated","properties":{"id":"p1","type":"bash","sessionID":"s1","messageID":"m1","title":"Run tests","metadata":{},"time":{"created":42}}}}"#,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            approval.inner,
            Some(event::Inner::ApprovalRequested(ApprovalRequestedEvent {
                approval_id,
                session_id,
                ..
            })) if approval_id == "p1" && session_id == "s1"
        ));
        assert_eq!(
            adapter
                .pending_approvals
                .read()
                .unwrap()
                .get("p1")
                .unwrap()
                .directory,
            "/srv/app"
        );
    }

    #[test]
    fn retains_only_bounded_unknown_event_type_diagnostics() {
        let adapter = OpenCodeAdapter::new("http://127.0.0.1:9", None);
        for index in 0..130 {
            let event = format!(
                r#"{{"directory":"/srv/app","payload":{{"type":"future.event-{index}","properties":{{"secret":"must-not-leak"}}}}}}"#
            );
            assert!(adapter.normalize_global_event(&event).unwrap().is_none());
        }
        let diagnostics = adapter.compatibility_diagnostics().unwrap();
        assert_eq!(diagnostics.len(), COMPATIBILITY_DIAGNOSTIC_CAPACITY);
        assert_eq!(diagnostics[0].source_event_type, "future.event-2");
        let encoded = serde_json::to_string(&diagnostics).unwrap();
        assert!(!encoded.contains("must-not-leak"));
        assert!(!encoded.contains("/srv/app"));
    }

    #[tokio::test]
    async fn probe_records_version_and_reports_only_implemented_capabilities() {
        let (base_url, requests, server) = test_server(vec![json_response(
            r#"{"healthy":true,"version":"1.2.3"}"#,
        )])
        .await;
        let adapter = OpenCodeAdapter::new(base_url, Some("secret".into()));
        let probe = adapter.probe().await.unwrap();
        assert_eq!(probe.executable_version, "1.2.3");
        assert_eq!(probe.source_api_version, "1.2.3");
        assert_eq!(probe.capability_version, ADAPTER_CAPABILITY_VERSION);
        assert_eq!(probe.health, AdapterHealth::Healthy);
        assert!(probe.capabilities.can_stream_deltas);
        assert!(probe.capabilities.can_interrupt);
        assert!(probe.capabilities.can_approve_commands);
        assert!(!probe.capabilities.can_switch_credentials_live);
        assert_eq!(adapter.observed_version().unwrap().as_deref(), Some("1.2.3"));
        server.await.unwrap();
        let requests = requests.lock().unwrap();
        assert!(requests[0].starts_with("GET /global/health "));
        assert!(requests[0].contains("authorization: Basic b3BlbmNvZGU6c2VjcmV0"));
    }

    #[tokio::test]
    async fn creates_project_scoped_session_then_dispatches_prompt() {
        let (base_url, requests, server) = test_server(vec![
            json_response(
                r#"{"id":"s1","title":"New","directory":"/srv/app","time":{"created":1,"updated":1}}"#,
            ),
            no_content_response(),
        ])
        .await;
        let adapter = OpenCodeAdapter::new(base_url, None);
        let session_id = adapter
            .start_session("/srv/app", "build it", "")
            .await
            .unwrap();
        assert_eq!(session_id, "s1");
        server.await.unwrap();
        let requests = requests.lock().unwrap();
        assert!(requests[0].starts_with("POST /session?directory=%2Fsrv%2Fapp "));
        assert!(requests[1]
            .starts_with("POST /session/s1/prompt_async?directory=%2Fsrv%2Fapp "));
        assert!(requests[1].contains(r#""parts":[{"type":"text","text":"build it"}]"#));
    }

    #[tokio::test]
    async fn profile_bound_start_fails_before_network_io() {
        let adapter = OpenCodeAdapter::new("http://127.0.0.1:9", None);
        assert!(matches!(
            adapter
                .start_session("/srv/app", "build it", "account-a")
                .await,
            Err(AdapterError::Unsupported(_))
        ));
    }

    #[tokio::test]
    async fn managed_profile_discovers_auth_schema_activates_and_reads_back() {
        let (base_url, requests, server) = test_server(vec![
            json_response(r#"{"opencode":[{"type":"api","label":"API key"}]}"#),
            json_response("true"),
            json_response(r#"{"all":[],"default":{},"connected":["opencode"]}"#),
        ])
        .await;
        let adapter =
            OpenCodeAdapter::new_managed(base_url, Some("server-secret".into()), "go-a")
                .unwrap();
        let credential =
            CredentialMaterial::api_key("opencode", b"test-opencode-go-key").unwrap();

        adapter
            .activate_credential("go-a", &credential)
            .await
            .unwrap();

        server.await.unwrap();
        let requests = requests.lock().unwrap();
        assert!(requests[0].starts_with("GET /provider/auth "));
        assert!(requests[1].starts_with("PUT /auth/opencode "));
        assert!(requests[1].contains(
            r#"{"type":"api","key":"test-opencode-go-key"}"#
        ));
        assert!(requests[2].starts_with("GET /provider "));
        assert!(requests.iter().all(|request| request
            .contains("authorization: Basic b3BlbmNvZGU6c2VydmVyLXNlY3JldA==")));
    }

    #[tokio::test]
    async fn managed_profile_rejects_wrong_profile_and_unsupported_auth_method() {
        let adapter =
            OpenCodeAdapter::new_managed("http://127.0.0.1:9", None, "go-a").unwrap();
        let credential = CredentialMaterial::api_key("opencode", b"test-key").unwrap();
        assert!(matches!(
            adapter.activate_credential("go-b", &credential).await,
            Err(AdapterError::InvalidInput(_))
        ));

        let (base_url, _requests, server) = test_server(vec![json_response(
            r#"{"opencode":[{"type":"oauth","label":"Browser"}]}"#,
        )])
        .await;
        let adapter = OpenCodeAdapter::new_managed(base_url, None, "go-a").unwrap();
        assert!(matches!(
            adapter.validate_credential(&credential).await,
            Err(AdapterError::CredentialInvalid(_))
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn external_runtime_credential_mutation_stays_fail_closed() {
        let adapter = OpenCodeAdapter::new("http://127.0.0.1:9", None);
        let credential = CredentialMaterial::api_key("opencode", b"test-key").unwrap();
        assert!(matches!(
            adapter.activate_credential("", &credential).await,
            Err(AdapterError::Unsupported(_))
        ));
        assert!(matches!(
            adapter.validate_credential(&credential).await,
            Err(AdapterError::Unsupported(_))
        ));
    }

    #[tokio::test]
    async fn rejects_embedded_url_credentials_before_network_io() {
        let adapter = OpenCodeAdapter::new("http://user:password@127.0.0.1:9", None);
        assert!(matches!(
            adapter.probe().await,
            Err(AdapterError::InitFailed(_))
        ));
    }

    #[tokio::test]
    async fn lists_and_reconciles_sessions_across_projects() {
        let (base_url, requests, server) = test_server(vec![
            json_response(
                r#"[{"id":"project-a","worktree":"/srv/a"},{"id":"project-b","worktree":"/srv/b"}]"#,
            ),
            json_response(
                r#"[{"id":"session-a","title":"A","directory":"/srv/a","time":{"created":1}}]"#,
            ),
            json_response(r#"{"session-a":{"type":"busy"}}"#),
            json_response(
                r#"[{"id":"session-b","title":"B","directory":"/srv/b","time":{"created":2}}]"#,
            ),
            json_response(r#"{"session-b":{"type":"idle"}}"#),
        ])
        .await;
        let adapter = OpenCodeAdapter::new(base_url, None);
        let sessions = adapter.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "session-a");
        assert_eq!(sessions[0].status, "busy");
        assert_eq!(sessions[1].session_id, "session-b");
        assert_eq!(sessions[1].status, "idle");
        server.await.unwrap();
        let requests = requests.lock().unwrap();
        assert!(requests[1].starts_with("GET /session?directory=%2Fsrv%2Fa "));
        assert!(requests[4].starts_with(
            "GET /session/status?directory=%2Fsrv%2Fb "
        ));
    }

    #[tokio::test]
    async fn approval_reply_reconciles_session_after_adapter_restart() {
        let (base_url, requests, server) = test_server(vec![
            json_response(r#"[{"id":"project-a","worktree":"/srv/a"}]"#),
            json_response(
                r#"[{"id":"session-a","title":"A","directory":"/srv/a","time":{"created":1}}]"#,
            ),
            json_response("true"),
        ])
        .await;
        let adapter = OpenCodeAdapter::new(base_url, None);
        adapter
            .respond_approval("session-a", "permission-a", true, "approved remotely")
            .await
            .unwrap();
        server.await.unwrap();
        let requests = requests.lock().unwrap();
        assert!(requests[2].starts_with(
            "POST /session/session-a/permissions/permission-a?directory=%2Fsrv%2Fa "
        ));
        assert!(requests[2].contains(r#""response":"once""#));
    }

    #[tokio::test]
    async fn streams_normalized_global_events() {
        let body = concat!(
            "data: {\"directory\":\"/srv/app\",\"payload\":{\"type\":\"message.part.updated\",",
            "\"properties\":{\"part\":{\"type\":\"text\",\"sessionID\":\"s1\",",
            "\"messageID\":\"m1\"},\"delta\":\"hello\"}}}\n\n"
        );
        let (base_url, _requests, server) =
            test_server(vec![sse_response(body)]).await;
        let adapter = OpenCodeAdapter::new(base_url, None);
        let mut events = adapter.subscribe_events().await.unwrap();
        let event = events.next().await.unwrap().unwrap();
        assert!(matches!(
            event.inner,
            Some(event::Inner::StreamDelta(StreamDeltaEvent {
                delta_text,
                ..
            })) if delta_text == "hello"
        ));
        server.await.unwrap();
    }

    async fn test_server(
        responses: Vec<String>,
    ) -> (
        String,
        Arc<Mutex<Vec<String>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let server = tokio::spawn(async move {
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = read_request(&mut socket).await;
                captured.lock().unwrap().push(request);
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.shutdown().await.unwrap();
            }
        });
        (format!("http://{address}"), requests, server)
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 1024];
        let header_end;
        loop {
            let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0, "client closed before sending request headers");
            bytes.extend_from_slice(&chunk[..count]);
            if let Some(position) = find_bytes(&bytes, b"\r\n\r\n") {
                header_end = position + 4;
                break;
            }
        }
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or_default();
        while bytes.len() < header_end + content_length {
            let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0, "client closed before sending request body");
            bytes.extend_from_slice(&chunk[..count]);
        }
        String::from_utf8(bytes).unwrap()
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn json_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn no_content_response() -> String {
        "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into()
    }

    fn sse_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }
}
