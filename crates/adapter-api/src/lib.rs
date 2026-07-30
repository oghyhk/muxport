use async_trait::async_trait;
use futures::Stream;
use muxport_protocol::{AgentType, CredentialStatus, Event};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Adapter initialization error: {0}")]
    InitFailed(String),
    #[error("Connection lost to agent runtime")]
    ConnectionLost,
    #[error("Approval target not found: {0}")]
    ApprovalNotFound(String),
    #[error("Credential validation failed: {0}")]
    CredentialInvalid(String),
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: String,
    pub status: String,
    pub credential_profile_id: String,
    pub created_at_ms: i64,
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, AdapterError>> + Send>>;

#[async_trait]
pub trait AgentAdapter: Send + Sync {
    fn agent_type(&self) -> AgentType;
    async fn probe(&self) -> Result<CapabilitySet, AdapterError>;
    async fn discover_projects(&self) -> Result<Vec<ProjectInfo>, AdapterError>;
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError>;
    async fn subscribe_events(&self) -> Result<EventStream, AdapterError>;
    async fn start_session(&self, project_path: &str, prompt: &str, profile_id: &str) -> Result<String, AdapterError>;
    async fn send_input(&self, session_id: &str, text: &str) -> Result<(), AdapterError>;
    async fn steer(&self, session_id: &str, instruction: &str) -> Result<(), AdapterError>;
    async fn interrupt(&self, session_id: &str, reason: &str) -> Result<(), AdapterError>;
    async fn respond_approval(&self, approval_id: &str, approved: bool, reason: &str) -> Result<(), AdapterError>;
    async fn validate_credential(&self, secret_payload: &str) -> Result<CredentialStatus, AdapterError>;
    async fn activate_credential(&self, profile_id: &str, secret_payload: &str) -> Result<(), AdapterError>;
    async fn shutdown_gracefully(&self) -> Result<(), AdapterError>;
}
