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

## Connector mirror and restart recovery

The daemon now restores its projection from the latest snapshot plus newer
journal events, verifies the journal before use, reconciles authoritative
OpenCode project/session/status state, journals live SSE before applying it,
persists state snapshots, and reconnects with bounded exponential backoff.
Taking a second snapshot after opening SSE closes the list-before-subscribe race
for the state currently represented by host snapshots. Periodic source
reconciliation repairs incomplete session/status events without replaying
mutations.

Protocol events now retain session project paths and resolved approvals retain
their session IDs. The protocol build explicitly tracks the external `.proto`
file so schema edits cannot silently reuse stale generated Rust code.

After this slice, `cargo test --locked --workspace --all-targets` passes 45
tests on the clean Linux verification checkout. The daemon still declares
itself degraded because authenticated mobile transport is not connected.

A two-run daemon smoke test also passed with OpenCode intentionally offline:
the first process journaled degraded state and handled `SIGINT`, while the
second process reopened the same database at sequence 1, passed integrity
checking, restored its snapshot, and shut down cleanly. The first smoke attempt
exposed and led to correction of a boot-epoch overflow at the SQLite signed
integer boundary.

## Codex App Server adapter foundation

The Codex scaffold was replaced with a supervised stdio App Server client
checked against schemas generated by the installed `codex-cli
0.146.0-alpha.3.1`. It performs the required initialize handshake, bounds JSONL
frames, correlates concurrent requests, fails pending operations on transport
loss, drains stderr without logging it, and shuts down or kills its owned child
within a bounded interval.

The adapter now implements paginated thread listing, project derivation, thread
and turn creation, resume plus subsequent input, steering, interruption,
notification/delta normalization, and command/file approval round trips.
Mutating transport failures remain unknown until reconciliation, and callbacks
from a replaced App Server process cannot be approved accidentally.

An additional read-only smoke exchange with the installed Codex executable
completed `initialize` and `thread/list`. After this slice,
`cargo test --locked --workspace --all-targets` passes 51 tests on the clean
Linux verification checkout. Rustfmt and clippy remain unavailable there and
are not claimed.

## Durable multi-runtime daemon integration

The single-source OpenCode loop was replaced by independent OpenCode and Codex
monitors feeding one bounded update queue and one journal owner. Each runtime
now performs the snapshot-subscribe-snapshot sequence, periodic authoritative
reconciliation, degraded-state journaling, and capped reconnect backoff without
blocking or replacing the other runtime's projection. Shutdown releases blocked
producers, closes the owned Codex child, leaves external OpenCode ownership
untouched, and persists a final combined snapshot.

Codex active/completed/error statuses and approval resolution now project into
the runtime state machine, and tests verify that reconciling one runtime cannot
remove another runtime's sessions. After this slice,
`cargo test --locked --workspace --all-targets` passes 55 tests on the clean
Linux verification checkout.

A two-run daemon smoke test with both sources intentionally unavailable handled
`SIGINT` cleanly on both runs and restored the combined persisted projection on
the second run. A separate unavailable-source smoke confirmed that both runtime
monitors independently emitted degraded updates before the clean shutdown.

## Durable command dispatch boundary

The connector now initializes a separate SQLite/WAL command ledger and an
adapter-aware command router. Reservations bind each idempotency key to a
SHA-256 fingerprint of the protobuf command before side effects, persist a
dispatched marker, wait for an in-process duplicate, and persist the complete
terminal result. Reusing a key for different bytes is rejected.

Completed duplicates survive restart and are not redispatched. An interrupted
or crash-recovered nonterminal record becomes `ReconciliationRequired` instead
of being replayed. Expired commands never reach adapters, while retries of an
already completed command still return its stored result after the original
deadline. Session mutations now carry an explicit runtime ID so routing never
guesses across OpenCode and Codex.

After this slice, `cargo test --locked --workspace --all-targets` passes 65
tests on the clean Linux verification checkout.

## Authenticated session and envelope foundation

The cryptographic layer now signs fresh X25519 handshake claims with pinned
Ed25519 device and host identities, binds HKDF and the SAS to the complete
signed transcript, rejects non-contributory keys, and derives separate
directional AEAD keys and nonce prefixes. Ordered decryption now requires the
exact next sequence rather than accepting gaps.

Paired-device registries can be serialized with a host-identity signature and
reject tampering, wrong-host loading, malformed keys, duplicates, revoked
re-registration, and identity replacement. A secure connector envelope codec
binds frames to the authenticated route/transcript and validates protocol,
sender, recipient, sequence, boot epoch, size, payload type, and command
idempotency before releasing a command to dispatch.

After this slice, `cargo test --locked --workspace --all-targets` passes 71
tests on the clean Linux verification checkout. No listener is enabled yet:
the physical SAS confirmation interface and OS protection for the host identity
key remain required.

## Restart-safe pairing transaction

Pairing offers now use random 256-bit rendezvous tokens stored only as
domain-separated hashes in a full-sync SQLite/WAL store. A verified signed
initiator can claim a token once, SAS comparison is constant-time, and separate
phone and host confirmations are both mandatory.

Unclaimed or one-sided pairings are cancelled on connector restart because
their ephemeral X25519 secrets no longer exist. Fully confirmed pairings can
finalize after restart. Finalization updates the host-signed device registry and
the pairing record in one database transaction, eliminating the registry/state
crash window.

After this slice, `cargo test --locked --workspace --all-targets` passes 75
tests on the clean Linux verification checkout. Host identity private-key
storage and the physical QR/SAS interface remain deliberately unimplemented.

## Single-instance store ownership

The daemon now acquires an OS-owned exclusive lock before opening its state,
command, or pairing databases. A concurrent connector using the same store
configuration fails closed. The marker records only the process ID and start
time and is retained across exits; operating-system lock ownership, not file
presence, identifies the live process.

Focused tests prove contention and reacquisition after drop. A Linux
process-level smoke test also starts a real daemon, rejects a second process,
stops the owner with SIGINT, and verifies that a new daemon reacquires the same
lock and restores its persisted projection.

After this slice, `cargo test --locked --workspace --all-targets` passes 77
tests on the clean Linux verification checkout.

## Journal compaction/restart correctness

The event journal now preflights an existing database read-only with
`quick_check` before any schema write. Corrupt evidence is rejected without
being overwritten. Reopen restores the SQLite AUTOINCREMENT high-water mark,
so deleting every acknowledged event cannot reset the connector sequence.

Replay now rejects cursors ahead of the connector and reports a gap when all
events after a stale cursor were compacted. That forces snapshot replacement
instead of incorrectly treating an empty replay as synchronized state. A
two-run daemon smoke verifies the read-only preflight against a real WAL
database and restores the projection on the second run.

After this slice, `cargo test --locked --workspace --all-targets` passes 79
tests on the clean Linux verification checkout.

## OS-protected host identity and pin

The connector's long-term Ed25519 seed now has a versioned native-secret-store
implementation for Windows Credential Manager, macOS Keychain Services, and
Linux Secret Service. New keys are generated randomly, read back before use,
and held without a plaintext-file fallback. Temporary serialized secret bytes
are zeroized.

The pairing database separately pins the logical host ID and public key. A
legacy signed registry must verify before receiving its first pin, and later
host-ID or key replacement is rejected. If the native store is unavailable or
the pin mismatches, the daemon continues local non-secret mirroring while
keeping authenticated pairing disabled.

After this slice, `cargo test --locked --workspace --all-targets` passes 85
tests on the clean Linux verification checkout. A forced-invalid Secret Service
endpoint smoke verifies locked-store startup, shutdown, and projection restore.
Native desktop integration tests and reviewed headless unlock/recovery backends
remain required.

## Cross-platform atomic vault replacement

Vault persistence no longer implements replacement with
`std::fs::rename`, which cannot overwrite an existing destination on Windows.
It now uses a reviewed same-directory atomic writer that fsyncs and commits the
replacement on Windows and Unix. Unix writes explicitly recreate the vault as
owner-only `0600`; the credential-vault lifecycle test verifies that mode after
multiple updates.

The locked workspace remains at 85 passing tests after this change.

## Major work still required

- Prove OpenCode credential profile isolation with managed runtimes,
  compatibility fixtures, stage/validate/activate/rollback, and restart tests.
- Add Codex compatibility fixtures across supported CLI versions and cover
  permission-profile/tool/MCP request shapes where the mobile protocol can
  represent them safely.
- Add reviewed headless host-identity unlock/recovery and rotation flows, then
  expose the existing authenticated pairing and revocation primitives through
  a bounded transport endpoint and physical QR/SAS interface.
- Bind the encrypted persistent credential vault to OS key storage and connect
  its atomic stage/validate/activate/rollback operations to provider-specific
  managed runtimes.
- Implement direct and optional relay command/event transport and wire decoded
  authenticated envelopes into the existing persistent command router.
- Complete supervised-process monitoring, graceful shutdown, exponential
  backoff, process identity checks, and adopted-runtime behavior.
- Replace hardcoded Flutter demo data with state management, encrypted transport,
  secure storage, biometric gates, pairing, lifecycle recovery, and failure UI.
- Add integration, compatibility, chaos, physical-device, accessibility,
  external security, release-signing, deployment, and beta evidence required by
  `PLAN.md`.
