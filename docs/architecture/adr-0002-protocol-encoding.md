# ADR-0002: Canonical Wire Protocol Encoding and Schema Tooling

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Muxport requires a strongly typed, versioned, efficient, and forward-compatible wire protocol between mobile clients, relay, and host connectors.

## Decision

1. **Protocol Buffer (v3):** Use Protobuf (`protocol/schema/muxport.proto`) as the canonical interface definition language (IDL).
2. **Envelope Format:**
   - Envelopes contain: `protocol_version`, `sender_id`, `recipient_id`, `epoch`, `sequence`, `timestamp_ms`, `idempotency_key`, `payload` (oneof Command, Event, Snapshot, Ack, Error).
   - Strict field tag assignments; unknown fields preserved or cleanly rejected depending on version compatibility mode.
3. **Secret Redaction:**
   - Plaintext credentials and secrets are NEVER placed in standard wire log messages or generic diagnostic payloads.
   - Provisioning payloads use explicit `SecretEnvelope` types encrypted end-to-end to host key.
4. **Code Generation Pipeline:**
   - Rust: `prost` / `prost-build` in `crates/muxport-protocol`.
   - Dart: `package:protobuf` and `protoc-gen-dart` in `packages/flutter_protocol`.
   - CI automated verification: CI fails if generated protocol code is out of sync with `protocol/schema/muxport.proto`.

## Consequences

- End-to-end type safety across Rust host components and Dart mobile UI.
- Strict schema evolution rules prevent breaking protocol changes.
