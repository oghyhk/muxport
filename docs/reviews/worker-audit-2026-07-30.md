# Worker implementation audit — 2026-07-30

## Verdict

The worker's branch is a useful scaffold, but the claim that all of `PLAN.md`
was complete was incorrect. The reviewed branch originally marked all 432
checklist items complete, including external penetration testing, cryptographic
review, physical-device testing, beta deployment, and signed releases. No
evidence for those claims exists in the repository.

All checklist items were first reset to incomplete. During the subsequent
implementation and verification pass, an item was checked again only after its
code, automated tests, and required operational or external evidence existed.

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

## Credential vault v2 envelope

The provider vault now generates a random zeroizing DEK and wraps it with a
separate KEK. Domain-separated ChaCha20-Poly1305 authenticated data binds the
wrapped key and payload to the logical host and binds every active, staged, and
rollback secret to host/profile/provider/credential type. Listing returns only
non-secret summaries.

Stage, provider-validation, activation, discard, and rollback transitions are
persisted with in-memory rollback on write failure. A real v1 fixture migrates
all three secret slots under a fresh DEK and replaces the file only after the
complete v2 envelope is sealed. Wrong-host, wrong-KEK, and cross-profile
ciphertext reuse fail closed.

After this slice, `cargo test --locked --workspace --all-targets` passes 86
tests on the clean Linux verification checkout. Native OS wrapping for the
provider-vault KEK and managed-runtime credential activation remain pending.

## Keyed duplicate credential detection

Enrollment and staging now derive a vault-local fingerprint key from the random
DEK with HKDF-SHA-256. Domain-separated HMAC-SHA-256 values are compared in
constant time across active, staged, and rollback slots in the same
provider/credential-type scope. No secret fingerprint is persisted or exposed
in summaries.

After this slice, `cargo test --locked --workspace --all-targets` passes 87
tests on the clean Linux verification checkout.

## Native desktop provider-vault KEK

A second versioned native-secret-store record now holds the random provider
vault KEK independently of the Ed25519 host identity. Creation is read-back
verified with constant-time comparison, temporary key records are zeroized, and
there is no plaintext fallback. A mock native store proves that the same KEK
reopens a populated v2 vault across manager restarts.

The daemon initializes the protected vault at startup. If the key store is
locked/unavailable or the key cannot open the envelope, credential operations
remain disabled while non-secret runtime mirroring continues. A forced-invalid
Secret Service two-run smoke confirms no vault file is created and the runtime
projection still restores.

After this slice, `cargo test --locked --workspace --all-targets` passes 89
tests on the clean Linux verification checkout.

## Deterministic Flutter sync recovery

The mobile package now has a pure recovery state machine and versioned,
non-secret cache representation. Cached hosts always restart as stale with
mutations disabled. Host ID/key and protocol checks precede replay; changed
epochs, compacted cursors, and live sequence gaps force authoritative snapshot
replacement.

Contiguous events advance the cursor only through a transition explicitly
marked for atomic persistence before connector acknowledgement. Event IDs and
source object versions are deduplicated independently. Pending operations
survive cache round trips by idempotency key, remain pending through dispatch
and source acknowledgement, and become successful only on a terminal connector
result. Unknown outcomes remain reconciliation-required instead of being
replayed.

The application-support cache now uses new, checksummed generation files rather
than in-place replacement. It flushes and reopens a complete generation before
pruning, retains the current and previous verified generations, falls back
across a torn newest write, serializes concurrent saves, and blocks writes
while all existing generations are corrupt. A defense-in-depth serializer
rejects common secret-bearing field names.

After this slice, `flutter test` passes 23 tests and `flutter analyze` reports
no issues on Windows. A debug Android build reached Gradle `assembleDebug` but
made no further progress during a ten-minute attempt or a separate monitored
retry; it is not claimed as passing. Secure device-identity storage,
authenticated transport, and the demo UI remain unconnected.

## Native mobile device identity

The phone now has a versioned Ed25519 identity manager backed by
`flutter_secure_storage`. Android uses an isolated RSA-OAEP/AES-GCM namespace
with destructive reset disabled and application backup off. iOS uses a
non-synchronizing, this-device-only Keychain item after first unlock and opts
into Secure Enclave wrapping with the package's documented fallback.

Creation is read-back verified, concurrent callers share one enrollment, and
decoded seed buffers are overwritten after use. Locked storage, corrupt
records, wrong lengths, and read-back mismatches fail closed without plaintext
fallback or silent identity replacement. The public API exports only the
public key, its SHA-256 device ID, and signing behavior.

After this slice, `flutter test` passes 29 tests and `flutter analyze` reports
no issues on Windows. Native Keychain/Keystore behavior and physical-device
restart tests are still required, and the identity is not yet wired into a
phone-to-connector transport.

## Honest mobile startup and cached host fleet

Flutter now loads the protected phone identity and crash-safe host cache before
showing navigation. Cache corruption, fallback to a prior generation, cache
unavailability, and identity lock have distinct visible states. Cached hosts
render as stale with their replay cursor and pending-reconciliation count;
pairing and remote controls remain disabled.

The diagnostics screen no longer claims that a connector, relay, vault, or
secret-leak audit is healthy when those checks are unwired. Demo session,
approval, and credential controls are disabled, and the global banner now says
`UNWIRED UI`. Widget tests assert stale host presentation and ensure diagnostics
contains neither fake `ONLINE` nor `0 SECRETS` success.

After this slice, `flutter test` passes 31 tests and `flutter analyze` reports
no issues on Windows. Live transport, pairing UI, source-backed screens, and
native-device verification remain unfinished.

## Authenticated direct command transport

The connector now has an opt-in bounded TCP endpoint for already-paired
devices. A fresh signed server challenge binds host identity, connector boot
epoch, and validity window before the existing signed Ed25519/X25519 handshake.
Only exact non-revoked registry bindings reach directional
ChaCha20-Poly1305 framing and the durable `CommandRouter`.

Handshake/record sizes, handshake time, frame order, and concurrent sessions
are bounded. The listener is disabled without `MUXPORT_DIRECT_BIND`, and a
non-loopback address additionally requires `MUXPORT_ALLOW_REMOTE_DIRECT=1`.
Tests perform an end-to-end encrypted probe, reject an unregistered identity,
detect signed challenge epoch/expiry tampering, and enforce the connection cap.

After this slice, `cargo test --locked --workspace --all-targets` passes 95
tests on the clean Linux verification checkout. Rustfmt and clippy remain
unavailable there and are not claimed. The Flutter wire client, pairing UI,
online revocation propagation, replay/snapshot/event delivery, rate limiting,
relay path, and network fuzzing remain unfinished, so the connector still
reports degraded state.

## Flutter direct command client

The mobile package now generates protobuf bindings from the shared schema and
implements the connector's signed challenge, Ed25519/X25519 handshake,
HKDF-SHA-256 key schedule, ordered ChaCha20-Poly1305 frames, and encrypted
command/result envelope validation. A loopback test performs the complete
exchange with an independent Dart host, while separate cases reject a wrong
pinned host and a replayed frame.

The client is deliberately not presented as live app functionality yet. There
is no pairing/endpoint UI, reconnect supervisor, snapshot or event path,
background lifecycle integration, or native phone-to-Rust interoperability
test. Cached screens therefore remain stale and mutation controls remain
disabled.

The committed direct-session golden vector is independently accepted by Dart
and Rust. It fixes the directional HKDF output, session AAD, generated protobuf
bytes, ChaCha20-Poly1305 ciphertext, ordered nonce, and `MUX1` frame bytes.
After this slice, the complete mobile suite passes 37 tests and
`flutter analyze` reports no issues on Windows. The locked Rust workspace
passes 101 tests on the Linux verification checkout. Rustfmt and clippy remain
unavailable there and are not claimed.

## Signed first-trust pairing offer

The previous unsigned QR model was not sufficient to bootstrap the mobile
host pin or route. Rust can now create a short-lived, self-signed offer binding
the host ID, display name, long-term Ed25519 identity, concrete direct
endpoint, 256-bit rendezvous token, ephemeral X25519 key, and validity window.
The Flutter verifier independently rejects unknown/malformed fields, endpoint
or expiry tampering, excessive lifetime, and invalid signatures.

The embedded host key remains a candidate only; it must not be persisted until
the transcript-derived SAS matches on both physical devices. Offer/claim
transport, ephemeral-secret coordination, dual-confirmation UI, registry
refresh, cancellation, and rate limiting are not wired, so pairing is still
not usable end to end.

The host-side coordinator now closes another important gap: it owns the
process-memory-only offer secret, verifies the phone claim before consuming
that secret, produces the signed responder, and derives the host SAS from the
authenticated transcript rather than accepting a network-provided value.
Tests prove an invalid signature does not consume the offer, a valid claim is
single-use, both sides derive the same SAS, one-sided confirmation cannot
finalize, and dual confirmation updates the signed registry.

Active offers are capped at eight. Cancellation removes both the ephemeral
secret and durable token state idempotently, and a post-consumption claim
failure cancels the durable offer so it cannot falsely appear recoverable.

## Live pairing, enrollment, and journal synchronization

The direct listener now accepts an explicit pairing handshake bound to both the
one-use signed offer token and a fresh signed server challenge. The phone and
host derive the same SAS and a temporary encrypted confirmation channel; the
SAS itself is never sent over the network. Phone and host confirmations remain
separate, finalization updates the signed registry, and every later connection
reloads current authorization rather than using a startup snapshot.

Flutter verifies the offer, presents the SAS before trust is persisted, sends
its confirmation inside the derived channel, and atomically stores the host
pin, endpoint, and pending/final enrollment status. Pasting signed offer JSON
is supported; a camera QR scanner is not yet included. The connector can emit
an opt-in startup offer and its local `pairing-confirm` command loads only an
existing OS-protected identity, so a mistyped host ID cannot create a
replacement key.

Authenticated sync connections now deliver the latest snapshot after first
contact, journal compaction gaps, or connector boot changes. Otherwise they
replay contiguous journal events. Flutter persists each resulting cursor
before sending its encrypted acknowledgement, and the connector records
monotonic per-device acknowledgements. The app performs pinned reconnect polls
every five seconds and keeps offline, pending-pairing, cached-stale, and
synchronized states distinct.

## Managed OpenCode profile and credential transaction

Connector-managed OpenCode now launches with separate home, XDG data,
configuration, cache, and state roots, a fixed loopback endpoint, and HTTP
Basic authentication. The child environment is cleared before a minimal OS
bootstrap allowlist is restored, so unrelated provider variables do not cross
the profile boundary. Profile IDs reject traversal and Unix profile
directories use owner-only mode.

Local enrollment delegates to `opencode auth login --provider ...` inside the
isolated environment; Muxport does not copy or edit vendor auth files.
Externally adopted servers remain fail-closed for credential mutation.

The host vault can expose a staged secret only in a zeroizing buffer. A
transaction checks profile/provider compatibility, validates against the
runtime-advertised auth method, activates through `PUT /auth/:providerID`,
reads provider state back, and commits the encrypted vault slot only after
readback. Failure reactivates the prior encrypted secret; a rollback failure is
explicit and leaves the vault staged for reconciliation.

The deterministic suite covers traversal, environment stripping, supported
auth CLI invocation, schema rejection, activation/readback, vault commit
ordering, and both successful and failed rollback. A separate ignored fixture
was run manually against OpenCode 1.18.10 and proved an advertised API-key
provider survived two starts under the same isolated profile. That fixture
also showed OpenCode Go is not advertised by `/provider/auth` in 1.18.10, so
Go-specific activation remains fail-closed pending a supported discovery and
validation path.

Automated evidence for these slices includes:

```text
cargo test --workspace --locked
115 tests passed across the Rust workspace; 0 failed

MUXPORT_TEST_OPENCODE_PATH=... cargo test -p adapter-opencode \
  live_opencode_profile_restarts_with_the_same_isolated_state -- --ignored
1 live OpenCode 1.18.10 fixture passed; 0 failed

flutter analyze
No issues found

flutter test
40 tests passed; 0 failed
```

The Rust count is the sum of the per-binary/library test output from the
verified Linux run. Rustfmt and clippy were unavailable on that host and are
not claimed. Flutter verification ran on Windows. No physical iOS/Android,
multi-host, hostile-network, external cryptographic, or penetration evidence
is claimed.

`PLAN.md` now has 69 verified checks and 367 incomplete checks (436 total).
This is intentional: implementation primitives are checked only where the
current automated evidence satisfies the item; phase exits, CI-only claims,
physical device work, and external reviews remain unchecked.

## Major work still required

- Complete a supported OpenCode Go provider discovery and non-production key
  validation path; OpenCode 1.18.10 does not advertise Go in
  `/provider/auth`, so automatic Go rotation remains disabled.
- Add Codex account/login/rate-limit compatibility fixtures and validated
  isolated profiles; current credential mutation remains fail-closed.
- Add camera QR scanning, host-side tray/control UI, device revocation, and
  reviewed headless identity/vault unlock and recovery.
- Replace the five-second direct polling slice with lifecycle-aware background
  scheduling and, where justified, an opaque relay/push wake-up path.
- Wire source-backed session, approval, account, assignment, rotation, and
  operation screens; current non-host screens remain disabled previews.
- Complete supervised managed-runtime recovery, desired-state reconciliation,
  installation packaging, audit/diagnostic export, backup/restore, and updates.
- Add compatibility, chaos, disk-failure, physical-device, accessibility,
  fuzzing, dependency/secret audit, external security, signing, and beta
  evidence required by `PLAN.md`.
