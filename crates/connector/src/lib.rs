mod command_router;
mod pairing_store;
mod secure_session;

pub use command_router::{CommandDispatchError, CommandRouter};
pub use pairing_store::{
    ConfirmingParty, PairingStore, PairingStoreError,
};
pub use secure_session::{
    AuthenticatedCommand, SecureEnvelopeSession, SecureSessionError,
    CURRENT_PROTOCOL_VERSION, DEFAULT_MAX_PLAINTEXT_BYTES,
};

use adapter_api::{ProjectInfo, SessionSummary};
use event_journal::{EventJournal, JournalError};
use muxport_protocol::{
    event, AgentType, ConnectorState, Event, HostSnapshot, RuntimeInfo, RuntimeState, SessionInfo,
};

/// In-memory projection of authoritative runtime snapshots plus journaled
/// normalized events. It contains no credentials or provider secrets.
pub struct RuntimeMirror {
    snapshot: HostSnapshot,
}

impl RuntimeMirror {
    pub fn new(
        host_id: impl Into<String>,
        hostname: impl Into<String>,
        connector_state: ConnectorState,
        snapshot_sequence: u64,
    ) -> Self {
        Self {
            snapshot: HostSnapshot {
                host_id: host_id.into(),
                hostname: hostname.into(),
                connector_state: connector_state as i32,
                runtimes: Vec::new(),
                credential_profiles: Vec::new(),
                active_sessions: Vec::new(),
                snapshot_sequence,
            },
        }
    }

    pub fn from_snapshot(snapshot: HostSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn set_connector_state(&mut self, state: ConnectorState) {
        self.snapshot.connector_state = state as i32;
    }

    /// Replaces one runtime's cached projection with an authoritative source
    /// snapshot. Source disappearance removes stale sessions instead of
    /// preserving them from cache.
    pub fn reconcile_runtime(
        &mut self,
        runtime_id: &str,
        agent_type: AgentType,
        runtime_name: &str,
        projects: Vec<ProjectInfo>,
        sessions: Vec<SessionSummary>,
    ) {
        let mut project_paths = projects
            .into_iter()
            .map(|project| project.path)
            .collect::<Vec<_>>();
        project_paths.sort();
        project_paths.dedup();

        let runtime_state = observed_runtime_state(&sessions);
        self.snapshot
            .active_sessions
            .retain(|session| session.runtime_id != runtime_id);
        self.snapshot
            .active_sessions
            .extend(sessions.into_iter().map(|session| SessionInfo {
                session_id: session.session_id,
                runtime_id: runtime_id.to_owned(),
                project_path: session.project_path,
                title: session.title,
                credential_profile_id: session.credential_profile_id,
                status: session.status,
            }));
        self.snapshot
            .active_sessions
            .sort_by(|left, right| left.session_id.cmp(&right.session_id));

        match self
            .snapshot
            .runtimes
            .iter_mut()
            .find(|runtime| runtime.runtime_id == runtime_id)
        {
            Some(runtime) => {
                runtime.agent_type = agent_type as i32;
                runtime.name = runtime_name.to_owned();
                runtime.state = runtime_state as i32;
                runtime.project_paths = project_paths;
            }
            None => self.snapshot.runtimes.push(RuntimeInfo {
                runtime_id: runtime_id.to_owned(),
                agent_type: agent_type as i32,
                name: runtime_name.to_owned(),
                state: runtime_state as i32,
                active_credential_profile_id: String::new(),
                project_paths,
            }),
        }
        self.snapshot
            .runtimes
            .sort_by(|left, right| left.runtime_id.cmp(&right.runtime_id));
    }

    pub fn mark_runtime_state(
        &mut self,
        runtime_id: &str,
        agent_type: AgentType,
        runtime_name: &str,
        state: RuntimeState,
    ) {
        match self
            .snapshot
            .runtimes
            .iter_mut()
            .find(|runtime| runtime.runtime_id == runtime_id)
        {
            Some(runtime) => runtime.state = state as i32,
            None => self.snapshot.runtimes.push(RuntimeInfo {
                runtime_id: runtime_id.to_owned(),
                agent_type: agent_type as i32,
                name: runtime_name.to_owned(),
                state: state as i32,
                active_credential_profile_id: String::new(),
                project_paths: Vec::new(),
            }),
        }
        self.snapshot.connector_state = if state == RuntimeState::Degraded
            || state == RuntimeState::Crashed
            || state == RuntimeState::CrashLoop
        {
            ConnectorState::Degraded as i32
        } else {
            self.snapshot.connector_state
        };
    }

    /// Adds the runtime correlation missing from a vendor-neutral adapter
    /// before the event receives its durable connector sequence.
    pub fn correlate_event(event: &mut Event, runtime_id: &str) {
        if let Some(event::Inner::SessionUpdated(session)) = event.inner.as_mut() {
            session.runtime_id = runtime_id.to_owned();
        }
    }

    /// Applies an event only after it has been durably appended.
    pub fn apply_event(&mut self, runtime_id: &str, event: &Event) {
        match event.inner.as_ref() {
            Some(event::Inner::SessionUpdated(update)) => {
                if update.status == "deleted" {
                    self.snapshot
                        .active_sessions
                        .retain(|session| session.session_id != update.session_id);
                    return;
                }
                match self
                    .snapshot
                    .active_sessions
                    .iter_mut()
                    .find(|session| session.session_id == update.session_id)
                {
                    Some(session) => {
                        if !update.title.is_empty() {
                            session.title.clone_from(&update.title);
                        }
                        if !update.project_path.is_empty() {
                            session.project_path.clone_from(&update.project_path);
                        }
                        if !update.credential_profile_id.is_empty() {
                            session
                                .credential_profile_id
                                .clone_from(&update.credential_profile_id);
                        }
                        if !update.status.is_empty() && update.status != "unknown" {
                            session.status.clone_from(&update.status);
                        }
                    }
                    None => self.snapshot.active_sessions.push(SessionInfo {
                        session_id: update.session_id.clone(),
                        runtime_id: runtime_id.to_owned(),
                        project_path: update.project_path.clone(),
                        title: update.title.clone(),
                        credential_profile_id: update.credential_profile_id.clone(),
                        status: update.status.clone(),
                    }),
                }
                if let Some(state) = runtime_state_for_session_status(&update.status) {
                    self.set_existing_runtime_state(runtime_id, state);
                }
            }
            Some(event::Inner::StreamDelta(delta)) if !delta.is_final => {
                if let Some(session) = self
                    .snapshot
                    .active_sessions
                    .iter_mut()
                    .find(|session| session.session_id == delta.session_id)
                {
                    session.status = "running".into();
                }
                self.set_existing_runtime_state(runtime_id, RuntimeState::OnlineRunning);
            }
            Some(event::Inner::ApprovalRequested(approval)) => {
                if let Some(session) = self
                    .snapshot
                    .active_sessions
                    .iter_mut()
                    .find(|session| session.session_id == approval.session_id)
                {
                    session.status = "waiting_approval".into();
                }
                self.set_existing_runtime_state(runtime_id, RuntimeState::OnlineWaitingApproval);
            }
            Some(event::Inner::ApprovalResolved(approval)) => {
                if let Some(session) = self
                    .snapshot
                    .active_sessions
                    .iter_mut()
                    .find(|session| session.session_id == approval.session_id)
                {
                    session.status = "running".into();
                }
                self.set_existing_runtime_state(runtime_id, RuntimeState::OnlineRunning);
            }
            Some(event::Inner::RuntimeState(runtime)) if runtime.runtime_id == runtime_id => {
                if let Ok(state) = RuntimeState::try_from(runtime.state) {
                    let agent_type = AgentType::try_from(runtime.agent_type)
                        .unwrap_or(AgentType::Unspecified);
                    self.mark_runtime_state(runtime_id, agent_type, runtime_id, state);
                }
            }
            _ => {}
        }
        self.snapshot
            .active_sessions
            .sort_by(|left, right| left.session_id.cmp(&right.session_id));
    }

    pub fn apply_replayed_event(&mut self, event: &Event) {
        let runtime_id = match event.inner.as_ref() {
            Some(event::Inner::SessionUpdated(update)) if !update.runtime_id.is_empty() => {
                Some(update.runtime_id.clone())
            }
            Some(event::Inner::StreamDelta(delta)) => self
                .snapshot
                .active_sessions
                .iter()
                .find(|session| session.session_id == delta.session_id)
                .map(|session| session.runtime_id.clone()),
            Some(event::Inner::ApprovalRequested(approval)) => self
                .snapshot
                .active_sessions
                .iter()
                .find(|session| session.session_id == approval.session_id)
                .map(|session| session.runtime_id.clone()),
            Some(event::Inner::ApprovalResolved(approval)) => self
                .snapshot
                .active_sessions
                .iter()
                .find(|session| session.session_id == approval.session_id)
                .map(|session| session.runtime_id.clone()),
            Some(event::Inner::RuntimeState(runtime)) if !runtime.runtime_id.is_empty() => {
                Some(runtime.runtime_id.clone())
            }
            _ => None,
        };
        if let Some(runtime_id) = runtime_id {
            self.apply_event(&runtime_id, event);
        }
    }

    pub fn snapshot_at(&self, sequence: u64) -> HostSnapshot {
        let mut snapshot = self.snapshot.clone();
        snapshot.snapshot_sequence = sequence;
        snapshot
    }

    pub fn save_snapshot(&self, journal: &EventJournal) -> Result<(), JournalError> {
        journal.save_snapshot(&self.snapshot_at(journal.current_sequence()))
    }

    pub fn connector_state(&self) -> ConnectorState {
        ConnectorState::try_from(self.snapshot.connector_state)
            .unwrap_or(ConnectorState::Unspecified)
    }

    pub fn snapshot_sequence(&self) -> u64 {
        self.snapshot.snapshot_sequence
    }

    fn set_existing_runtime_state(&mut self, runtime_id: &str, state: RuntimeState) {
        if let Some(runtime) = self
            .snapshot
            .runtimes
            .iter_mut()
            .find(|runtime| runtime.runtime_id == runtime_id)
        {
            runtime.state = state as i32;
        }
    }
}

/// Durability ordering for source events: correlate, append, project, snapshot.
/// If append fails, the in-memory projection remains unchanged.
pub fn journal_runtime_event(
    journal: &mut EventJournal,
    mirror: &mut RuntimeMirror,
    runtime_id: &str,
    mut event: Event,
    save_snapshot: bool,
) -> Result<u64, JournalError> {
    RuntimeMirror::correlate_event(&mut event, runtime_id);
    let sequence = journal.append_event(&event)?;
    mirror.apply_event(runtime_id, &event);
    if save_snapshot {
        mirror.save_snapshot(journal)?;
    }
    Ok(sequence)
}

pub fn replay_events_after_snapshot(
    journal: &EventJournal,
    mirror: &mut RuntimeMirror,
) -> Result<usize, JournalError> {
    let mut cursor = mirror.snapshot_sequence();
    let mut replayed = 0;
    loop {
        let events = journal.get_events_after(cursor, 256)?;
        if events.is_empty() {
            return Ok(replayed);
        }
        for (sequence, event) in events {
            mirror.apply_replayed_event(&event);
            cursor = sequence;
            replayed += 1;
        }
    }
}

fn observed_runtime_state(sessions: &[SessionSummary]) -> RuntimeState {
    if sessions
        .iter()
        .any(|session| session.status == "waiting_approval")
    {
        RuntimeState::OnlineWaitingApproval
    } else if sessions
        .iter()
        .any(|session| session.status == "systemError")
    {
        RuntimeState::Degraded
    } else if sessions
        .iter()
        .any(|session| {
            matches!(
                session.status.as_str(),
                "busy" | "running" | "active" | "inProgress"
            )
        })
    {
        RuntimeState::OnlineRunning
    } else {
        RuntimeState::OnlineIdle
    }
}

fn runtime_state_for_session_status(status: &str) -> Option<RuntimeState> {
    match status {
        "busy" | "running" | "active" | "inProgress" => {
            Some(RuntimeState::OnlineRunning)
        }
        "idle" | "completed" | "interrupted" | "failed" | "notLoaded" | "archived"
        | "closed" => Some(RuntimeState::OnlineIdle),
        "waiting_approval" => Some(RuntimeState::OnlineWaitingApproval),
        "systemError" => Some(RuntimeState::Degraded),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxport_protocol::{
        ApprovalResolvedEvent, SessionUpdatedEvent, StreamDeltaEvent,
    };

    fn session(id: &str, project_path: &str, title: &str, status: &str) -> SessionSummary {
        SessionSummary {
            session_id: id.into(),
            project_path: project_path.into(),
            title: title.into(),
            status: status.into(),
            credential_profile_id: String::new(),
            created_at_ms: 1,
        }
    }

    #[test]
    fn authoritative_reconcile_replaces_stale_runtime_sessions() {
        let mut mirror =
            RuntimeMirror::new("host", "hostname", ConnectorState::Recovering, 0);
        mirror.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![ProjectInfo {
                path: "/old".into(),
                name: "old".into(),
            }],
            vec![session("stale", "/old", "Stale", "idle")],
        );
        mirror.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![ProjectInfo {
                path: "/new".into(),
                name: "new".into(),
            }],
            vec![session("current", "/new", "Current", "busy")],
        );

        let snapshot = mirror.snapshot_at(0);
        assert_eq!(snapshot.active_sessions.len(), 1);
        assert_eq!(snapshot.active_sessions[0].session_id, "current");
        assert_eq!(
            RuntimeState::try_from(snapshot.runtimes[0].state).unwrap(),
            RuntimeState::OnlineRunning
        );
    }

    #[test]
    fn partial_status_event_preserves_authoritative_title_and_path() {
        let mut mirror =
            RuntimeMirror::new("host", "hostname", ConnectorState::Recovering, 0);
        mirror.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![],
            vec![session("session-1", "/repo", "Existing", "idle")],
        );
        let mut event = Event {
            event_id: "event-1".into(),
            timestamp_ms: 2,
            inner: Some(event::Inner::SessionUpdated(SessionUpdatedEvent {
                session_id: "session-1".into(),
                runtime_id: String::new(),
                title: String::new(),
                status: "busy".into(),
                credential_profile_id: String::new(),
                updated_at_ms: 2,
                project_path: String::new(),
            })),
        };
        RuntimeMirror::correlate_event(&mut event, "opencode-1");
        mirror.apply_event("opencode-1", &event);

        let snapshot = mirror.snapshot_at(1);
        assert_eq!(snapshot.active_sessions[0].title, "Existing");
        assert_eq!(snapshot.active_sessions[0].project_path, "/repo");
        assert_eq!(snapshot.active_sessions[0].status, "busy");
    }

    #[test]
    fn journal_append_precedes_projection_and_snapshot() {
        let mut journal = EventJournal::open_in_memory(7).unwrap();
        let mut mirror =
            RuntimeMirror::new("host", "hostname", ConnectorState::Recovering, 0);
        mirror.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![],
            vec![session("session-1", "/repo", "Existing", "idle")],
        );
        let event = Event {
            event_id: "event-1".into(),
            timestamp_ms: 2,
            inner: Some(event::Inner::StreamDelta(StreamDeltaEvent {
                session_id: "session-1".into(),
                turn_id: "turn-1".into(),
                delta_text: "hello".into(),
                is_final: false,
            })),
        };

        let sequence =
            journal_runtime_event(&mut journal, &mut mirror, "opencode-1", event, true).unwrap();
        assert_eq!(sequence, 1);
        assert_eq!(mirror.snapshot_at(sequence).active_sessions[0].status, "running");
        assert_eq!(
            journal.latest_snapshot().unwrap().unwrap().snapshot_sequence,
            sequence
        );
    }

    #[test]
    fn restart_replays_events_newer_than_last_snapshot() {
        let mut journal = EventJournal::open_in_memory(7).unwrap();
        let mut live =
            RuntimeMirror::new("host", "hostname", ConnectorState::Recovering, 0);
        live.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![],
            vec![session("session-1", "/repo", "Existing", "idle")],
        );
        live.save_snapshot(&journal).unwrap();
        let event = Event {
            event_id: "event-after-snapshot".into(),
            timestamp_ms: 2,
            inner: Some(event::Inner::StreamDelta(StreamDeltaEvent {
                session_id: "session-1".into(),
                turn_id: "turn-1".into(),
                delta_text: "hello".into(),
                is_final: false,
            })),
        };
        journal_runtime_event(&mut journal, &mut live, "opencode-1", event, false).unwrap();

        let snapshot = journal.latest_snapshot().unwrap().unwrap();
        let mut recovered = RuntimeMirror::from_snapshot(snapshot);
        assert_eq!(
            replay_events_after_snapshot(&journal, &mut recovered).unwrap(),
            1
        );
        assert_eq!(
            recovered.snapshot_at(1).active_sessions[0].status,
            "running"
        );
    }

    #[test]
    fn failed_journal_append_does_not_mutate_projection() {
        let mut journal = EventJournal::open_in_memory(7).unwrap();
        let mut mirror =
            RuntimeMirror::new("host", "hostname", ConnectorState::Recovering, 0);
        mirror.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![],
            vec![session("session-1", "/repo", "Existing", "idle")],
        );
        let event = Event {
            event_id: "duplicate-event".into(),
            timestamp_ms: 2,
            inner: Some(event::Inner::StreamDelta(StreamDeltaEvent {
                session_id: "session-1".into(),
                turn_id: "turn-1".into(),
                delta_text: "hello".into(),
                is_final: false,
            })),
        };
        journal.append_event(&event).unwrap();

        assert!(journal_runtime_event(
            &mut journal,
            &mut mirror,
            "opencode-1",
            event,
            false,
        )
        .is_err());
        assert_eq!(mirror.snapshot_at(1).active_sessions[0].status, "idle");
    }

    #[test]
    fn runtime_reconciliation_isolated_between_opencode_and_codex() {
        let mut mirror =
            RuntimeMirror::new("host", "hostname", ConnectorState::Recovering, 0);
        mirror.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![],
            vec![session("open-session", "/open", "Open", "idle")],
        );
        mirror.reconcile_runtime(
            "codex-1",
            AgentType::Codex,
            "Codex",
            vec![],
            vec![session("codex-session", "/codex", "Codex", "active")],
        );
        mirror.reconcile_runtime(
            "opencode-1",
            AgentType::Opencode,
            "OpenCode",
            vec![],
            vec![session("open-new", "/open", "Open new", "busy")],
        );

        let snapshot = mirror.snapshot_at(0);
        assert_eq!(snapshot.active_sessions.len(), 2);
        assert!(snapshot
            .active_sessions
            .iter()
            .any(|session| session.session_id == "codex-session"));
        assert!(snapshot
            .active_sessions
            .iter()
            .any(|session| session.session_id == "open-new"));
        assert!(!snapshot
            .active_sessions
            .iter()
            .any(|session| session.session_id == "open-session"));
        let codex = snapshot
            .runtimes
            .iter()
            .find(|runtime| runtime.runtime_id == "codex-1")
            .unwrap();
        assert_eq!(
            RuntimeState::try_from(codex.state).unwrap(),
            RuntimeState::OnlineRunning
        );
    }

    #[test]
    fn codex_completion_and_approval_resolution_update_runtime_state() {
        let mut mirror =
            RuntimeMirror::new("host", "hostname", ConnectorState::Recovering, 0);
        mirror.reconcile_runtime(
            "codex-1",
            AgentType::Codex,
            "Codex",
            vec![],
            vec![session(
                "codex-session",
                "/codex",
                "Codex",
                "waiting_approval",
            )],
        );
        let resolved = Event {
            event_id: "resolved".into(),
            timestamp_ms: 2,
            inner: Some(event::Inner::ApprovalResolved(ApprovalResolvedEvent {
                approval_id: "approval-1".into(),
                approved: true,
                resolved_by: "muxport".into(),
                session_id: "codex-session".into(),
            })),
        };
        mirror.apply_event("codex-1", &resolved);
        assert_eq!(
            mirror.snapshot_at(0).active_sessions[0].status,
            "running"
        );

        let completed = Event {
            event_id: "completed".into(),
            timestamp_ms: 3,
            inner: Some(event::Inner::SessionUpdated(SessionUpdatedEvent {
                session_id: "codex-session".into(),
                runtime_id: "codex-1".into(),
                title: String::new(),
                status: "completed".into(),
                credential_profile_id: String::new(),
                updated_at_ms: 3,
                project_path: String::new(),
            })),
        };
        mirror.apply_event("codex-1", &completed);
        let snapshot = mirror.snapshot_at(0);
        assert_eq!(snapshot.active_sessions[0].status, "completed");
        assert_eq!(
            RuntimeState::try_from(snapshot.runtimes[0].state).unwrap(),
            RuntimeState::OnlineIdle
        );
    }
}
