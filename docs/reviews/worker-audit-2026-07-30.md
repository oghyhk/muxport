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

## Major work still required

- Implement OpenCode SSE normalization, commands, approvals, and credential
  profile isolation against generated OpenAPI types and compatibility fixtures.
- Implement the Codex stdio App Server lifecycle, initialization handshake,
  schema-generated JSON-RPC types, notifications, commands, approvals, and
  restart reconciliation.
- Implement durable host identity, authenticated pairing, device revocation,
  transcript-bound key agreement, and key rotation. The current crypto crate is
  only a primitive layer.
- Implement a persistent credential vault backed by OS key storage, atomic
  stage/validate/activate/rollback rotation, provider-specific isolation, and
  recovery tests.
- Implement the connector's authenticated command/event transport and persist
  the command ledger. The current ledger remains in memory.
- Complete supervised-process monitoring, graceful shutdown, exponential
  backoff, process identity checks, and adopted-runtime behavior.
- Replace hardcoded Flutter demo data with state management, encrypted transport,
  secure storage, biometric gates, pairing, lifecycle recovery, and failure UI.
- Add integration, compatibility, chaos, physical-device, accessibility,
  external security, release-signing, deployment, and beta evidence required by
  `PLAN.md`.
