# Muxport — Engineering Implementation Plan

Status: working implementation plan  
Date: 2026-07-30  
Companion document: `SPEC.md`

## 1. Purpose of this document

`SPEC.md` defines what Muxport should do. This document defines how to build, validate, secure, ship, recover, and maintain it.

The plan is organized as an implementation checklist. A checkbox is complete only when its verification item and exit condition pass; writing code alone is not completion.

## 2. Fixed architectural decisions

- [ ] Use **Flutter** for the iOS and Android application.
- [ ] Use **Rust** for the host connector, relay, protocol core, credential vault, runtime supervision, and shared security-sensitive code.
- [ ] Keep OpenCode and Codex running on user-controlled hosts; the mobile app is a control and synchronization client.
- [ ] Integrate OpenCode through its supported HTTP/OpenAPI and SSE interfaces.
- [ ] Integrate Codex through a locally supervised `codex app-server` using its supported stdio JSON-RPC transport.
- [ ] Do not depend on Codex's experimental WebSocket listener for production.
- [ ] Treat OpenCode and Codex integrations as adapters behind one versioned internal interface.
- [ ] Make adapters built-in and signed for MVP; do not load arbitrary third-party code into the connector process.
- [ ] Keep the relay optional and unable to decrypt application payloads.
- [ ] Support a direct path over LAN or a user-managed private network such as Tailscale.
- [ ] Keep credentials on explicitly selected hosts; the phone stores provider secrets only transiently during optional provisioning.
- [ ] Treat runtime session state on the host as the source of truth.
- [ ] Treat mobile caches and the connector event journal as reconstructible state.
- [ ] Apply account changes to new sessions by default; never silently move an active turn to another identity.
- [ ] Model bulk switching as a tracked multi-host operation with per-host results, not as an impossible all-or-nothing distributed transaction.

## 3. System architecture

```text
┌──────────────────────────────── Flutter mobile app ────────────────────────────────┐
│ Host list │ Session UI │ Approvals │ Accounts │ Rotation │ Settings │ Local cache │
│           Native key store / biometrics / push token / encrypted transport         │
└───────────────────────────────┬─────────────────────────────────────────────────────┘
                                │
                 direct LAN/private network or E2EE relay
                                │
┌───────────────────────────────▼─────────────────────────────────────────────────────┐
│ Optional relay                                                                      │
│ Opaque routing │ connection presence │ encrypted frame forwarding │ generic pushes  │
│ No provider credentials, code, prompts, diffs, command text, or plaintext metadata │
└───────────────────────────────┬─────────────────────────────────────────────────────┘
                                │ outbound connection from host
┌───────────────────────────────▼─────────────────────────────────────────────────────┐
│ Muxport connector                                                                   │
│ Pairing │ E2EE │ command dedupe │ desired-state reconciler │ event journal │ audit │
│ Credential vault │ runtime supervisor │ capability/version negotiation             │
├───────────────────────────────┬─────────────────────────────────────────────────────┤
│ OpenCode adapter              │ Codex adapter                                       │
│ HTTP/OpenAPI snapshots        │ App Server JSON-RPC over stdio                     │
│ SSE event stream              │ thread/turn/item/account notifications              │
│ Supported auth endpoints      │ supported account login/logout endpoints            │
└───────────────────────────────┴─────────────────────────────────────────────────────┘
```

### Trust boundaries

1. The mobile OS protects the device identity key and biometric gate.
2. The encrypted session protects phone-to-connector traffic from the relay and network.
3. The connector protects provider credentials from the mobile UI, plugins, logs, and relay.
4. OpenCode and Codex receive only the credential selected for their isolated runtime profile.
5. A compromised development host can observe credentials while they are being used on that host; Muxport cannot eliminate this host-trust requirement.

## 4. Monorepo and codebase design

Create one repository with reproducible builds and independently releasable artifacts:

```text
muxport/
├─ apps/
│  ├─ mobile/                    # Flutter iOS/Android app
│  └─ relay-admin/               # Minimal operator UI, post-MVP
├─ crates/
│  ├─ connector/                 # Host daemon and CLI entrypoint
│  ├─ connector-core/            # Reconciler, operations, lifecycle state machines
│  ├─ adapter-api/               # Stable adapter traits and capability model
│  ├─ adapter-opencode/          # OpenCode HTTP/SSE implementation
│  ├─ adapter-codex/             # Codex App Server stdio JSON-RPC implementation
│  ├─ process-supervisor/        # Managed child lifecycle and crash-loop control
│  ├─ credential-vault/          # Secret envelope storage and OS integrations
│  ├─ event-journal/             # Replay log, cursor management, compaction
│  ├─ muxport-protocol/          # Generated types and protocol versioning
│  ├─ muxport-crypto/            # Audited E2EE and pairing primitives
│  ├─ relay/                     # Opaque WebSocket routing and push triggers
│  ├─ mobile-core/               # Optional Rust library exposed to Flutter over FFI
│  └─ test-harness/              # Fake adapters, failure injection, protocol fixtures
├─ packages/
│  ├─ flutter_protocol/          # Generated Dart protocol package
│  ├─ flutter_design_system/     # Shared widgets, colors, spacing, accessibility
│  └─ adapter_sdk/               # Future out-of-process plugin SDK and schemas
├─ protocol/
│  ├─ schema/                    # Protobuf or equivalent canonical schemas
│  ├─ fixtures/                  # Cross-language golden messages
│  └─ compatibility/             # Supported version and capability matrices
├─ deploy/
│  ├─ docker/                    # Relay and headless connector images
│  ├─ systemd/                   # Linux user/system services
│  ├─ launchd/                   # macOS LaunchAgent definitions
│  └─ windows/                   # Windows per-user startup/service packaging
├─ docs/
│  ├─ architecture/
│  ├─ threat-model/
│  ├─ runbooks/
│  ├─ protocol/
│  └─ release/
├─ scripts/                      # Reproducible build, signing, schema generation
├─ tests/
│  ├─ e2e/
│  ├─ recovery/
│  ├─ security/
│  └─ compatibility/
├─ SPEC.md
└─ PLAN.md
```

### Code ownership boundaries

- `apps/mobile` may display secret metadata but may not read host vault files.
- Adapters may request a short-lived credential handle but may not enumerate or export other credentials.
- Relay code must not depend on agent payload schemas beyond opaque envelope routing fields.
- Event-journal code must not import credential-vault internals.
- External plugins, when introduced, run out of process and receive explicit capabilities; they never link into the vault.

## 5. Runtime and plugin architecture

### Adapter contract

Define a versioned `AgentAdapter` interface with:

- [x] `probe()` — executable version, API version, capabilities, and health.
- [x] `discover_projects()` — repositories/workspaces visible to the runtime.
- [x] `list_sessions()` and `read_session()` — authoritative snapshot.
- [x] `subscribe()` — normalized live-event stream.
- [x] `start_session()`, `send_input()`, `steer()`, and `interrupt()`.
- [x] `respond_to_approval()` with source correlation identifiers.
- [x] `read_account_state()` and `read_usage()` when supported.
- [ ] `prepare_credential()`, `validate_credential()`, and `activate_credential()`.
- [x] `shutdown_gracefully()` for connector-managed runtime instances.
- [x] A declared capability set so unsupported controls are hidden rather than failing late.

### Built-in adapter strategy

- [x] Compile OpenCode and Codex adapters into the signed connector for MVP.
- [x] Make each adapter use only documented public interfaces.
- [ ] Generate OpenCode client types from the running server's OpenAPI document during compatibility testing, not dynamically in production.
- [ ] Generate Codex JSON-RPC types from the installed Codex App Server schema for compatibility tests.
- [ ] Preserve unknown fields and retain raw source event type/version for diagnostics.
- [x] Add per-version contract fixtures so vendor updates cannot silently change behavior.

### Future plugin host

- [ ] Define an out-of-process plugin protocol only after the built-in adapter API stabilizes.
- [ ] Require a signed manifest containing plugin ID, version, executable hash, requested capabilities, supported source versions, and publisher.
- [ ] Launch plugins in a sandbox with no vault-directory access.
- [ ] Pass one-use credential handles or scoped requests rather than plaintext vault contents.
- [ ] Allow plugins to emit normalized events and request narrowly scoped operations.
- [ ] Provide explicit install, update, disable, quarantine, and revoke workflows.

## 6. Connector design

### Connector responsibilities

- [x] Pair and authenticate mobile devices.
- [ ] Maintain direct and relay transports.
- [ ] Own desired configuration and observed runtime state.
- [x] Supervise managed OpenCode and Codex processes.
- [ ] Adopt compatible externally launched runtimes when a stable endpoint is supplied.
- [x] Normalize and journal source events.
- [x] Deduplicate and execute remote commands.
- [ ] Store credentials and assignments.
- [ ] Reconcile after every connector, runtime, host, or network restart.
- [ ] Produce local, redacted diagnostics and audit events.

### Host installation mode

- [ ] macOS: per-user `LaunchAgent`, with explicit opt-in to keep the host awake.
- [ ] Windows desktop: per-user background process installed through signed packaging; avoid a system service until user-vault behavior is proven.
- [ ] Linux desktop: `systemd --user`.
- [ ] Linux server: dedicated unprivileged service account with a systemd service and explicitly configured vault-unlock method.
- [x] Container: rootless image where possible, with project directories and encrypted vault storage mounted separately.
- [ ] Never run the connector or agent runtimes as administrator/root unless a documented deployment explicitly requires it.

### Desired-state reconciler

Persist desired state separately from observed state:

```text
desired:
  runtime should exist
  runtime profile and project
  assigned credential profile
  autostart policy
  restart policy

observed:
  process PID/start time
  source version and health
  active account fingerprint
  connected sessions and active turns
  last event cursor
  last successful snapshot
```

- [ ] Reconcile on connector start, runtime event, configuration change, and periodic health tick.
- [ ] Never infer success solely from a previously sent command.
- [ ] Read back account and runtime state after every credential activation.
- [ ] Surface desired/observed drift to the phone.
- [ ] Require user action before killing an unmanaged external process.

## 7. Local storage separation

Use separate stores so corruption or compromise of one subsystem does not expose or destroy the others:

```text
state/
├─ metadata.db             # hosts, runtimes, projects, assignments, operations
├─ events.db               # bounded reconstructible event journal
├─ audit.db                # append-only security and control history
├─ vault/
│  ├─ vault.sealed         # encrypted provider secrets
│  └─ key-metadata.json    # non-secret key IDs and algorithm versions
├─ profiles/
│  ├─ opencode/<profile>/  # isolated vendor-managed state
│  └─ codex/<profile>/     # isolated vendor-managed state
└─ diagnostics/            # redacted rotating logs
```

- [ ] Use SQLite transactions, foreign keys, integrity checks, and explicit schema migrations.
- [ ] Treat SQLite WAL/SHM files as live data, never disposable cache during recovery.
- [ ] Back up a consistent SQLite snapshot, not a copied main file without its WAL.
- [ ] Mark `events.db` as reconstructible and safe to rebuild only after preserving evidence.
- [ ] Keep `vault.sealed` independent of metadata and event-journal migrations.
- [x] Store no secret plaintext, auth headers, full environment dumps, or raw approval payloads in logs.
- [ ] Add a redacted diagnostic export that requires user confirmation and enumerates included files.

## 8. Credential security architecture

### Host vault

- [x] Generate a random data-encryption key for the vault.
- [ ] Wrap that key using an OS-protected key-encryption key:
  - [ ] macOS Keychain, hardware-backed when available.
  - [ ] Windows DPAPI/Credential Manager under the connector user.
  - [ ] Linux Secret Service when available.
  - [ ] Headless Linux fallback: Argon2id-derived wrapping key from an operator passphrase, TPM/systemd credential integration, or an explicitly configured external secret manager.
- [x] Use a reviewed authenticated-encryption construction and a versioned envelope format.
- [x] Bind ciphertext to profile ID, host ID, credential type, and schema version as authenticated data.
- [x] Zeroize plaintext buffers and prevent them from entering panic reports.
- [ ] Keep the vault locked after reboot until its configured unlock condition succeeds.
- [x] Never use a compiled-in master key or relay-held decryption key.

### Credential records

Store non-secret metadata separately:

```text
profile_id
display_name
provider
credential_type
account_fingerprint
created_at
last_validated_at
status                 # staged | active | cooling_down | invalid | revoked
allowed_host_ids
allowed_project_ids
rotation_pool_id?
secret_handle          # opaque reference into vault
```

- [ ] Display only labels, last validation time, account fingerprint, and status.
- [x] Never reveal a stored API key after enrollment.
- [x] Detect accidental duplicate enrollment using a non-reversible keyed fingerprint.
- [ ] Provide disable and revoke separately; local disable must not falsely claim the provider key was revoked upstream.

### Mobile secret handling

- [x] Store the mobile device identity in Keychain/Android Keystore.
- [x] Keep provider secrets off the phone by default.
- [ ] When a user enters a key on the phone, encrypt it directly to each selected host before leaving the credential screen.
- [ ] Keep plaintext only in a short-lived buffer; clear clipboard and field state after provisioning.
- [ ] Disable screenshots/app-switcher previews on credential-entry and recovery-key screens where the OS permits.
- [ ] Require biometric/device authentication before provisioning, bulk assignment, export, or rotation.
- [ ] Exclude provider credentials and E2EE private keys from ordinary cloud backup.

### Codex and OpenCode vendor state

- [x] Use supported login/auth APIs; never copy or edit vendor token databases.
- [x] Protect each managed account with a separate runtime-state directory once supported isolation is validated.
- [ ] Enforce owner-only filesystem permissions on profile directories.
- [ ] Record only an account fingerprint and plan label in Muxport metadata.
- [ ] Do not include vendor-managed auth state in support bundles.

## 9. Pairing and end-to-end encryption

- [ ] Perform a cryptographic design review before choosing final primitives.
- [ ] Use established libraries and protocols; do not create custom cryptography.
- [x] Give every phone and connector a long-term device identity key.
- [ ] Pair with a short-lived, one-use QR payload containing host identity, rendezvous data, expiry, and an authenticated key agreement challenge.
- [x] Display and verify a human-readable safety code on both devices for first pairing.
- [ ] Derive separate keys for commands, events, secret provisioning, and attachment transfer.
- [x] Rotate session keys and bind every frame to host ID, device ID, protocol version, sequence, and direction.
- [x] Reject replayed, expired, out-of-order-without-window, or wrong-host frames.
- [ ] Support device revocation and host key rotation without rotating provider credentials.
- [x] Require re-pairing after host identity loss; never silently trust a replacement key.
- [ ] Commission an external cryptographic review before public beta.

## 10. Protocol and synchronization

### Wire protocol

- [x] Define canonical schemas and generate Rust and Dart types.
- [ ] Include protocol version, capability version, request ID, idempotency key, host epoch, sequence, timestamp, and expiry where applicable.
- [x] Separate command, command-result, event, snapshot, acknowledgement, and error envelopes.
- [ ] Limit frame size and chunk large diffs/attachments with hashes.
- [ ] Use explicit redaction types so a secret cannot be serialized into an ordinary event by mistake.
- [x] Maintain golden cross-language fixtures in CI.

### Command semantics

- [x] Treat network delivery as at-least-once.
- [x] Persist a mutating command and its idempotency key before execution.
- [x] Return the previous result when a command is retried with the same key.
- [x] Give commands a deadline; reject stale approvals, rotations, and interrupts.
- [x] Never automatically replay a command whose outcome is unknown unless the source operation is independently idempotent.
- [ ] Make offline writes opt-in and limited to safe operations such as “start this new session when host reconnects”; never queue approvals or immediate credential rotation while offline.

### Event journal

- [x] Assign a connector-monotonic sequence to every normalized event.
- [x] Include connector boot epoch so sequence resets are unambiguous.
- [ ] Persist event and cursor before acknowledging it to the source adapter when possible.
- [x] Retain a bounded journal by age and size.
- [x] Compact only events older than every connected client's acknowledged cursor or an explicit retention threshold.
- [x] Store periodic authoritative snapshots.
- [x] Detect cursor gaps and force snapshot reconciliation.
- [ ] Coalesce high-frequency output deltas without changing final content.

### Mobile sync state

- [x] Flutter starts from its local cache and visibly marks it stale until host confirmation.
- [x] Request replay from the last acknowledged host epoch and sequence.
- [x] Replace cache with a snapshot after a gap, incompatible version, or journal reset.
- [x] Deduplicate by event ID and source object version.
- [x] Keep optimistic UI limited to commands with reversible presentation; show “pending” until connector acknowledgement.
- [x] Never show an approval as accepted until the source runtime confirms the response.

## 11. Lifecycle and restart recovery

### State machines

Connector:

```text
starting
  ├─> vault_locked
  ├─> recovering
  │     ├─> ready
  │     └─> degraded
  └─> fatal_configuration_error
```

Runtime:

```text
unknown -> discovering -> starting -> synchronizing -> online
                                      │                 ├─ idle
                                      │                 ├─ running
                                      │                 └─ waiting_for_user
                                      ├─ degraded
                                      ├─ stopped
                                      ├─ crashed
                                      ├─ crash_loop
                                      └─ credential_locked
```

Remote operation:

```text
created -> persisted -> dispatched -> source_acknowledged -> reconciled -> succeeded
                         │                    │
                         ├─ expired           ├─ failed
                         ├─ cancelled         └─ outcome_unknown -> reconciliation_required
                         └─ rejected_offline
```

- [ ] Implement these as explicit enums with allowed transitions.
- [ ] Persist every mutating transition transactionally.
- [ ] Reject impossible transitions and emit a redacted diagnostic event.
- [ ] Give the Flutter UI a consistent status and explanation for every state.

### Recovery invariant

After any restart, Muxport must reconstruct truth from the source runtime before accepting a new mutation. It must not infer that an action succeeded because it was sent before the failure.

### Failure and recovery matrix

| Failure | Expected behavior | Recovery action | Forbidden behavior |
|---|---|---|---|
| Mobile app backgrounded/killed | Host work continues; connector journals events | Reconnect, authenticate, replay cursor or request snapshot | Restarting or cancelling host work |
| Phone offline | Host continues; generic pushes may wait | Resume from last acknowledged sequence | Queueing stale approvals |
| Phone lost/stolen | Existing sessions continue; device access can be revoked | Revoke device certificate from another paired client or host CLI | Rotating every provider key automatically |
| Relay unavailable | Connector buffers bounded events; direct path may still work | Reconnect with backoff and replay | Falling back to plaintext/public agent ports |
| Connector killed | Managed agent process policy determines whether child survives; mobile marks host unavailable | OS supervisor restarts connector, then full reconcile | Re-sending unknown mutating commands |
| Host sleeps/reboots | Presence expires; no remote mutations accepted | Connector autostarts, unlocks vault, discovers runtimes, snapshots | Claiming sessions are still running based on cache |
| OpenCode server exits | Runtime becomes crashed/degraded | Restart only if connector-managed and policy permits; resnapshot sessions | Applying a new key because a crash was misclassified as quota failure |
| Codex App Server exits | In-flight result becomes unknown; persisted threads remain discoverable | Restart same isolated profile, list/read threads, mark unfinished turn interrupted/unknown | Re-submitting the last prompt automatically |
| Agent runtime crash loop | Stop automatic retries | Surface logs and require user action after threshold | Infinite restart loop |
| Credential vault locked | Session viewing from reconstructed non-secret state may be limited; new authenticated work blocked | Unlock through configured OS/passphrase method | Writing secrets to a temporary plaintext file |
| Credential activation fails | Old assignment remains active | Roll back staged assignment and report exact host failure | Deleting or overwriting the previous working credential |
| Event DB corrupt | Preserve files; connector enters degraded recovery | Snapshot/copy evidence, integrity check, rebuild reconstructible journal from sources | Deleting the only DB/WAL copy |
| Metadata DB corrupt | Stop mutations | Preserve evidence, restore verified snapshot, reconcile sources | Guessing assignments from partial rows |
| Vault corrupt | Stop credential operations | Preserve sealed vault and restore verified encrypted backup | Resetting vault and losing the only credential copy |
| Protocol version mismatch | Read-only or unsupported state | Capability downgrade or require upgrade | Executing a mutation with unknown semantics |
| App update interrupted | Previous signed version remains bootable | Atomic installer rollback | Partial in-place binary replacement |

### Mobile restart checklist

- [x] Persist host list, non-secret labels, last snapshot, cursors, pending local operation IDs, and device identity through atomic storage.
- [x] On launch, render cached state with a clear “reconnecting” marker.
- [x] Re-establish transport and verify host identity before accepting events.
- [x] Query status of every locally pending operation by idempotency key.
- [x] Resolve each operation to succeeded, failed, expired, or reconciliation required.
- [ ] Clear local plaintext inputs and temporary attachment files left by an OS kill.
- [ ] Re-register push token only after E2EE identity is restored.

### Connector restart checklist

- [x] Acquire a single-instance lock without deleting another process's lock blindly.
- [ ] Open stores read-only first, verify schema and integrity, then migrate one subsystem at a time.
- [x] Load desired runtime configuration and operation ledger.
- [ ] Unlock vault or enter `vault_locked`.
- [x] Generate a new connector boot epoch.
- [ ] Discover existing managed child processes using verifiable PID, executable path, start time, and profile markers; do not trust PID alone.
- [ ] Probe every source API and obtain authoritative snapshots.
- [x] Reconcile pending operations without automatically replaying unknown mutations.
- [ ] Start event subscriptions only after snapshot baseline identifiers are recorded.
- [ ] Reconnect relay/direct clients and publish a new snapshot boundary.

### Runtime restart policy

- [ ] Distinguish connector-managed, user-managed, and externally adopted runtimes.
- [ ] Automatically restart only connector-managed runtimes with restart policy enabled.
- [x] Use exponential backoff with jitter and a configurable cap.
- [x] Enter `crash_loop` after five failures within ten minutes by default.
- [ ] Preserve exit code, signal, stderr tail, source version, and profile ID in redacted diagnostics.
- [x] Never include environment values or tokens in captured crash output.
- [ ] Require a fresh health check, account readback, session snapshot, and event subscription before declaring recovery complete.

### Host reboot and vault unlock

- [ ] Desktop default: unlock through the signed-in user's OS credential store.
- [ ] Headless default: require an operator-selected TPM/systemd credential, external secret manager, or boot-time passphrase workflow.
- [ ] If unlock is unavailable, bring connector networking up in a limited `vault_locked` state so the phone can explain the problem.
- [ ] Do not allow remote entry of the host-vault master passphrase unless a separately reviewed recovery protocol is implemented.
- [ ] Permit per-runtime autostart only after its credential profile is available.

## 12. Safe credential switching and rotation

### Single-runtime switch transaction

- [x] Create an operation with a unique idempotency key.
- [ ] Check actor authorization and require biometric step-up on mobile.
- [x] Confirm target profile is compatible with runtime/provider.
- [x] Resolve the secret handle locally on the target host.
- [x] Stage the credential without changing the active assignment.
- [ ] Validate through a non-destructive supported provider operation.
- [ ] Inspect active turns.
- [ ] If work is active, default to “apply to new sessions”; require explicit drain/restart confirmation for immediate mode.
- [x] Activate the credential through the supported adapter method.
- [x] Read back account fingerprint/provider state.
- [x] Commit the assignment only after readback matches.
- [x] On failure, retain or restore the previous assignment.
- [ ] Record a redacted audit event and per-host result.

### Bulk switch

- [ ] Build and display an impact plan before dispatch.
- [ ] List compatible, incompatible, offline, locked, busy, and unmanaged runtimes separately.
- [ ] Let the user exclude individual targets.
- [ ] Dispatch one independently idempotent child operation per host/runtime.
- [ ] Show partial progress and partial failure honestly.
- [ ] Do not claim global success until every selected target has reconciled.
- [ ] Offer retry for failed targets with the original operation group ID.
- [ ] Never attempt distributed rollback of already successful hosts unless the user explicitly chooses “revert successful targets.”

### Rotation pools

- [ ] Define ordered pools of compatible credential profiles.
- [ ] Store pool policy, eligibility, cooldown, host restrictions, and last-selection cursor as non-secret metadata.
- [ ] Support manual, scheduled, round-robin, and confirmed-failure failover modes.
- [ ] Classify provider errors into authentication, permission, rate limit, quota, network, runtime crash, and unknown.
- [ ] Trigger automatic failover only from adapter-tested signals, never from matching arbitrary error text alone.
- [ ] Do not rotate on network failure, connector restart, malformed response, or agent crash.
- [ ] Apply cooldown and maximum-switch-per-hour limits to prevent rotation storms.
- [ ] Notify the user and record the reason for every automatic switch.
- [ ] Feature-gate quota-triggered rotation until OpenCode Go behavior and applicable terms have been validated.

### Credential replacement

- [x] Add new secret as `staged`.
- [ ] Validate it without overwriting the current secret.
- [x] Atomically point the profile to the new secret version.
- [x] Retain the old encrypted version for a short rollback window unless the user requests immediate removal.
- [ ] Confirm dependent runtimes can authenticate.
- [ ] Mark old version superseded and securely remove it after the rollback window.
- [ ] Clearly distinguish local deletion from upstream provider revocation.

## 13. OpenCode adapter implementation

- [x] Probe `/global/health` and record server version.
- [ ] Fetch or test against `/doc` OpenAPI during development/compatibility CI.
- [ ] Use `/project`, `/session`, `/session/status`, session messages, todos, diffs, and permission endpoints for snapshots and controls.
- [ ] Subscribe to `/event` or `/global/event` SSE and record the initial `server.connected` boundary.
- [ ] Normalize session, message, part, permission, todo, diff, provider, and error events.
- [ ] Re-fetch affected source objects after ambiguous or incomplete SSE events.
- [x] Use async prompt endpoints when appropriate and correlate returned/source IDs.
- [x] Abort through the supported session abort endpoint.
- [x] Use provider/auth endpoints and runtime-discovered auth schemas for credential validation and activation.
- [x] Never assume a hard-coded OpenCode Go provider ID without verifying it against the installed version.
- [x] Support managed fixed host/port launch so the connector can reliably reconnect.
- [x] Protect the local OpenCode server with connector-only binding/authentication; do not expose port 4096 publicly.
- [ ] Test SSE reconnect, duplicate events, missing events, reordered mobile delivery, and source restart.

### OpenCode restart recovery

- [ ] On health loss, mark subscription stale and stop accepting mutations.
- [ ] If connector-managed, request graceful shutdown only when necessary; otherwise preserve the process.
- [x] Restart with the same isolated profile and project configuration.
- [ ] Wait for health and provider/account validation.
- [ ] List all sessions and compare source IDs, message counts, statuses, and active permissions to the last snapshot.
- [ ] Emit synthetic reconciliation events for changes that occurred during downtime.
- [ ] Expire approvals that no longer exist at the source.
- [x] Never recreate or manually modify OpenCode's internal session database.

## 14. Codex adapter implementation

- [x] Launch `codex app-server` over stdio under a dedicated managed profile.
- [x] Perform initialization and capability negotiation.
- [x] Generate/test JSON-RPC schemas for the installed Codex version.
- [ ] Implement thread list/read/start/resume/fork where supported.
- [x] Implement turn start, steer, and interrupt.
- [ ] Stream thread, turn, item, message delta, command, file-change, diff, usage, warning, and completion notifications.
- [x] Implement server-initiated command/edit approval requests and correlated responses.
- [x] Implement account read, login start/cancel, logout, update notifications, and rate-limit reads.
- [x] Preserve unknown notifications for compatibility diagnostics without displaying untrusted raw payloads as privileged UI.
- [x] Keep stderr tracing separate from JSON-RPC stdout.
- [ ] Apply bounded queues and backpressure; coalesce UI deltas before mobile transmission.
- [x] Do not expose App Server directly to the network.

### Codex restart recovery

- [ ] Detect App Server exit and capture only redacted crash metadata.
- [ ] Mark any in-flight turn `outcome_unknown` until source reconciliation.
- [x] Restart the same isolated profile when policy allows.
- [x] Reinitialize and list/read persisted threads.
- [ ] Compare known turn and item IDs with authoritative thread state.
- [ ] Mark an unfinished turn interrupted/failed only when source state establishes that result; otherwise display “connection lost—verify before retry.”
- [x] Never automatically resend the last turn.
- [ ] Re-read account identity and rate limits before accepting new work.
- [ ] Run a dedicated spike to determine whether a new App Server can attach to a turn owned by the official Codex desktop app.

## 15. Flutter application implementation

### Project foundation

- [x] Create Flutter app with iOS and Android targets.
- [ ] Establish feature-first modules: onboarding, hosts, sessions, approvals, accounts, operations, settings, and diagnostics.
- [ ] Select one predictable state-management approach and enforce it throughout the app.
- [ ] Use declarative routing with deep links for push notifications.
- [x] Generate Dart protocol types from the canonical schema.
- [x] Use an encrypted or non-secret-only local database; keep device keys in native secure storage.
- [ ] Add platform channels/FFI only for security, background, and transport features that cannot be safely implemented in portable Dart.

### Screens

- [ ] Onboarding and security explanation.
- [ ] QR host pairing and safety-code verification.
- [x] Host fleet overview.
- [ ] Host/runtime/project detail.
- [x] Unified OpenCode/Codex session timeline.
- [x] Approval inbox.
- [x] Credential profiles and validation status.
- [ ] Assignment matrix by host/runtime/project.
- [ ] Rotation-pool editor.
- [ ] Bulk-switch impact preview and progress.
- [x] Recovery/degraded-state explanations.
- [ ] Device/host revocation and security log.
- [ ] Redacted diagnostics export.

### Mobile lifecycle

- [ ] Handle foreground, background, suspension, process death, network change, and low-memory events.
- [x] Persist sync cursor before background suspension.
- [ ] Use push as a wake-up hint, never as authoritative state.
- [ ] Fetch the current approval from the connector before rendering an action button from a notification.
- [x] Prevent double approval from repeated taps or duplicate pushes.
- [ ] Test Android process death and iOS background eviction, not only hot reload/restart.
- [x] Blur sensitive content in app switcher previews.
- [ ] Provide an optional app lock with biometric/PIN fallback.

### Accessibility and phone ergonomics

- [ ] Meet platform text scaling, screen-reader, contrast, and touch-target requirements.
- [ ] Render diffs and command approvals as structured mobile cards rather than terminal dumps.
- [ ] Make destructive and credential-changing actions visually distinct.
- [ ] Support external keyboard input without making it required.
- [ ] Preserve position in long streaming timelines without forcing auto-scroll.

## 16. Relay and notification service

### Relay

- [ ] Route opaque encrypted frames by unguessable connection identifiers.
- [ ] Store no decrypted application data.
- [ ] Keep presence metadata minimal and short-lived.
- [ ] Rate-limit pairing, connection, and frame forwarding.
- [ ] Enforce frame-size, connection-count, and idle-time limits.
- [ ] Support regional/self-hosted deployment from the same code.
- [ ] Make relay failure non-destructive; connectors retain bounded journals.
- [ ] Publish a clear metadata and retention document.

### Push notifications

- [ ] Store APNs/FCM tokens separately from application identities where practical.
- [ ] Send generic notifications such as “A host needs your attention.”
- [ ] Do not place prompts, code, command text, diffs, account names, or provider keys in push payloads.
- [ ] On tap, open the app, establish E2EE, and fetch current state.
- [ ] Collapse obsolete notifications after approval/session completion.
- [ ] Allow per-host and per-event notification settings.

## 17. Backup, restore, and disaster recovery

### What to back up

- [ ] Versioned connector configuration with secret references only.
- [ ] Consistent metadata and audit SQLite snapshots.
- [ ] Encrypted vault backup only when the user explicitly enables secret recovery.
- [ ] Device and host registry with revocation state.
- [ ] No raw OpenCode/Codex live SQLite/WAL/SHM files in Muxport backups.
- [ ] Use vendor-supported export mechanisms separately for agent-session portability when available.

### Backup format

- [ ] Authenticated, encrypted, versioned archive.
- [ ] Manifest containing schema versions, hashes, timestamp, host ID, file sizes, record counts, and encryption parameters.
- [ ] Recovery key/passphrase generated outside relay control.
- [ ] Secrets excluded by default; UI must explain the consequence.
- [ ] Never overwrite older generations automatically.

### Restore procedure

- [ ] Restore into a temporary isolated location.
- [ ] Verify archive authentication, hashes, SQLite integrity, schema versions, age, and expected record counts.
- [ ] Start connector in isolated recovery mode with no agent mutations.
- [ ] Reconcile restored assignments against installed runtimes and currently available credentials.
- [ ] Promote only after health checks pass.
- [ ] Preserve the original damaged state and restored test copy.
- [ ] Document hourly/daily/monthly retention for self-hosted operators.

### Vault loss behavior

- [ ] Muxport remains able to show non-secret metadata and explain which profiles are unavailable.
- [ ] Provider credentials are not recoverable without an enabled encrypted vault backup or upstream reauthentication.
- [ ] Re-enrollment creates a new secret version without modifying existing agent session data.
- [ ] Device re-pairing does not imply provider-key recovery.

## 18. Testing strategy

### Unit tests

- [ ] State-machine transition tables.
- [ ] Assignment precedence.
- [ ] Rotation selection, cooldown, and storm prevention.
- [x] Envelope encryption/decryption and replay rejection.
- [x] Command idempotency and deadline handling.
- [x] Event ordering, compaction, cursor gaps, and snapshot replacement.
- [x] Secret redaction and serialization denial.
- [x] Error classification with unknown-safe default.

### Contract tests

- [ ] Supported OpenCode versions against recorded and live API fixtures.
- [ ] Supported Codex versions against generated schema fixtures.
- [x] Golden Rust/Dart protocol messages.
- [ ] Capability downgrade and unsupported-version behavior.
- [x] Approval request/response correlation.

### Recovery and chaos tests

- [ ] Kill Flutter during streaming, approval, key entry, and bulk switching.
- [ ] Kill connector before dispatch, after dispatch, before source acknowledgement, and before result persistence.
- [ ] Kill OpenCode/Codex before and after a tool call.
- [ ] Reboot host with vault locked and unlocked.
- [ ] Drop relay connectivity, reorder frames, duplicate frames, and delay acknowledgements.
- [ ] Corrupt copies of metadata, event, audit, and vault files independently.
- [ ] Exhaust disk space during event append and credential staging.
- [ ] Simulate clock skew and expired operations.
- [ ] Upgrade/downgrade connector and mobile versions across the support window.
- [ ] Verify no test ever repairs the only copy of a corrupted database.

### Security tests

- [ ] Threat-model review for stolen phone, malicious relay, compromised host, malicious plugin, replay, MITM, supply chain, logs, backups, and clipboard leakage.
- [ ] Static analysis and dependency auditing for Rust, Dart, iOS, and Android.
- [ ] Secret scanning in repository and release artifacts.
- [ ] Fuzz encrypted frame parsing, JSON-RPC parsing, SSE parsing, schema decoding, and plugin manifests.
- [ ] Attempt privilege escalation from adapter/plugin process to vault.
- [ ] Verify relay database and logs contain no decryptable content.
- [ ] External penetration test and cryptographic review before public beta.

### End-to-end acceptance tests

- [ ] Pair two phones with three hosts.
- [ ] Run OpenCode and Codex concurrently on each supported desktop/server OS.
- [ ] Start sessions, approve actions, rotate an OpenCode Go key, and perform a partial bulk switch.
- [ ] Kill and restart each layer independently while a turn is running.
- [ ] Recover with no duplicated prompt, no false approval, no lost credential assignment, and an honest unknown state where exact outcome cannot be proven.
- [ ] Revoke one phone without affecting the other phone or provider credentials.

## 19. CI/CD and release engineering

- [ ] Pin toolchains and generate lockfiles.
- [ ] Reproduce protocol generation in CI and fail on uncommitted schema output.
- [ ] Build/test Rust on Windows, macOS, and Linux.
- [ ] Build/test Flutter on iOS and Android with physical-device smoke testing before release.
- [ ] Sign connector binaries, installers, mobile applications, and update manifests.
- [ ] Generate SBOMs and provenance attestations.
- [ ] Publish checksums and verify them in updater code.
- [ ] Use staged release channels: development, internal, alpha, beta, stable.
- [ ] Implement atomic connector updates with previous-version rollback.
- [ ] Block automatic migrations until a verified backup/snapshot exists.
- [ ] Maintain a compatibility matrix for mobile, connector, OpenCode, Codex, and protocol versions.
- [ ] Keep at least one independently usable management/recovery path when updating the connector or relay.

## 20. Delivery phases

### Phase 0 — Decisions, threat model, and feasibility spikes

- [ ] Record architecture decisions for Flutter, Rust, protocol, crypto, vault, relay, and plugin isolation.
- [ ] Complete the seven validation spikes in `SPEC.md`.
- [ ] Add restart/attach experiments for OpenCode TUI and official Codex desktop sessions.
- [ ] Confirm OpenCode Go error/rate-limit signals and credential activation behavior.
- [ ] Review relevant provider terms for multi-account and rotation behavior.
- [ ] Produce wireframes for host fleet, session, account matrix, and bulk-switch preview.

Exit: no unresolved blocker to safe managed-session sync, isolated profiles, or key switching.

### Phase 1 — Monorepo, protocol, and fake system

- [ ] Scaffold repository and CI.
- [x] Define canonical schemas and generated Rust/Dart packages.
- [ ] Implement fake connector, fake OpenCode/Codex adapters, and deterministic event playback.
- [ ] Build Flutter navigation and screens against fake data.
- [x] Implement operation ledger, event journal, and state-machine tests.

Exit: mobile app can drive a simulated multi-host system through disconnect/reconnect tests.

### Phase 2 — Local connector and OpenCode vertical slice

- [ ] Implement host installation, pairing, direct connection, vault prototype, and OpenCode adapter.
- [ ] Stream one real OpenCode session to Flutter.
- [ ] Send prompts, answer approvals, interrupt, and reconcile after OpenCode/connector restart.
- [ ] Add one OpenCode Go profile and safely switch new sessions.

Exit: one phone controls one OpenCode host with verified restart recovery and no plaintext secret leakage.

### Phase 3 — Codex vertical slice

- [ ] Implement supervised App Server stdio client.
- [ ] Stream thread/turn/item events and diffs.
- [ ] Handle approvals, interruption, account login, and rate limits.
- [ ] Validate isolated Codex profiles.
- [ ] Reconcile after App Server and connector restart.

Exit: one app operates OpenCode and Codex through one normalized UI without losing source-specific safety semantics.

### Phase 4 — Fleet, bulk switching, and rotation

- [x] Pair multiple hosts.
- [ ] Implement assignment precedence and operation impact planning.
- [ ] Implement bulk switching with partial results.
- [ ] Implement rotation pools, cooldown, round robin, schedule, and guarded failover.
- [ ] Add audit log and device/host revocation.

Exit: multi-host credential operations are idempotent, auditable, recoverable, and tested under partial failure.

### Phase 5 — Relay, push, and mobile lifecycle hardening

- [ ] Deploy opaque relay.
- [ ] Add APNs/FCM generic pushes.
- [ ] Test phone suspension/process death and hostile network conditions.
- [ ] Add event replay, snapshot fallback, and relay outage handling at scale.

Exit: remote operation works across networks without exposing agent ports or plaintext to relay infrastructure.

### Phase 6 — Security and recovery hardening

- [ ] Complete platform key-store implementations.
- [ ] Complete encrypted backup/restore and disaster-recovery runbooks.
- [ ] Run fuzzing, dependency audits, secret scans, penetration test, and crypto review.
- [ ] Fix all critical/high findings and document accepted lower risks.
- [ ] Validate signed atomic updates and rollback.

Exit: security review approves public beta and restore drills succeed from verified backups.

### Phase 7 — Public beta and stable release

- [ ] Publish compatibility matrix and known limitations.
- [ ] Add opt-in redacted crash reporting.
- [ ] Run closed beta across Windows, macOS, Linux, iOS, and Android.
- [ ] Measure reconnect correctness, approval latency, crash loops, and rotation failures.
- [ ] Freeze v1 protocol and migration policy.
- [ ] Publish self-hosting, privacy, security, and recovery documentation.

Exit: release criteria in Section 22 pass for two consecutive candidate builds.

## 21. Operational runbooks

- [ ] Lost phone/device revocation.
- [ ] Host replacement and re-pairing.
- [ ] Vault unlock failure.
- [ ] Credential suspected compromised.
- [ ] Failed credential rotation and rollback.
- [ ] Connector crash loop.
- [ ] OpenCode/Codex incompatibility after vendor update.
- [ ] Metadata/event/audit database corruption.
- [ ] Vault restore from encrypted backup.
- [ ] Relay outage and regional failover.
- [ ] Bad signed connector release and rollback.
- [ ] Security incident evidence preservation.

Every incident runbook must preserve facts, hypotheses, unknowns, logs, file hashes, versions, and original state before repair.

## 22. Release gates and definition of done

### Correctness

- [ ] No mutating command is duplicated across retry/reconnect tests.
- [ ] Unknown outcomes are displayed as unknown, never guessed successful.
- [ ] Snapshot reconciliation repairs all tested event gaps.
- [ ] Active sessions never change credential identity silently.

### Restart resilience

- [ ] Phone, connector, relay, OpenCode, Codex, and host can each restart independently.
- [ ] Connector recovers desired/observed state and pending-operation status.
- [x] Runtime crash loops stop automatically.
- [ ] Interrupted updates roll back to a signed working version.

### Credential safety

- [ ] Provider secrets are absent from relay storage, mobile cache, logs, analytics, crash reports, push payloads, and repository history.
- [ ] Bulk switch requires step-up authentication and impact confirmation.
- [ ] New credential activation validates before replacing the old assignment.
- [ ] Vault backup and restore pass an isolated recovery drill.

### Compatibility

- [ ] Each supported OpenCode/Codex version passes contract and recovery tests.
- [ ] Unsupported versions fail closed for mutations while retaining useful diagnostics.
- [ ] Protocol downgrade behavior is documented and tested.

### User experience

- [ ] Pairing a host takes under two minutes in usability testing.
- [ ] Normal live updates reach the phone within one second at the target percentile.
- [ ] Credential identity is visible on every runtime and session.
- [ ] Partial bulk-operation failure is understandable and recoverable.
- [ ] Accessibility audit passes on supported iOS and Android versions.

## 23. Decisions still required

These choices should be resolved during Phase 0 and recorded as architecture decision records:

1. Whether provider secrets can be provisioned from mobile in v1 or must first be enrolled locally on a host.
2. Whether an encrypted vault can be replicated to several hosts or each host must be provisioned separately.
3. Final E2EE/pairing protocol and hardware-key integration.
4. Canonical wire encoding and schema tooling.
5. Flutter state-management, local-database, and Rust-FFI boundaries.
6. Exact supported isolation mechanism for multiple OpenCode and Codex identities.
7. Whether quota-triggered OpenCode Go rotation is reliable and permissible enough for v1.
8. Whether externally started live sessions can be adopted safely or require Muxport-managed launch.
9. Relay hosting model, retention limits, and self-hosted parity.
10. Minimum supported OS and OpenCode/Codex versions.

## 24. Immediate next actions

- [ ] Review this plan together and resolve the product decisions in Section 23.
- [ ] Create architecture decision records for accepted choices.
- [ ] Prototype OpenCode SSE recovery and Codex App Server recovery before building UI polish.
- [ ] Prototype isolated credential profiles and restart them repeatedly before implementing automatic rotation.
- [ ] Build the Flutter app against deterministic fake events while host spikes are underway.
- [ ] Do not begin public relay development until E2EE framing and pairing have passed design review.
