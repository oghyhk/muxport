# ADR-0001: Flutter/Rust Boundary and Transport Architecture

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Muxport requires a high-performance cross-platform mobile client (iOS/Android) and host daemon/relay components written in Rust. We need a clear architectural boundary between Flutter (Dart UI layer) and Rust (security-sensitive protocol, encryption, state management, daemon logic).

## Decision

1. **Host Daemon & Relay:** Pure Rust workspace (`crates/connector`, `crates/connector-core`, `crates/muxport-crypto`, etc.).
2. **Mobile Client:** Flutter application (`apps/mobile`) using Dart for rendering and state management. Protocol communication between phone and host connector occurs over E2EE WebSockets (via relay) or direct E2EE TCP/HTTP sockets (on LAN/private network).
3. **Shared Protocol Types:** Canonical schemas defined in `protocol/schema/muxport.proto`. Rust code generated via `prost` / `tonic-build`, Dart code generated via `protoc-gen-dart`.
4. **Mobile FFI Boundary (Optional):** Platform-native secure key storage (Keychain / Android Keystore) accessed natively via Dart platform channels / secure storage packages. E2EE payload encryption can run natively in Dart using audited crypto primitives or via `mobile-core` Rust FFI bindings.

## Consequences

- Clear separation of concerns: Dart owns mobile rendering and state presentation; Rust owns connector orchestration, process supervision, vault encryption, and host networking.
- Single source of truth for protocol messages via Protobuf definitions.
