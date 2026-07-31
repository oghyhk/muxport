# Mobile direct transport client

- **Status:** Authenticated command client implemented; app integration pending
- **Reviewed:** 2026-07-30

The Flutter package now contains a real client for the connector's opt-in
direct TCP endpoint. It validates the connector's signed, expiring challenge
against the cached host ID and pinned Ed25519 key before it sends the device
identity or accepts any host response.

The client then:

1. signs an initiator hello with the OS-protected mobile Ed25519 identity;
2. performs ephemeral X25519 key agreement;
3. verifies the responder hello and complete transcript binding;
4. derives directional keys and nonce prefixes with HKDF-SHA-256;
5. encodes the shared protobuf schema and exchanges ordered
   ChaCha20-Poly1305 command/result frames; and
6. fixes the connector boot epoch for the lifetime of the encrypted session.

Only one command is allowed in flight on a connection. This matches the
current connector's request/result loop and avoids ambiguous response routing.
Frame size, route, protocol version, sequence, command ID, host identity, and
boot epoch are checked before a command result is returned to the app.
In-memory session keys are explicitly destroyed on close.

The protobuf Dart files under
`packages/flutter_protocol/lib/src/generated/` are generated from
`protocol/schema/muxport.proto`. Run `scripts/generate_protocol.ps1` after
installing `protoc` and `protoc-gen-dart`; do not hand-edit generated files.

## Verification and limits

Flutter tests run a real loopback TCP exchange with an independent fake host:
challenge verification, Ed25519/X25519 handshake, HKDF derivation, protobuf
probe, ChaCha20-Poly1305 result, host-key mismatch rejection, and replay
rejection are exercised.

`protocol/fixtures/direct_session_v1.json` is a committed golden vector for the
directional key schedule, session AAD, protobuf probe envelope, ordered nonce,
ChaCha20-Poly1305 ciphertext, and `MUX1` framing. Both the Dart and Rust suites
recompute and compare every field. Regenerate candidate JSON with
`dart run tool/generate_direct_fixture.dart` from `apps/mobile`, then review the
fixture diff rather than overwriting it automatically.

The connection is not yet launched from the app because pairing and host
endpoint management UI do not exist. Native phone-to-Rust interoperability,
reconnect/retry orchestration, snapshot/replay/event traffic, online
revocation, background execution, and hostile-network testing remain required.
