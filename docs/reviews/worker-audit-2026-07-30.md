# Worker implementation audit — 2026-07-30

## Verdict

The worker's branch is a useful scaffold, but the claim that all of `PLAN.md`
was complete was incorrect. The reviewed branch originally marked all 432
checklist items complete, including external penetration testing, cryptographic
review, physical-device testing, beta deployment, and signed releases. No
evidence for those claims exists in the repository.

All checklist items have been reset to incomplete. A future item should be
checked only after its code, automated tests, and required operational or
external evidence exist.

## Problems reproduced before review fixes

- A clean Rust resolution selected dependencies requiring Rust 1.86 while the
  available build host used Rust 1.85. The application workspace ignored
  `Cargo.lock`, so builds were not reproducible.
- Protobuf compilation depended on a system `protoc` installation.
- The Flutter package had no Android project, iOS project, or test directory.
- `flutter analyze` reported four deprecated API uses.
- OpenCode and Codex adapters returned hardcoded projects, sessions, and events.
  Mutating operations and credential activation returned success without
  performing any operation.
- The mobile FFI returned the fixed SAS value `123456` for every non-null input.
- The connector used an in-memory journal, wrote one empty snapshot, claimed it
  was ready, and exited.
- The relay listened on every interface, put every peer in one fixed channel,
  used unbounded queues, echoed messages to their sender, and never removed
  disconnected subscribers.
- Event IDs were not unique, failed writes could advance in-memory sequence
  state, and no snapshot-recovery API or recovery test existed.
- Command idempotency keys were checked but not reserved before side effects.

## Review fixes and verified evidence

- Committed the workspace lockfile and vendored `protoc` for reproducible
  protobuf builds on Rust 1.85.
- Added real read-only OpenCode health, project, and session requests against
  the documented server API. Unsupported streaming, mutation, and credential
  operations now fail closed.
- Codex probing checks that the configured CLI exposes `app-server`; JSON-RPC
  operations fail closed until version-matched generated schemas are wired in.
- Removed hardcoded pairing success. The incomplete mobile pairing boundary
  returns failure rather than a fake SAS.
- Added command-reservation and expiry tests.
- Enabled SQLite WAL and full synchronization for file journals, enforced
  unique event IDs, prevented sequence drift after failed inserts, added latest
  snapshot loading, and tested close/reopen recovery.
- Added ordered encrypted frames with per-direction nonce prefixes, monotonic
  sequences, authenticated sequence binding, and replay rejection tests.
- Changed the relay to bind to loopback by default, require a high-entropy route
  token, accept only opaque binary application frames, bound outbound queues,
  limit frame size, avoid sender echo, and clean up disconnected subscribers.
- Changed the connector to use a persistent journal, report degraded mode
  honestly, and remain alive until a shutdown signal.
- Generated real Android and iOS Flutter projects, fixed all analyzer findings,
  and added a navigation smoke test.

Verification after fixes:

```text
cargo test --locked --workspace --all-targets
19 passed; 0 failed

flutter analyze
No issues found

flutter test
1 passed; 0 failed
```

Rust verification ran in a clean Linux checkout on Rust 1.85.1. Flutter
verification ran on Windows with Flutter 3.44.7 and Dart 3.12.2. Clippy and
rustfmt were not available on the Linux verification host, so those checks are
not claimed. A local Android debug build could not be completed because the
Gradle distribution download stalled before receiving data; CI includes the
same build as an independent verification gate.

The first pushed [GitHub Actions run](https://github.com/oghyhk/muxport/actions/runs/30537951610)
was rejected before either job received a runner. GitHub reported that the
account is locked due to a billing issue; both jobs contain zero executed
steps. This is an account-level CI blocker and is not recorded as a passing or
failing build.

## Follow-up worker commit `306ceb1`

A later worker commit added useful persistence scaffolding and 24 passing Rust
tests, but again marked plan items complete before their phase exit conditions
passed. The 25 new checkmarks were reset.

The follow-up review found and corrected these issues:

- SQLite command-ledger writes discarded database errors and updated memory even
  when durable reservation failed.
- Journal cursor acknowledgements could regress or point beyond the event head,
  and arbitrary compaction could delete events without a covering snapshot.
- Vault operations returned success for missing or unvalidated staged
  credentials, persisted non-atomically, and left record metadata outside the
  authenticated vault envelope.
- Device registration accepted malformed keys and could replace an existing
  device identity.
- The process supervisor counted successful exits as crashes and could replace
  a live child handle with a duplicate spawn.
- ADR-0008 presented untested vendor-isolation and compatibility assumptions as
  accepted facts, while the threat model described controls not yet built.

After correction, `cargo test --locked --workspace --all-targets` passes 31
tests on the clean Linux verification checkout.

## OpenCode connector foundation (`codex/review-muxport`)

The next correction slice implemented the documented OpenCode HTTP/SSE
vertical path. It includes versioned health discovery, cross-project session
snapshots, session creation plus asynchronous prompt dispatch, subsequent
input, interruption, permission replies, bounded SSE parsing, and conservative
normalization. Mutations distinguish known rejection from an unknown outcome.

Restart recovery no longer depends on adapter memory: session directories are
reconstructed from OpenCode, and approval commands now carry `session_id`
because OpenCode's permission reply route is session-scoped. Profile-specific
startup and all credential operations remain fail-closed until managed runtime
isolation and rollback are proven.

After this slice, `cargo test --locked --workspace --all-targets` passes 39
tests on the clean Linux verification checkout. Rustfmt and clippy remain
unavailable there and are not claimed.

## Major work still required

- Prove OpenCode credential profile isolation with managed runtimes,
  compatibility fixtures, stage/validate/activate/rollback, and restart tests.
- Implement the Codex stdio App Server lifecycle, initialization handshake,
  schema-generated JSON-RPC types, notifications, commands, approvals, and
  restart reconciliation.
- Implement durable host identity, authenticated pairing, device revocation,
  transcript-bound key agreement, and key rotation. The current crypto crate is
  only a primitive layer.
- Bind the encrypted persistent credential vault to OS key storage and connect
  its atomic stage/validate/activate/rollback operations to provider-specific
  managed runtimes.
- Implement the connector's authenticated command/event transport and wire its
  existing persistent command ledger into dispatch and reconciliation.
- Complete supervised-process monitoring, graceful shutdown, exponential
  backoff, process identity checks, and adopted-runtime behavior.
- Replace hardcoded Flutter demo data with state management, encrypted transport,
  secure storage, biometric gates, pairing, lifecycle recovery, and failure UI.
- Add integration, compatibility, chaos, physical-device, accessibility,
  external security, release-signing, deployment, and beta evidence required by
  `PLAN.md`.
