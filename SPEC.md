# Muxport — Draft Product Specification

Status: discussion draft  
Date: 2026-07-30  
Working name: **Muxport**

## 1. Product statement

Muxport is a mobile-first control center for managing OpenCode and Codex across multiple computers and servers. It mirrors live agent state, lets the user operate sessions remotely, and routes each runtime or new session through the selected account or API-key profile.

The product is not a mobile IDE and does not execute coding work on the phone. Code, tools, credentials, and agent processes remain on user-controlled hosts.

## 2. Core problem

A user may have:

- several desktops, laptops, VPSs, or home servers;
- OpenCode and Codex installed on more than one host;
- multiple OpenCode Go subscriptions or provider keys;
- multiple Codex identities, including ChatGPT accounts and API keys;
- different account assignments for personal, work, and client projects;
- a need to change one host or every host to another credential with one action.

Existing remote-agent applications mainly manage sessions. Muxport makes **multi-host identity routing and credential rotation** a first-class feature.

## 3. Goals

### MVP goals

- Pair multiple user-controlled hosts with one mobile app.
- Register one or more OpenCode and Codex runtimes per host.
- Show actual projects, threads/sessions, messages, running state, tool activity, diffs, approvals, errors, and completion state in near real time.
- Start, resume, steer, interrupt, and approve supported agent actions.
- Create labeled credential profiles without exposing plaintext secrets in ordinary UI or relay traffic.
- Assign a default credential profile by runtime and optionally by project.
- Switch one runtime, selected runtimes, or all compatible runtimes to another profile with one action.
- Bind every session to a visible credential profile so the active identity is never ambiguous.
- Rotate authorized OpenCode Go keys manually and through an explicit opt-in policy.
- Reconcile state after phone suspension, connector restart, or network loss.

### Later goals

- Push notifications for approvals, questions, completion, failure, and credential exhaustion.
- Usage and rate-limit dashboards when providers expose reliable data.
- Scheduled rotation, failover, cooldown, and budget policies.
- Tablet, desktop, and web clients using the same protocol.
- Additional coding agents through adapters.

## 4. Product model

| Entity | Meaning |
|---|---|
| Host | A paired desktop, laptop, VPS, or server |
| Connector | The local Muxport service running on a host |
| Runtime | One managed OpenCode or Codex installation/profile |
| Project | A repository or working directory exposed by a runtime |
| Credential profile | A labeled OpenCode Go key, provider key, Codex API key, or Codex account login |
| Assignment | The default credential profile for a runtime or project |
| Session binding | The immutable record of which profile started a session |
| Rotation policy | Rules for selecting the next eligible credential profile |

Precedence for new sessions:

1. Explicit session selection.
2. Project assignment.
3. Runtime assignment.
4. Host default.
5. User-wide compatible default.

Changing a default does not silently rebind a running session.

## 5. Primary experience

### Home

- All hosts with online, sleeping, offline, or degraded status.
- OpenCode and Codex runtimes under each host.
- Counts for running, waiting, failed, and completed sessions.
- The active/default credential badge for every runtime.

### Session

- Live conversation and tool timeline.
- Incremental assistant output.
- Command, edit, MCP, and other approval cards.
- Current plan/todos, diff, test output, usage, and error state when available.
- Send prompt, steer, approve, reject, interrupt, or resume.
- Permanent “Running as” credential badge.

### Accounts

- Add and label OpenCode Go keys, provider keys, Codex API keys, and Codex account profiles.
- Show which hosts and projects use each profile.
- Disable, revoke, rotate, or replace a profile.
- Never display a stored secret again after enrollment.

### One-tap switching

The user selects a credential profile and chooses:

- Use for this runtime.
- Use for this project.
- Use on selected hosts.
- Use on all compatible runtimes.
- Use for the next session only.

The confirmation screen must show affected hosts, active sessions, and whether a runtime restart is required.

## 6. OpenCode Go rotation

OpenCode Go supplies an API key and behaves as an OpenCode provider. Muxport will treat each user-authorized Go key as a distinct credential profile.

Supported rotation modes:

- **Manual:** change the selected runtime or project immediately for new sessions.
- **Bulk:** apply one key to all selected OpenCode runtimes.
- **Failover:** move to the next eligible profile after a confirmed authentication or quota/rate-limit failure.
- **Round robin:** distribute newly created sessions among an ordered profile pool.
- **Scheduled:** change the default at a user-defined time.

Safety requirements:

- Rotation applies to new work by default.
- An active turn is never silently moved between identities.
- Immediate replacement requires explicit confirmation and may drain or restart the managed runtime.
- A profile can be paused, assigned a cooldown, or excluded from a host.
- Every switch records actor, time, source profile, destination profile, scope, reason, and result.
- The connector uses OpenCode's supported auth/provider APIs or supported process configuration; it must not rewrite OpenCode databases or session state.
- Only accounts and keys the user is authorized to operate are supported. Muxport must not present rotation as a way to evade provider restrictions.

Open question: determine whether parallel OpenCode Go accounts on one host can be isolated using supported runtime state directories. Until verified, MVP guarantees one active Go profile per managed OpenCode runtime; simultaneous profiles require separate managed runtimes.

## 7. Codex identity management

Codex App Server exposes supported account operations for API-key login, ChatGPT browser/device-code login, logout, account state, plan type, and rate limits.

Muxport will:

- start a local Codex App Server for each managed Codex runtime;
- use supported login methods rather than copying OAuth tokens or editing authentication state;
- keep ChatGPT account profiles isolated from each other;
- display the account/profile bound to every new thread;
- support one-tap assignment for new Codex sessions;
- keep API-key and ChatGPT-managed identities distinct.

Open question: validate the supported isolation mechanism for multiple concurrent Codex account profiles on one OS account. The likely design is one Codex state root and App Server process per profile, but the implementation must be proven against current Codex behavior before it becomes a requirement.

OpenAI currently does not support its multi-account switcher in Codex desktop or native ChatGPT mobile apps, making this an important Muxport use case.

## 8. Real-time synchronization

### Feasibility

Real-time sync is feasible for sessions managed through Muxport:

- **OpenCode:** HTTP/OpenAPI for snapshots and commands, plus `/event` or `/global/event` Server-Sent Events for live bus events.
- **Codex:** local App Server over its supported stdio JSON-RPC transport, providing thread, turn, item, output-delta, diff, approval, account, and usage notifications.

Codex's direct WebSocket listener is currently experimental and must not be the production dependency. The connector translates the stable local stdio stream into Muxport's encrypted transport.

### Sync algorithm

1. Connector obtains a complete source snapshot.
2. Connector subscribes to the source event stream.
3. Connector normalizes source events into a common envelope.
4. Connector assigns a host-scoped monotonic sequence number.
5. Phone applies ordered events and stores a bounded local cache.
6. After reconnect, phone requests events after its last acknowledged sequence.
7. If replay is unavailable or a gap is detected, connector sends a fresh snapshot and resumes streaming.

The host remains the source of truth. The mobile cache is disposable.

### Normalized event envelope

```text
event_id
host_id
runtime_id
project_id?
session_id?
source            # opencode | codex
source_version
sequence
timestamp
event_type
payload
```

### Important limitation

Muxport can guarantee full live control for sessions launched or adopted through its connector. Attaching to an arbitrary already-running OpenCode TUI or official Codex desktop session may not expose a stable attach point.

- OpenCode can be adopted when its server endpoint is known or it is launched in managed host/port mode.
- Codex persisted threads can be listed through App Server, but attaching to a turn already owned by another live Codex client requires a technical spike.

MVP should state this honestly: **managed sessions sync fully; externally launched live sessions are best-effort until attach behavior is verified.**

## 9. Proposed architecture

```text
Mobile app
  |
  | end-to-end encrypted session
  v
Optional relay (opaque routing + push only)
  |
  v
Muxport Connector on each host
  |-- OpenCode adapter -> OpenCode HTTP/OpenAPI + SSE
  |-- Codex adapter    -> Codex App Server JSON-RPC over stdio
  |-- Credential vault -> OS keychain / secure host storage
  `-- Event journal    -> bounded replay + reconciliation metadata
```

The connector initiates outbound connections so users do not need to expose inbound agent ports. Direct LAN or private-network mode should also be supported.

## 10. Security requirements

- Credentials stay on their assigned hosts unless the user explicitly provisions them elsewhere.
- A relay never receives plaintext source, prompts, output, credentials, or approval contents.
- Host enrollment uses QR pairing with short-lived, single-use material.
- Device keys are hardware-backed where available.
- Viewing credential metadata and changing assignments are separate permissions.
- Bulk switching, credential export/provisioning, and destructive actions require biometric or device-auth step-up.
- Connector logs redact tokens, headers, environment variables, and auth payloads.
- No credential, live database, WAL/SHM file, or unencrypted state backup enters source control.
- Revoking a phone or host invalidates its access without rotating unrelated provider keys.
- Secret replacement is atomic: store and verify the new secret before changing assignment; preserve the previous assignment until validation succeeds.

## 11. MVP boundaries

Included:

- iOS and Android client.
- Windows, macOS, and Linux connector.
- Multiple hosts.
- OpenCode and Codex adapters.
- Managed-session real-time sync and approvals.
- Credential profiles and visible session bindings.
- Manual and bulk OpenCode Go switching.
- Opt-in OpenCode Go failover/round-robin policy with audit history.
- Direct private-network mode and optional encrypted relay.

Not included:

- General-purpose remote desktop.
- Full mobile code editor or IDE.
- Automatic migration of active turns between credentials.
- Sharing personal subscription credentials with other people.
- Editing internal OpenCode/Codex databases or token files.
- Guaranteed attachment to sessions owned by unrelated live clients.

## 12. Validation spikes before implementation

1. Connect to OpenCode SSE, reconstruct a complete live session, disconnect, and reconcile without missing events.
2. Replace an OpenCode Go credential through supported APIs and document which actions require a runtime restart.
3. Run two isolated OpenCode Go profiles concurrently on one host using only supported state/config mechanisms.
4. Drive Codex App Server over stdio, including deltas, diffs, approvals, interruption, login, and rate-limit events.
5. Run two isolated Codex ChatGPT accounts concurrently without copying or modifying token storage.
6. Test whether a connector can attach to a Codex turn already active in the official desktop app.
7. Confirm provider terms and store-review implications for user-controlled account rotation.

## 13. Success criteria

- Pairing a new host takes less than two minutes.
- A live agent update normally appears on the phone within one second.
- Reconnect restores a correct session view without duplicated or missing final state.
- The user can identify the account used by any session in one glance.
- A bulk profile change previews every affected runtime and reports individual success or failure.
- No credential plaintext appears in relay storage, analytics, crash reports, or normal logs.

## 14. Discussion questions

1. Is OpenCode Go rotation primarily manual, quota-triggered failover, scheduled, or load balancing?
2. Should the same credential be provisioned to many hosts, or should a central host act as a gateway so the key exists in only one place?
3. Must Muxport adopt sessions started in the ordinary OpenCode/Codex UI, or is it acceptable for reliable sync to require starting them through Muxport?
4. Is the first release personal-only, or must it support teams and delegated access?
5. Is an optional hosted relay acceptable, or must all remote connectivity use Tailscale/self-hosting?
6. Should credential/profile switching be available from the phone only, or also from a desktop/web dashboard and CLI?

## 15. Research basis

- OpenCode Server: https://dev.opencode.ai/docs/server/
- OpenCode providers and Go authentication: https://opencode.ai/docs/providers
- Codex App Server: https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md
- OpenAI account switching: https://help.openai.com/en/articles/20001068-use-multiple-accounts-with-account-switching

The name Muxport is a working product name, not legal trademark clearance.
