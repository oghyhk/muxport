mod managed;
mod jsonrpc;

pub use managed::{ManagedCodexError, ManagedCodexProfile};
use adapter_api::{
    AccountState, AdapterError, AgentAdapter, CapabilitySet, CredentialKind, CredentialMaterial,
    CredentialValidation, EventStream, ProjectInfo, SessionSummary, UsageBucket, UsageSnapshot,
    UsageWindow,
};
use async_trait::async_trait;
use jsonrpc::{Incoming, JsonRpcPeer, ProcessConfig};
use muxport_protocol::{
    event, AgentType, ApprovalRequestedEvent, ApprovalResolvedEvent, CredentialStatus, Event,
    SessionUpdatedEvent, StreamDeltaEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, Mutex, Notify};
use tokio::process::Command;

const EVENT_CHANNEL_CAPACITY: usize = 256;
const THREAD_PAGE_LIMIT: usize = 100;
const MAX_THREAD_PAGES: usize = 100;
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const APP_SERVER_START_WINDOW: Duration = Duration::from_secs(10 * 60);
const MAX_APP_SERVER_STARTS_PER_WINDOW: usize = 5;

/// Codex adapter backed by the stable App Server JSONL/JSON-RPC API.
///
/// Each adapter owns one supervised `codex app-server` child. Credential
/// validation and switching remain unavailable until Muxport can prove
/// per-profile process and state isolation with rollback.
#[derive(Clone)]
pub struct CodexAdapter {
    process: ProcessConfig,
    profile_id: String,
    client: Arc<Mutex<Option<Arc<JsonRpcPeer>>>>,
    state: Arc<CodexState>,
    observed_version: Arc<RwLock<Option<String>>>,
    starts: Arc<Mutex<AppServerStartGuard>>,
}

#[derive(Default)]
struct AppServerStartGuard {
    attempts: VecDeque<Instant>,
    crash_loop_tripped: bool,
}

impl AppServerStartGuard {
    fn before_start(&mut self) -> Result<(), AdapterError> {
        if self.crash_loop_tripped {
            return Err(AdapterError::InitFailed(
                "Codex App Server crash loop is latched; operator restart is required".into(),
            ));
        }
        let now = Instant::now();
        while self
            .attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= APP_SERVER_START_WINDOW)
        {
            self.attempts.pop_front();
        }
        if self.attempts.len() >= MAX_APP_SERVER_STARTS_PER_WINDOW {
            self.crash_loop_tripped = true;
            return Err(AdapterError::InitFailed(format!(
                "Codex App Server crash loop: {} starts within {} seconds; automatic restart stopped",
                MAX_APP_SERVER_STARTS_PER_WINDOW,
                APP_SERVER_START_WINDOW.as_secs()
            )));
        }
        self.attempts.push_back(now);
        Ok(())
    }
}

struct CodexState {
    profile_id: String,
    events: broadcast::Sender<AdapterMessage>,
    sessions: RwLock<HashMap<String, SessionContext>>,
    active_turns: RwLock<HashMap<String, ActiveTurn>>,
    pending_approvals: RwLock<HashMap<String, PendingApproval>>,
    account: RwLock<Option<CodexAccount>>,
    rate_limits: RwLock<Option<Value>>,
    login_completions: RwLock<HashMap<String, Result<(), String>>>,
    login_completion_changed: Notify,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexAccount {
    ApiKey,
    Chatgpt {
        email: Option<String>,
        #[serde(rename = "planType")]
        plan_type: Option<String>,
    },
    AmazonBedrock {
        #[serde(rename = "credentialSource")]
        credential_source: Option<String>,
        #[serde(rename = "usesCodexManagedCredentials")]
        uses_codex_managed_credentials: Option<bool>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountRead {
    pub account: Option<CodexAccount>,
    pub requires_openai_auth: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexLoginStart {
    ApiKey,
    Chatgpt { login_id: String, auth_url: String },
    ChatgptDeviceCode {
        login_id: String,
        verification_url: String,
        user_code: String,
    },
}

#[derive(Clone, Debug)]
enum AdapterMessage {
    Event(Event),
    Failure(String),
}

#[derive(Clone, Debug, Default)]
struct SessionContext {
    project_path: String,
    title: String,
    status: String,
}

#[derive(Clone, Debug)]
struct ActiveTurn {
    turn_id: String,
    peer_id: String,
}

#[derive(Clone, Debug)]
struct PendingApproval {
    request_id: Value,
    session_id: String,
    peer_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadPage {
    data: Vec<CodexThread>,
    next_cursor: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThread {
    id: String,
    cwd: String,
    preview: String,
    name: Option<String>,
    status: Value,
    created_at: i64,
}

impl CodexAdapter {
    pub fn new(cmd_path: impl Into<String>) -> Self {
        Self::with_process(ProcessConfig::inherited(cmd_path.into()), String::new())
    }

    pub(crate) fn new_managed(process: ProcessConfig, profile_id: String) -> Self {
        Self::with_process(process, profile_id)
    }

    fn with_process(process: ProcessConfig, profile_id: String) -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            process,
            profile_id: profile_id.clone(),
            client: Arc::new(Mutex::new(None)),
            state: Arc::new(CodexState {
                profile_id: profile_id.clone(),
                events,
                sessions: RwLock::new(HashMap::new()),
                active_turns: RwLock::new(HashMap::new()),
                pending_approvals: RwLock::new(HashMap::new()),
                account: RwLock::new(None),
                rate_limits: RwLock::new(None),
                login_completions: RwLock::new(HashMap::new()),
                login_completion_changed: Notify::new(),
            }),
            observed_version: Arc::new(RwLock::new(None)),
            starts: Arc::new(Mutex::new(AppServerStartGuard::default())),
        }
    }

    pub fn observed_version(&self) -> Result<Option<String>, AdapterError> {
        self.observed_version
            .read()
            .map(|version| version.clone())
            .map_err(|_| AdapterError::Internal("Codex version state lock was poisoned".into()))
    }

    fn unsupported(operation: &str) -> AdapterError {
        AdapterError::Unsupported(format!(
            "Codex App Server {operation} is not implemented by this connector build"
        ))
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

    async fn ensure_client(&self) -> Result<Arc<JsonRpcPeer>, AdapterError> {
        let mut slot = self.client.lock().await;
        if let Some(client) = slot.as_ref() {
            if !client.is_closed() {
                return Ok(Arc::clone(client));
            }
        }
        if let Some(stale) = slot.take() {
            let _ = stale.shutdown().await;
        }

        self.starts.lock().await.before_start()?;
        let (client, incoming) = JsonRpcPeer::connect_process(&self.process).await?;
        let state = Arc::clone(&self.state);
        let reader_client = Arc::downgrade(&client);
        let peer_id = client.instance_id().to_owned();
        tokio::spawn(async move {
            run_incoming(reader_client, peer_id, state, incoming).await;
        });
        *slot = Some(Arc::clone(&client));
        Ok(client)
    }

    async fn record_version(&self) -> Result<(), AdapterError> {
        let output = tokio::time::timeout(
            VERSION_TIMEOUT,
            {
                let mut command = Command::new(self.process.cmd_path());
                self.process.configure(&mut command);
                command.arg("--version").output()
            },
        )
        .await
        .map_err(|_| AdapterError::InitFailed("Codex version probe timed out".into()))?
        .map_err(|error| AdapterError::InitFailed(error.to_string()))?;
        if !output.status.success() {
            return Err(AdapterError::InitFailed(format!(
                "`{} --version` exited with {}",
                self.process.cmd_path().display(),
                output.status
            )));
        }
        let version = String::from_utf8(output.stdout)
            .map_err(|_| AdapterError::Protocol("Codex version was not UTF-8".into()))?
            .trim()
            .to_owned();
        Self::validate_nonempty(&version, "Codex version")?;
        *self
            .observed_version
            .write()
            .map_err(|_| AdapterError::Internal("Codex version state lock was poisoned".into()))? =
            Some(version);
        Ok(())
    }

    pub async fn read_account(&self, refresh_token: bool) -> Result<CodexAccountRead, AdapterError> {
        let client = self.ensure_client().await?;
        let response = client
            .request(
                "account/read",
                json!({"refreshToken": refresh_token}),
                false,
            )
            .await?;
        let account: CodexAccountRead = serde_json::from_value(response).map_err(|error| {
            AdapterError::Protocol(format!(
                "Codex account/read returned an invalid response: {error}"
            ))
        })?;
        *self
            .state
            .account
            .write()
            .map_err(|_| AdapterError::Internal("Codex account state lock was poisoned".into()))? =
            account.account.clone();
        Ok(account)
    }

    pub async fn start_chatgpt_login(&self) -> Result<CodexLoginStart, AdapterError> {
        self.require_managed_profile("ChatGPT login")?;
        self.start_login(json!({
            "type": "chatgpt",
            "useHostedLoginSuccessPage": true,
            "appBrand": "codex"
        }))
        .await
    }

    pub async fn start_device_code_login(&self) -> Result<CodexLoginStart, AdapterError> {
        self.require_managed_profile("ChatGPT device-code login")?;
        self.start_login(json!({"type": "chatgptDeviceCode"})).await
    }

    async fn start_login(&self, params: Value) -> Result<CodexLoginStart, AdapterError> {
        let client = self.ensure_client().await?;
        let response = client
            .request("account/login/start", params, true)
            .await?;
        serde_json::from_value(response).map_err(|error| {
            AdapterError::Protocol(format!(
                "Codex account/login/start returned an invalid response: {error}"
            ))
        })
    }

    pub async fn cancel_login(&self, login_id: &str) -> Result<(), AdapterError> {
        Self::validate_nonempty(login_id, "login id")?;
        self.require_managed_profile("login cancellation")?;
        self.ensure_client()
            .await?
            .request(
                "account/login/cancel",
                json!({"loginId": login_id}),
                true,
            )
            .await?;
        Ok(())
    }

    pub async fn wait_for_login_completion(
        &self,
        login_id: &str,
        timeout: Duration,
    ) -> Result<(), AdapterError> {
        Self::validate_nonempty(login_id, "login id")?;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let notified = self.state.login_completion_changed.notified();
            if let Some(result) = self
                .state
                .login_completions
                .write()
                .map_err(|_| {
                    AdapterError::Internal(
                        "Codex login completion state lock was poisoned".into(),
                    )
                })?
                .remove(login_id)
            {
                return result.map_err(AdapterError::CredentialInvalid);
            }
            tokio::time::timeout_at(deadline, notified)
                .await
                .map_err(|_| {
                    AdapterError::OutcomeUnknown(format!(
                        "Codex login {login_id} did not complete before the local timeout"
                    ))
                })?;
        }
    }

    pub async fn logout(&self) -> Result<(), AdapterError> {
        self.require_managed_profile("logout")?;
        self.ensure_client()
            .await?
            .request("account/logout", json!({}), true)
            .await?;
        Ok(())
    }

    pub async fn read_rate_limits(&self) -> Result<Value, AdapterError> {
        let response = self
            .ensure_client()
            .await?
            .request("account/rateLimits/read", json!({}), false)
            .await?;
        *self.state.rate_limits.write().map_err(|_| {
            AdapterError::Internal("Codex rate-limit state lock was poisoned".into())
        })? = Some(response.clone());
        Ok(response)
    }

    fn require_managed_profile(&self, operation: &str) -> Result<(), AdapterError> {
        if self.profile_id.is_empty() {
            Err(Self::unsupported(&format!(
                "{operation} requires an isolated managed profile"
            )))
        } else {
            Ok(())
        }
    }

    async fn list_all_threads(
        &self,
        client: &JsonRpcPeer,
    ) -> Result<Vec<CodexThread>, AdapterError> {
        let mut threads = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen_cursors = HashSet::new();
        for _ in 0..MAX_THREAD_PAGES {
            let params = match cursor.as_ref() {
                Some(cursor) => json!({"limit": THREAD_PAGE_LIMIT, "cursor": cursor}),
                None => json!({"limit": THREAD_PAGE_LIMIT}),
            };
            let response = client.request("thread/list", params, false).await?;
            let page: ThreadPage = serde_json::from_value(response).map_err(|error| {
                AdapterError::Protocol(format!(
                    "Codex thread/list returned an invalid response: {error}"
                ))
            })?;
            for thread in &page.data {
                self.state.remember_thread(thread)?;
            }
            threads.extend(page.data);
            match page.next_cursor {
                Some(next) if !next.is_empty() => {
                    if !seen_cursors.insert(next.clone()) {
                        return Err(AdapterError::Protocol(
                            "Codex thread/list repeated a pagination cursor".into(),
                        ));
                    }
                    cursor = Some(next);
                }
                _ => return Ok(threads),
            }
        }
        Err(AdapterError::Protocol(format!(
            "Codex thread/list exceeded {MAX_THREAD_PAGES} pages"
        )))
    }

    async fn resume_thread(
        &self,
        client: &JsonRpcPeer,
        session_id: &str,
    ) -> Result<(), AdapterError> {
        let response = client
            .request("thread/resume", json!({"threadId": session_id}), false)
            .await?;
        if let Some(thread) = response.get("thread") {
            self.state.remember_thread_value(thread)?;
        } else {
            return Err(AdapterError::Protocol(
                "Codex thread/resume response had no thread".into(),
            ));
        }
        Ok(())
    }

    async fn start_turn(
        &self,
        client: &JsonRpcPeer,
        session_id: &str,
        text: &str,
    ) -> Result<String, AdapterError> {
        let response = client
            .request(
                "turn/start",
                json!({
                    "threadId": session_id,
                    "input": [{"type": "text", "text": text}]
                }),
                true,
            )
            .await?;
        let turn_id = response
            .get("turn")
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AdapterError::OutcomeUnknown(format!(
                    "Codex turn/start returned no turn id; reconcile thread {session_id}"
                ))
            })?
            .to_owned();
        self.state.set_active_turn(
            session_id,
            ActiveTurn {
                turn_id: turn_id.clone(),
                peer_id: client.instance_id().to_owned(),
            },
        )?;
        Ok(turn_id)
    }
}

const MAX_USAGE_BUCKETS: usize = 32;
const MAX_USAGE_BUCKET_ID_CHARS: usize = 128;

fn normalized_usage_window(value: Option<&Value>) -> Option<UsageWindow> {
    let value = value?.as_object()?;
    let used_percent = value.get("usedPercent")?.as_i64()?;
    Some(UsageWindow {
        used_percent: used_percent.clamp(0, 100) as i32,
        resets_at_unix_seconds: value
            .get("resetsAt")
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0),
        duration_minutes: value
            .get("windowDurationMins")
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0),
    })
}

fn bounded_usage_bucket_id(value: &str) -> Option<String> {
    let bounded = value
        .trim()
        .chars()
        .take(MAX_USAGE_BUCKET_ID_CHARS)
        .collect::<String>();
    (!bounded.is_empty()).then_some(bounded)
}

fn normalized_usage_bucket(value: &Value, fallback_id: Option<&str>) -> Option<UsageBucket> {
    let value = value.as_object()?;
    let primary = normalized_usage_window(value.get("primary"));
    let secondary = normalized_usage_window(value.get("secondary"));
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    let bucket_id = value
        .get("limitId")
        .and_then(Value::as_str)
        .or(fallback_id)
        .and_then(bounded_usage_bucket_id);
    Some(UsageBucket {
        bucket_id,
        primary,
        secondary,
    })
}

fn normalized_usage_buckets(response: &Value) -> Vec<UsageBucket> {
    if let Some(by_id) = response
        .get("rateLimitsByLimitId")
        .and_then(Value::as_object)
        .filter(|buckets| !buckets.is_empty())
    {
        let mut entries = by_id.iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(right.0));
        return entries
            .into_iter()
            .take(MAX_USAGE_BUCKETS)
            .filter_map(|(id, value)| normalized_usage_bucket(value, Some(id)))
            .collect();
    }
    response
        .get("rateLimits")
        .and_then(|value| normalized_usage_bucket(value, None))
        .into_iter()
        .collect()
}

#[async_trait]
impl AgentAdapter for CodexAdapter {
    fn agent_type(&self) -> AgentType {
        AgentType::Codex
    }

    async fn probe(&self) -> Result<CapabilitySet, AdapterError> {
        self.record_version().await?;
        self.ensure_client().await?;
        Ok(CapabilitySet {
            can_stream_deltas: true,
            can_approve_commands: true,
            can_approve_edits: true,
            can_interrupt: true,
            can_switch_credentials_live: !self.profile_id.is_empty(),
            can_read_usage: !self.profile_id.is_empty(),
        })
    }

    async fn discover_projects(&self) -> Result<Vec<ProjectInfo>, AdapterError> {
        let sessions = self.list_sessions().await?;
        let mut projects = BTreeMap::new();
        for session in sessions {
            if session.project_path.is_empty() {
                continue;
            }
            projects
                .entry(session.project_path.clone())
                .or_insert_with(|| ProjectInfo {
                    name: path_name(&session.project_path),
                    path: session.project_path,
                });
        }
        Ok(projects.into_values().collect())
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, AdapterError> {
        let client = self.ensure_client().await?;
        self.list_all_threads(&client)
            .await?
            .into_iter()
            .map(|thread| thread_to_summary(thread, &self.profile_id))
            .collect()
    }

    async fn read_session(&self, session_id: &str) -> Result<SessionSummary, AdapterError> {
        Self::validate_nonempty(session_id, "Codex session id")?;
        self.list_sessions()
            .await?
            .into_iter()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| AdapterError::SessionNotFound(session_id.to_owned()))
    }

    async fn subscribe_events(&self) -> Result<EventStream, AdapterError> {
        self.ensure_client().await?;
        let receiver = self.state.events.subscribe();
        let stream = futures::stream::unfold(Some(receiver), |state| async move {
            let mut receiver = state?;
            match receiver.recv().await {
                Ok(AdapterMessage::Event(event)) => Some((Ok(event), Some(receiver))),
                Ok(AdapterMessage::Failure(detail)) => Some((
                    Err(AdapterError::ConnectionLostWithDetail(detail)),
                    None,
                )),
                Err(broadcast::error::RecvError::Lagged(count)) => Some((
                    Err(AdapterError::ConnectionLostWithDetail(format!(
                        "Codex event subscriber missed {count} messages; resynchronize"
                    ))),
                    None,
                )),
                Err(broadcast::error::RecvError::Closed) => None,
            }
        });
        Ok(Box::pin(stream))
    }

    async fn start_session(
        &self,
        project_path: &str,
        prompt: &str,
        profile_id: &str,
    ) -> Result<String, AdapterError> {
        Self::validate_nonempty(project_path, "project path")?;
        Self::validate_nonempty(prompt, "prompt")?;
        if !Path::new(project_path).is_absolute() {
            return Err(AdapterError::InvalidInput(
                "project path must be absolute".into(),
            ));
        }
        if profile_id != self.profile_id {
            return Err(AdapterError::InvalidInput(format!(
                "Codex session profile {profile_id:?} does not match adapter profile {:?}",
                self.profile_id
            )));
        }

        let client = self.ensure_client().await?;
        let response = client
            .request(
                "thread/start",
                json!({
                    "cwd": project_path,
                    "approvalPolicy": "on-request",
                    "approvalsReviewer": "user",
                    "sandbox": "workspace-write"
                }),
                true,
            )
            .await?;
        let thread = response.get("thread").ok_or_else(|| {
            AdapterError::OutcomeUnknown(
                "Codex thread/start returned no thread; reconcile before retrying".into(),
            )
        })?;
        let session_id = required_string(thread, "id", "thread/start")?.to_owned();
        self.state.remember_thread_value(thread)?;

        if let Err(error) = self.start_turn(&client, &session_id, prompt).await {
            return Err(AdapterError::OutcomeUnknown(format!(
                "Codex created thread {session_id}, but its initial turn was not confirmed: {error}"
            )));
        }
        Ok(session_id)
    }

    async fn send_input(&self, session_id: &str, text: &str) -> Result<(), AdapterError> {
        Self::validate_nonempty(session_id, "session id")?;
        Self::validate_nonempty(text, "input text")?;
        let client = self.ensure_client().await?;
        self.resume_thread(&client, session_id).await?;
        self.start_turn(&client, session_id, text).await?;
        Ok(())
    }

    async fn steer(&self, session_id: &str, instruction: &str) -> Result<(), AdapterError> {
        Self::validate_nonempty(session_id, "session id")?;
        Self::validate_nonempty(instruction, "steering instruction")?;
        let client = self.ensure_client().await?;
        let active = self
            .state
            .active_turn(session_id)?
            .filter(|turn| turn.peer_id == client.instance_id())
            .ok_or_else(|| {
                AdapterError::Protocol(format!(
                    "Codex thread {session_id} has no known active turn; reconcile before steering"
                ))
            })?;
        let response = client
            .request(
                "turn/steer",
                json!({
                    "threadId": session_id,
                    "expectedTurnId": active.turn_id,
                    "input": [{"type": "text", "text": instruction}]
                }),
                true,
            )
            .await?;
        let returned_turn = response
            .get("turnId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AdapterError::OutcomeUnknown(format!(
                    "Codex turn/steer returned no turn id; reconcile thread {session_id}"
                ))
            })?;
        if returned_turn != active.turn_id {
            return Err(AdapterError::OutcomeUnknown(format!(
                "Codex turn/steer returned unexpected turn {returned_turn}; reconcile thread {session_id}"
            )));
        }
        Ok(())
    }

    async fn interrupt(&self, session_id: &str, _reason: &str) -> Result<(), AdapterError> {
        Self::validate_nonempty(session_id, "session id")?;
        let client = self.ensure_client().await?;
        let active = self
            .state
            .active_turn(session_id)?
            .filter(|turn| turn.peer_id == client.instance_id())
            .ok_or_else(|| {
                AdapterError::Protocol(format!(
                    "Codex thread {session_id} has no known active turn; reconcile before interrupting"
                ))
            })?;
        client
            .request(
                "turn/interrupt",
                json!({"threadId": session_id, "turnId": active.turn_id}),
                true,
            )
            .await?;
        self.state.clear_active_turn(
            session_id,
            &active.turn_id,
            client.instance_id(),
        )?;
        Ok(())
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
            .state
            .pending_approval(approval_id)?
            .ok_or_else(|| AdapterError::ApprovalNotFound(approval_id.to_owned()))?;
        if pending.session_id != session_id {
            return Err(AdapterError::InvalidInput(format!(
                "approval {approval_id} belongs to session {}, not {session_id}",
                pending.session_id
            )));
        }
        let client = self.ensure_client().await?;
        if pending.peer_id != client.instance_id() {
            return Err(AdapterError::ApprovalNotFound(format!(
                "{approval_id} belongs to a previous Codex App Server process"
            )));
        }
        client
            .respond_result(
                pending.request_id,
                json!({"decision": if approved { "accept" } else { "decline" }}),
            )
            .await?;
        self.state.remove_pending_approval(approval_id)?;
        self.state.emit(new_event(
            now_ms(),
            event::Inner::ApprovalResolved(ApprovalResolvedEvent {
                approval_id: approval_id.to_owned(),
                approved,
                resolved_by: "muxport".into(),
                session_id: session_id.to_owned(),
            }),
        ));
        Ok(())
    }

    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<CredentialValidation, AdapterError> {
        self.require_managed_profile("credential validation")?;
        if credential.provider_id() != "openai" || credential.kind() != CredentialKind::ApiKey {
            return Err(AdapterError::CredentialInvalid(
                "Codex managed profiles accept only OpenAI API keys".into(),
            ));
        }
        let secret = credential.secret_utf8()?;
        if secret.trim() != secret || secret.len() < 8 || secret.chars().any(char::is_whitespace) {
            return Err(AdapterError::CredentialInvalid(
                "OpenAI API key has an invalid shape".into(),
            ));
        }
        Ok(CredentialValidation {
            status: CredentialStatus::Staged,
            provider_id: "openai".into(),
            account_fingerprint: None,
        })
    }

    async fn activate_credential(
        &self,
        profile_id: &str,
        credential: &CredentialMaterial,
    ) -> Result<(), AdapterError> {
        self.require_managed_profile("credential activation")?;
        if profile_id != self.profile_id {
            return Err(AdapterError::InvalidInput(format!(
                "credential activation profile {profile_id:?} does not match adapter profile {:?}",
                self.profile_id
            )));
        }
        self.validate_credential(credential).await?;
        let response = self
            .ensure_client()
            .await?
            .request(
                "account/login/start",
                json!({"type": "apiKey", "apiKey": credential.secret_utf8()?}),
                true,
            )
            .await?;
        let login: CodexLoginStart = serde_json::from_value(response).map_err(|error| {
            AdapterError::OutcomeUnknown(format!(
                "Codex accepted account/login/start but returned an invalid response: {error}"
            ))
        })?;
        if login != CodexLoginStart::ApiKey {
            return Err(AdapterError::OutcomeUnknown(
                "Codex API-key login returned an unexpected login type".into(),
            ));
        }
        Ok(())
    }

    async fn read_account_state(&self, provider_id: &str) -> Result<AccountState, AdapterError> {
        self.require_managed_profile("account state")?;
        if provider_id != "openai" {
            return Err(AdapterError::InvalidInput(
                "Codex account provider must be openai".into(),
            ));
        }
        let account = self.read_account(false).await?;
        let connected = matches!(
            account.account.as_ref(),
            Some(CodexAccount::ApiKey | CodexAccount::Chatgpt { .. })
        );
        Ok(AccountState {
            provider_id: provider_id.to_owned(),
            connected,
            account_fingerprint: match account.account.as_ref() {
                Some(CodexAccount::Chatgpt { email, .. }) => email.clone(),
                _ => None,
            },
        })
    }

    async fn read_usage(&self) -> Result<UsageSnapshot, AdapterError> {
        let response = self.read_rate_limits().await?;
        Ok(UsageSnapshot {
            provider_id: "openai".into(),
            observed_at_ms: now_ms(),
            buckets: normalized_usage_buckets(&response),
        })
    }

    async fn shutdown_gracefully(&self) -> Result<(), AdapterError> {
        let client = self.client.lock().await.take();
        match client {
            Some(client) => client.shutdown().await,
            None => Ok(()),
        }
    }
}

impl CodexState {
    fn emit(&self, event: Event) {
        let _ = self.events.send(AdapterMessage::Event(event));
    }

    fn fail(&self, detail: String) {
        let _ = self.events.send(AdapterMessage::Failure(detail));
    }

    fn remember_thread(&self, thread: &CodexThread) -> Result<(), AdapterError> {
        self.remember_session(
            &thread.id,
            &thread.cwd,
            &thread_title(thread),
            thread_status(&thread.status)?,
        )
    }

    fn remember_thread_value(&self, thread: &Value) -> Result<(), AdapterError> {
        let parsed: CodexThread = serde_json::from_value(thread.clone()).map_err(|error| {
            AdapterError::Protocol(format!("Codex returned an invalid thread: {error}"))
        })?;
        self.remember_thread(&parsed)
    }

    fn remember_session(
        &self,
        session_id: &str,
        project_path: &str,
        title: &str,
        status: &str,
    ) -> Result<(), AdapterError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| AdapterError::Internal("Codex session state lock was poisoned".into()))?;
        let prior = sessions.get(session_id).cloned().unwrap_or_default();
        sessions.insert(
            session_id.to_owned(),
            SessionContext {
                project_path: if project_path.is_empty() {
                    prior.project_path
                } else {
                    project_path.to_owned()
                },
                title: if title.is_empty() {
                    prior.title
                } else {
                    title.to_owned()
                },
                status: if status.is_empty() {
                    prior.status
                } else {
                    status.to_owned()
                },
            },
        );
        Ok(())
    }

    fn session_context(&self, session_id: &str) -> Result<SessionContext, AdapterError> {
        self.sessions
            .read()
            .map_err(|_| AdapterError::Internal("Codex session state lock was poisoned".into()))
            .map(|sessions| sessions.get(session_id).cloned().unwrap_or_default())
    }

    fn set_active_turn(
        &self,
        session_id: &str,
        turn: ActiveTurn,
    ) -> Result<(), AdapterError> {
        self.active_turns
            .write()
            .map_err(|_| AdapterError::Internal("Codex turn state lock was poisoned".into()))?
            .insert(session_id.to_owned(), turn);
        Ok(())
    }

    fn active_turn(&self, session_id: &str) -> Result<Option<ActiveTurn>, AdapterError> {
        self.active_turns
            .read()
            .map_err(|_| AdapterError::Internal("Codex turn state lock was poisoned".into()))
            .map(|turns| turns.get(session_id).cloned())
    }

    fn clear_active_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        peer_id: &str,
    ) -> Result<(), AdapterError> {
        let mut turns = self
            .active_turns
            .write()
            .map_err(|_| AdapterError::Internal("Codex turn state lock was poisoned".into()))?;
        if matches!(
            turns.get(session_id),
            Some(turn) if turn.turn_id == turn_id && turn.peer_id == peer_id
        ) {
            turns.remove(session_id);
        }
        Ok(())
    }

    fn pending_approval(
        &self,
        approval_id: &str,
    ) -> Result<Option<PendingApproval>, AdapterError> {
        self.pending_approvals
            .read()
            .map_err(|_| AdapterError::Internal("Codex approval state lock was poisoned".into()))
            .map(|approvals| approvals.get(approval_id).cloned())
    }

    fn remove_pending_approval(&self, approval_id: &str) -> Result<(), AdapterError> {
        self.pending_approvals
            .write()
            .map_err(|_| AdapterError::Internal("Codex approval state lock was poisoned".into()))?
            .remove(approval_id);
        Ok(())
    }

    fn clear_peer_state(&self, peer_id: &str) -> Result<(), AdapterError> {
        self.active_turns
            .write()
            .map_err(|_| AdapterError::Internal("Codex turn state lock was poisoned".into()))?
            .retain(|_, turn| turn.peer_id != peer_id);
        self.pending_approvals
            .write()
            .map_err(|_| AdapterError::Internal("Codex approval state lock was poisoned".into()))?
            .retain(|_, approval| approval.peer_id != peer_id);
        Ok(())
    }

    fn notification_events(
        &self,
        method: &str,
        params: &Value,
        peer_id: &str,
    ) -> Result<Vec<Event>, AdapterError> {
        match method {
            "thread/started" => {
                let thread = params.get("thread").ok_or_else(|| {
                    AdapterError::Protocol("Codex thread/started had no thread".into())
                })?;
                let parsed: CodexThread =
                    serde_json::from_value(thread.clone()).map_err(|error| {
                        AdapterError::Protocol(format!(
                            "Codex thread/started contained an invalid thread: {error}"
                        ))
                    })?;
                self.remember_thread(&parsed)?;
                Ok(vec![session_event(
                    &parsed.id,
                    &thread_title(&parsed),
                    thread_status(&parsed.status)?,
                    &parsed.cwd,
                    &self.profile_id,
                    now_ms(),
                )])
            }
            "thread/status/changed" => {
                let session_id = required_string(params, "threadId", method)?;
                let status = thread_status(params.get("status").ok_or_else(|| {
                    AdapterError::Protocol("Codex thread status event had no status".into())
                })?)?;
                self.remember_session(session_id, "", "", status)?;
                let context = self.session_context(session_id)?;
                Ok(vec![session_event(
                    session_id,
                    &context.title,
                    status,
                    &context.project_path,
                    &self.profile_id,
                    now_ms(),
                )])
            }
            "thread/name/updated" => {
                let session_id = required_string(params, "threadId", method)?;
                let title = params
                    .get("threadName")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.remember_session(session_id, "", title, "")?;
                let context = self.session_context(session_id)?;
                Ok(vec![session_event(
                    session_id,
                    &context.title,
                    &context.status,
                    &context.project_path,
                    &self.profile_id,
                    now_ms(),
                )])
            }
            "thread/archived" | "thread/deleted" | "thread/closed" | "thread/unarchived" => {
                let session_id = required_string(params, "threadId", method)?;
                let status = method.strip_prefix("thread/").unwrap_or(method);
                self.remember_session(session_id, "", "", status)?;
                let context = self.session_context(session_id)?;
                Ok(vec![session_event(
                    session_id,
                    &context.title,
                    status,
                    &context.project_path,
                    &self.profile_id,
                    now_ms(),
                )])
            }
            "turn/started" => {
                let session_id = required_string(params, "threadId", method)?;
                let turn = params.get("turn").ok_or_else(|| {
                    AdapterError::Protocol("Codex turn/started had no turn".into())
                })?;
                let turn_id = required_string(turn, "id", method)?;
                self.set_active_turn(
                    session_id,
                    ActiveTurn {
                        turn_id: turn_id.to_owned(),
                        peer_id: peer_id.to_owned(),
                    },
                )?;
                self.remember_session(session_id, "", "", "inProgress")?;
                let context = self.session_context(session_id)?;
                Ok(vec![session_event(
                    session_id,
                    &context.title,
                    "inProgress",
                    &context.project_path,
                    &self.profile_id,
                    timestamp_seconds_to_ms(
                        turn.get("startedAt").and_then(Value::as_i64),
                    ),
                )])
            }
            "turn/completed" => {
                let session_id = required_string(params, "threadId", method)?;
                let turn = params.get("turn").ok_or_else(|| {
                    AdapterError::Protocol("Codex turn/completed had no turn".into())
                })?;
                let turn_id = required_string(turn, "id", method)?;
                let status = required_string(turn, "status", method)?;
                self.clear_active_turn(session_id, turn_id, peer_id)?;
                self.remember_session(session_id, "", "", status)?;
                let context = self.session_context(session_id)?;
                let timestamp = timestamp_seconds_to_ms(
                    turn.get("completedAt").and_then(Value::as_i64),
                );
                Ok(vec![
                    new_event(
                        timestamp,
                        event::Inner::StreamDelta(StreamDeltaEvent {
                            session_id: session_id.to_owned(),
                            turn_id: turn_id.to_owned(),
                            delta_text: String::new(),
                            is_final: true,
                        }),
                    ),
                    session_event(
                        session_id,
                        &context.title,
                        status,
                        &context.project_path,
                        &self.profile_id,
                        timestamp,
                    ),
                ])
            }
            "item/agentMessage/delta" => {
                let session_id = required_string(params, "threadId", method)?;
                let turn_id = required_string(params, "turnId", method)?;
                let delta = required_string(params, "delta", method)?;
                Ok(vec![new_event(
                    now_ms(),
                    event::Inner::StreamDelta(StreamDeltaEvent {
                        session_id: session_id.to_owned(),
                        turn_id: turn_id.to_owned(),
                        delta_text: delta.to_owned(),
                        is_final: false,
                    }),
                )])
            }
            "serverRequest/resolved" => {
                let request_id = params.get("requestId").ok_or_else(|| {
                    AdapterError::Protocol(
                        "Codex serverRequest/resolved had no requestId".into(),
                    )
                })?;
                self.pending_approvals
                    .write()
                    .map_err(|_| {
                        AdapterError::Internal("Codex approval state lock was poisoned".into())
                    })?
                    .retain(|_, approval| {
                        approval.peer_id != peer_id || approval.request_id != *request_id
                    });
                Ok(Vec::new())
            }
            "account/updated" => {
                let auth_mode = params.get("authMode").and_then(Value::as_str);
                let account = match auth_mode {
                    None => None,
                    Some("apikey") => Some(CodexAccount::ApiKey),
                    Some("chatgpt") | Some("chatgptAuthTokens") => {
                        let prior_email = self
                            .account
                            .read()
                            .map_err(|_| {
                                AdapterError::Internal(
                                    "Codex account state lock was poisoned".into(),
                                )
                            })?
                            .as_ref()
                            .and_then(|account| match account {
                                CodexAccount::Chatgpt { email, .. } => email.clone(),
                                _ => None,
                            });
                        Some(CodexAccount::Chatgpt {
                            email: prior_email,
                            plan_type: params
                                .get("planType")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        })
                    }
                    Some(_) => return Ok(Vec::new()),
                };
                *self.account.write().map_err(|_| {
                    AdapterError::Internal("Codex account state lock was poisoned".into())
                })? = account;
                Ok(Vec::new())
            }
            "account/login/completed" => {
                let Some(login_id) = params.get("loginId").and_then(Value::as_str) else {
                    return Ok(Vec::new());
                };
                let success = params
                    .get("success")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        AdapterError::Protocol(
                            "Codex account/login/completed had no boolean success".into(),
                        )
                    })?;
                let result = if success {
                    Ok(())
                } else {
                    Err(params
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("Codex login failed")
                        .to_owned())
                };
                self.login_completions
                    .write()
                    .map_err(|_| {
                        AdapterError::Internal(
                            "Codex login completion state lock was poisoned".into(),
                        )
                    })?
                    .insert(login_id.to_owned(), result);
                self.login_completion_changed.notify_waiters();
                Ok(Vec::new())
            }
            "account/rateLimits/updated" => {
                *self.rate_limits.write().map_err(|_| {
                    AdapterError::Internal("Codex rate-limit state lock was poisoned".into())
                })? = Some(params.clone());
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn approval_request(
        &self,
        method: &str,
        request_id: Value,
        params: &Value,
        peer_id: &str,
    ) -> Result<Event, AdapterError> {
        let session_id = required_string(params, "threadId", method)?;
        required_string(params, "turnId", method)?;
        required_string(params, "itemId", method)?;
        let action_type = match method {
            "item/commandExecution/requestApproval" => "command",
            "item/fileChange/requestApproval" => "edit",
            _ => {
                return Err(AdapterError::Unsupported(format!(
                    "Codex server request {method}"
                )))
            }
        };
        let description = params
            .get("reason")
            .and_then(Value::as_str)
            .or_else(|| params.get("command").and_then(Value::as_str))
            .unwrap_or(if action_type == "command" {
                "Codex requests permission to run a command"
            } else {
                "Codex requests permission to edit files"
            });
        let approval_id = uuid::Uuid::new_v4().to_string();
        self.pending_approvals
            .write()
            .map_err(|_| AdapterError::Internal("Codex approval state lock was poisoned".into()))?
            .insert(
                approval_id.clone(),
                PendingApproval {
                    request_id,
                    session_id: session_id.to_owned(),
                    peer_id: peer_id.to_owned(),
                },
            );
        let timestamp = params
            .get("startedAtMs")
            .and_then(Value::as_i64)
            .unwrap_or_else(now_ms);
        Ok(new_event(
            timestamp,
            event::Inner::ApprovalRequested(ApprovalRequestedEvent {
                approval_id,
                session_id: session_id.to_owned(),
                action_type: action_type.to_owned(),
                description: description.to_owned(),
                payload_json: serde_json::to_string(params)
                    .map_err(|error| AdapterError::Internal(error.to_string()))?,
            }),
        ))
    }
}

async fn run_incoming(
    peer: Weak<JsonRpcPeer>,
    peer_id: String,
    state: Arc<CodexState>,
    mut incoming: mpsc::Receiver<Incoming>,
) {
    while let Some(message) = incoming.recv().await {
        match message {
            Incoming::Message(value) => {
                let method = match value.get("method").and_then(Value::as_str) {
                    Some(method) => method,
                    None => {
                        fail_peer(
                            &peer,
                            &state,
                            "Codex incoming message had no method".into(),
                        )
                        .await;
                        break;
                    }
                };
                if let Some(request_id) = value.get("id").cloned() {
                    let Some(active_peer) = peer.upgrade() else {
                        break;
                    };
                    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
                    match state.approval_request(
                        method,
                        request_id.clone(),
                        &params,
                        &peer_id,
                    ) {
                        Ok(event) => state.emit(event),
                        Err(AdapterError::Unsupported(_)) => {
                            if let Err(error) = active_peer
                                .respond_error(
                                    request_id,
                                    -32601,
                                    "Muxport does not support this Codex server request",
                                )
                                .await
                            {
                                state.fail(error.to_string());
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = active_peer
                                .respond_error(request_id, -32602, "Invalid approval request")
                                .await;
                            fail_peer(&peer, &state, error.to_string()).await;
                            break;
                        }
                    }
                } else {
                    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
                    match state.notification_events(method, &params, &peer_id) {
                        Ok(events) => {
                            for event in events {
                                state.emit(event);
                            }
                        }
                        Err(error) => {
                            fail_peer(&peer, &state, error.to_string()).await;
                            break;
                        }
                    }
                }
            }
            Incoming::Failure(detail) => {
                state.fail(detail);
                break;
            }
            Incoming::Closed => break,
        }
    }
    if let Err(error) = state.clear_peer_state(&peer_id) {
        state.fail(error.to_string());
    }
}

async fn fail_peer(peer: &Weak<JsonRpcPeer>, state: &CodexState, detail: String) {
    state.fail(detail.clone());
    if let Some(peer) = peer.upgrade() {
        peer.invalidate(detail).await;
    }
}

fn thread_to_summary(
    thread: CodexThread,
    profile_id: &str,
) -> Result<SessionSummary, AdapterError> {
    let title = thread_title(&thread);
    let status = thread_status(&thread.status)?.to_owned();
    Ok(SessionSummary {
        session_id: thread.id,
        project_path: thread.cwd,
        title,
        status,
        credential_profile_id: profile_id.to_owned(),
        created_at_ms: thread.created_at.saturating_mul(1000),
    })
}

fn thread_title(thread: &CodexThread) -> String {
    thread
        .name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| (!thread.preview.trim().is_empty()).then_some(thread.preview.as_str()))
        .unwrap_or("Untitled thread")
        .to_owned()
}

fn thread_status(status: &Value) -> Result<&str, AdapterError> {
    status.get("type").and_then(Value::as_str).ok_or_else(|| {
        AdapterError::Protocol("Codex thread status had no string type".into())
    })
}

fn required_string<'a>(
    value: &'a Value,
    field: &str,
    operation: &str,
) -> Result<&'a str, AdapterError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        AdapterError::Protocol(format!("Codex {operation} had no string {field}"))
    })
}

fn timestamp_seconds_to_ms(seconds: Option<i64>) -> i64 {
    seconds
        .map(|value| value.saturating_mul(1000))
        .unwrap_or_else(now_ms)
}

fn path_name(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
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

fn session_event(
    session_id: &str,
    title: &str,
    status: &str,
    project_path: &str,
    profile_id: &str,
    timestamp_ms: i64,
) -> Event {
    new_event(
        timestamp_ms,
        event::Inner::SessionUpdated(SessionUpdatedEvent {
            session_id: session_id.to_owned(),
            runtime_id: String::new(),
            title: title.to_owned(),
            status: status.to_owned(),
            credential_profile_id: profile_id.to_owned(),
            updated_at_ms: timestamp_ms,
            project_path: project_path.to_owned(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use tokio::io::{duplex, split, AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::sync::Notify;

    fn sample_thread() -> Value {
        json!({
            "id": "thread-1",
            "sessionId": "session-tree-1",
            "preview": "Initial prompt",
            "name": "Build feature",
            "cwd": "/srv/app",
            "cliVersion": "0.1.0",
            "modelProvider": "openai",
            "source": "appServer",
            "threadSource": null,
            "status": {"type": "idle"},
            "turns": [],
            "createdAt": 10,
            "updatedAt": 20,
            "ephemeral": false
        })
    }

    #[test]
    fn maps_thread_and_notification_state() {
        let adapter = CodexAdapter::new("codex");
        let thread: CodexThread = serde_json::from_value(sample_thread()).unwrap();
        let summary = thread_to_summary(thread, "").unwrap();
        assert_eq!(summary.session_id, "thread-1");
        assert_eq!(summary.project_path, "/srv/app");
        assert_eq!(summary.title, "Build feature");
        assert_eq!(summary.created_at_ms, 10_000);

        let events = adapter
            .state
            .notification_events(
                "thread/started",
                &json!({"thread": sample_thread()}),
                "peer-1",
            )
            .unwrap();
        assert!(matches!(
            events[0].inner,
            Some(event::Inner::SessionUpdated(SessionUpdatedEvent {
                ref session_id,
                ref project_path,
                ..
            })) if session_id == "thread-1" && project_path == "/srv/app"
        ));

        let deltas = adapter
            .state
            .notification_events(
                "item/agentMessage/delta",
                &json!({
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "itemId": "item-1",
                    "delta": "hello"
                }),
                "peer-1",
            )
            .unwrap();
        assert!(matches!(
            deltas[0].inner,
            Some(event::Inner::StreamDelta(StreamDeltaEvent {
                ref delta_text,
                is_final: false,
                ..
            })) if delta_text == "hello"
        ));
    }

    #[tokio::test]
    async fn routes_approval_request_and_response() {
        let (client_io, server_io) = duplex(8192);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let (peer, incoming) = JsonRpcPeer::from_io(client_reader, client_writer);
        let allow_request = Arc::new(Notify::new());
        let server_allow = Arc::clone(&allow_request);
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let initialize: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            server_writer
                .write_all(
                    format!("{{\"id\":{},\"result\":{{}}}}\n", initialize["id"]).as_bytes(),
                )
                .await
                .unwrap();
            let initialized: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(initialized["method"], "initialized");
            server_allow.notified().await;
            server_writer
                .write_all(
                    br#"{"id":"rpc-approval-1","method":"item/commandExecution/requestApproval","params":{"threadId":"thread-1","turnId":"turn-1","itemId":"item-1","startedAtMs":42,"command":"cargo test","reason":"Run tests"}}"#,
                )
                .await
                .unwrap();
            server_writer.write_all(b"\n").await.unwrap();
            let response: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(response["id"], "rpc-approval-1");
            assert_eq!(response["result"]["decision"], "accept");
        });

        peer.initialize().await.unwrap();
        let adapter = CodexAdapter::new("codex");
        *adapter.client.lock().await = Some(Arc::clone(&peer));
        let state = Arc::clone(&adapter.state);
        let reader_peer = Arc::downgrade(&peer);
        let peer_id = peer.instance_id().to_owned();
        tokio::spawn(async move {
            run_incoming(reader_peer, peer_id, state, incoming).await;
        });
        let mut events = adapter.subscribe_events().await.unwrap();
        allow_request.notify_one();
        let approval = events.next().await.unwrap().unwrap();
        let approval_id = match approval.inner {
            Some(event::Inner::ApprovalRequested(ApprovalRequestedEvent {
                approval_id,
                session_id,
                action_type,
                ..
            })) => {
                assert_eq!(session_id, "thread-1");
                assert_eq!(action_type, "command");
                approval_id
            }
            other => panic!("unexpected event: {other:?}"),
        };
        adapter
            .respond_approval("thread-1", &approval_id, true, "approved")
            .await
            .unwrap();
        server.await.unwrap();
        adapter.shutdown_gracefully().await.unwrap();
    }

    #[tokio::test]
    async fn external_adapter_rejects_profile_bound_start_before_process_spawn() {
        let adapter = CodexAdapter::new("definitely-not-a-command");
        assert!(matches!(
            adapter
                .start_session("/srv/app", "build it", "account-a")
                .await,
            Err(AdapterError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn credential_mutation_remains_fail_closed() {
        let adapter = CodexAdapter::new("codex");
        let credential = CredentialMaterial::api_key("openai", b"test-key").unwrap();
        assert!(matches!(
            adapter.activate_credential("profile", &credential).await,
            Err(AdapterError::Unsupported(_))
        ));
    }

    #[tokio::test]
    async fn managed_profile_uses_supported_account_endpoints() {
        let (client_io, server_io) = duplex(8192);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let (peer, _incoming) = JsonRpcPeer::from_io(client_reader, client_writer);
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let initialize: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            server_writer
                .write_all(
                    format!("{{\"id\":{},\"result\":{{}}}}\n", initialize["id"]).as_bytes(),
                )
                .await
                .unwrap();
            let _: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();

            let read: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(read["method"], "account/read");
            assert_eq!(read["params"]["refreshToken"], false);
            server_writer
                .write_all(
                    format!(
                        "{{\"id\":{},\"result\":{{\"account\":null,\"requiresOpenaiAuth\":true}}}}\n",
                        read["id"]
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();

            let login: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(login["method"], "account/login/start");
            assert_eq!(login["params"]["type"], "apiKey");
            assert_eq!(login["params"]["apiKey"], "test-secret-key");
            server_writer
                .write_all(
                    format!(
                        "{{\"id\":{},\"result\":{{\"type\":\"apiKey\"}}}}\n",
                        login["id"]
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();

            let limits: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(limits["method"], "account/rateLimits/read");
            server_writer
                .write_all(
                    format!(
                        "{{\"id\":{},\"result\":{{\"rateLimits\":{{}}}}}}\n",
                        limits["id"]
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });

        peer.initialize().await.unwrap();
        let adapter = CodexAdapter::new_managed(
            ProcessConfig::inherited("codex"),
            "profile-a".into(),
        );
        *adapter.client.lock().await = Some(Arc::clone(&peer));
        assert_eq!(adapter.read_account(false).await.unwrap().account, None);
        let credential =
            CredentialMaterial::api_key("openai", b"test-secret-key").unwrap();
        adapter
            .activate_credential("profile-a", &credential)
            .await
            .unwrap();
        assert_eq!(
            adapter.read_rate_limits().await.unwrap(),
            json!({"rateLimits": {}})
        );
        server.await.unwrap();
        adapter.shutdown_gracefully().await.unwrap();
    }

    #[test]
    fn tracks_account_and_rate_limit_notifications_without_exposing_tokens() {
        let adapter = CodexAdapter::new_managed(
            ProcessConfig::inherited("codex"),
            "profile-a".into(),
        );
        adapter
            .state
            .notification_events(
                "account/updated",
                &json!({"authMode": "chatgpt", "planType": "plus"}),
                "peer-a",
            )
            .unwrap();
        assert!(matches!(
            adapter.state.account.read().unwrap().as_ref(),
            Some(CodexAccount::Chatgpt {
                plan_type: Some(plan),
                ..
            }) if plan == "plus"
        ));
        adapter
            .state
            .notification_events(
                "account/rateLimits/updated",
                &json!({"rateLimits": {"limitId": "codex"}}),
                "peer-a",
            )
            .unwrap();
        assert_eq!(
            adapter.state.rate_limits.read().unwrap().as_ref(),
            Some(&json!({"rateLimits": {"limitId": "codex"}}))
        );
    }

    #[test]
    fn normalizes_usage_without_forwarding_untrusted_provider_fields() {
        let response = json!({
            "accessToken": "must-not-leak",
            "rateLimitsByLimitId": {
                "codex": {
                    "limitId": "codex",
                    "primary": {
                        "usedPercent": 137,
                        "resetsAt": 1_700_000_000,
                        "windowDurationMins": 300,
                        "secret": "must-not-leak"
                    },
                    "secondary": {"usedPercent": -4, "resetsAt": -1}
                }
            }
        });
        let usage = normalized_usage_buckets(&response);
        assert_eq!(
            usage,
            vec![UsageBucket {
                bucket_id: Some("codex".into()),
                primary: Some(UsageWindow {
                    used_percent: 100,
                    resets_at_unix_seconds: Some(1_700_000_000),
                    duration_minutes: Some(300),
                }),
                secondary: Some(UsageWindow {
                    used_percent: 0,
                    resets_at_unix_seconds: None,
                    duration_minutes: None,
                }),
            }]
        );
        let encoded = serde_json::to_string(&usage).unwrap();
        assert!(!encoded.contains("accessToken"));
        assert!(!encoded.contains("must-not-leak"));
    }

    #[test]
    fn caps_and_sorts_usage_buckets() {
        let mut buckets = serde_json::Map::new();
        for index in (0..40).rev() {
            buckets.insert(
                format!("bucket-{index:02}"),
                json!({"primary": {"usedPercent": index}}),
            );
        }
        let usage = normalized_usage_buckets(&json!({"rateLimitsByLimitId": buckets}));
        assert_eq!(usage.len(), MAX_USAGE_BUCKETS);
        assert_eq!(usage[0].bucket_id.as_deref(), Some("bucket-00"));
        assert_eq!(usage[31].bucket_id.as_deref(), Some("bucket-31"));
    }

    #[test]
    fn app_server_start_guard_latches_rapid_crash_loops() {
        let mut guard = AppServerStartGuard::default();
        for _ in 0..MAX_APP_SERVER_STARTS_PER_WINDOW {
            guard.before_start().unwrap();
        }
        assert!(matches!(
            guard.before_start(),
            Err(AdapterError::InitFailed(detail)) if detail.contains("crash loop")
        ));
        assert!(matches!(
            guard.before_start(),
            Err(AdapterError::InitFailed(detail)) if detail.contains("operator restart")
        ));
    }

    #[test]
    fn generated_account_contract_fixture_matches_supported_shapes() {
        let login_schema: Value = serde_json::from_str(include_str!(
            "../fixtures/0.146.0/v2/LoginAccountParams.json"
        ))
        .unwrap();
        let login_types = login_schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|shape| shape["properties"]["type"]["enum"][0].as_str())
            .collect::<HashSet<_>>();
        assert!(login_types.contains("apiKey"));
        assert!(login_types.contains("chatgpt"));
        assert!(login_types.contains("chatgptDeviceCode"));

        let account_schema: Value = serde_json::from_str(include_str!(
            "../fixtures/0.146.0/v2/GetAccountResponse.json"
        ))
        .unwrap();
        assert_eq!(account_schema["title"], "GetAccountResponse");
        assert!(account_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "requiresOpenaiAuth"));

        let current_bedrock: CodexAccountRead = serde_json::from_value(json!({
            "account": {
                "type": "amazonBedrock",
                "usesCodexManagedCredentials": true
            },
            "requiresOpenaiAuth": false
        }))
        .unwrap();
        assert!(matches!(
            current_bedrock.account,
            Some(CodexAccount::AmazonBedrock {
                uses_codex_managed_credentials: Some(true),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn login_completion_is_restart_safe_against_notification_races() {
        let adapter = CodexAdapter::new_managed(
            ProcessConfig::inherited("codex"),
            "profile-a".into(),
        );
        adapter
            .state
            .notification_events(
                "account/login/completed",
                &json!({
                    "loginId": "login-a",
                    "success": true,
                    "error": null
                }),
                "peer-a",
            )
            .unwrap();
        adapter
            .wait_for_login_completion("login-a", Duration::from_millis(10))
            .await
            .unwrap();

        adapter
            .state
            .notification_events(
                "account/login/completed",
                &json!({
                    "loginId": "login-b",
                    "success": false,
                    "error": "device code expired"
                }),
                "peer-a",
            )
            .unwrap();
        assert!(matches!(
            adapter
                .wait_for_login_completion("login-b", Duration::from_millis(10))
                .await,
            Err(AdapterError::CredentialInvalid(detail)) if detail == "device code expired"
        ));
    }
}
