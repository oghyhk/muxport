# PLAN implementation evidence — 2026-07-31

This audit marks only checklist items whose complete wording is backed by the
current implementation and automated tests. A combined item remains open if
any clause is missing.

## Adapter contract and built-ins

- `crates/adapter-api/src/lib.rs` defines project discovery, normalized event
  subscription, session start/input/steer/interrupt, correlated approvals,
  graceful shutdown, and an explicit capability set.
- `crates/adapter-opencode` and `crates/adapter-codex` implement that trait and
  are linked directly by `crates/connector`.
- OpenCode uses loopback HTTP/SSE and runtime-discovered supported auth
  endpoints. Codex uses App Server JSON-RPC over owned stdio. Neither adapter
  edits vendor credential or session databases.
- Versioned fixtures live under `crates/adapter-opencode/fixtures/1.18.10` and
  `crates/adapter-codex/fixtures/0.146.0`; recorded and live-fixture tests guard
  supported shapes.

Items that remain open include a dedicated `read_session`, usage in the common
trait, credential preparation, broader generated-schema coverage, and
compatibility preservation for every unknown source field.

## Pairing, synchronization, and commands

- Signed one-use pairing, dual SAS confirmation, pinned host identity,
  encrypted authenticated sessions, and registry persistence are tested in
  `pairing_coordinator`, `pairing_store`, `secure_session`, and the Dart direct
  transport tests.
- `RuntimeMirror` plus `EventJournal` normalize, correlate, durably sequence,
  snapshot, replay, and reconcile source events.
- `CommandRouter` persists reservation and dispatch state before mutation,
  rejects expired commands and idempotency-key payload reuse, returns durable
  prior results, and exposes unknown outcomes as reconciliation-required.
- Flutter keeps approval cards unresolved until the connector command has
  returned from the source adapter and authoritative sync removes or resolves
  the source approval. Repeated taps are blocked.

The opaque relay, push wakeups, queued offline writes, and mobile pending-
operation status query are still open.

## Secret and diagnostic boundaries

- Provider secrets stay in the host vault or vendor-owned isolated state; the
  mobile protocol and UI currently provide no provider-secret provisioning
  path.
- `CredentialMaterial` cannot be serialized, redacts `Debug`, and zeroizes its
  secret. The mobile cache recursively rejects secret-shaped keys before disk
  writes.
- OpenCode managed stdout/stderr are discarded. Codex stderr is drained without
  logging because it may contain prompts, paths, or provider details. Connector
  logs include identifiers and redacted errors, not environment values,
  authorization headers, provider keys, or raw command/approval bodies.
- Credential projections contain label, provider, account fingerprint, status,
  and validation timestamp only. No API or screen returns vault plaintext after
  enrollment.

Encrypted backup/restore, diagnostic export, upstream revocation, phone-side
secret provisioning, and headless vault unlock remain open.

## Restart and mobile evidence

- The connector loads the versioned runtime manifest and durable command ledger
  on start, prunes removed runtimes, restores snapshot plus journal events, and
  independently resynchronizes every configured adapter.
- Managed OpenCode and Codex children implement same-profile restart,
  kill-on-drop ownership, five-failures-in-ten-minutes latching, and graceful
  `SIGINT`/`SIGTERM` shutdown.
- Flutter renders one unified timeline from OpenCode and Codex projections
  across independently pinned, cached, and synchronized hosts.
- Rust workspace tests, live OpenCode/Codex compatibility fixtures, Flutter
  analysis, Flutter tests, a release build, a graceful SIGTERM exercise, and a
  rootless Docker build passed during this audit.
