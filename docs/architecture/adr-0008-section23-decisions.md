# ADR-0008: Resolution of Key Architecture and Scope Decisions (Section 23)

- **Status:** Proposed — vendor isolation and minimum-version decisions require validation
- **Date:** 2026-07-30
- **Context:** `PLAN.md` Section 23 listed 10 key architectural and policy choices that require formal decisions prior to v1 delivery. This ADR records working defaults, not proof that vendor behavior or release support has been validated.

## Proposed decisions

1. **Mobile Secret Provisioning:** Secret entry on mobile is permitted only via explicit E2EE direct host encryption, with biometric authentication required before dispatch. Local host vault enrollment remains the recommended primary flow.
2. **Vault Replication:** Multi-host environments maintain isolated per-host vaults. Secrets are provisioned individually to each selected host vault rather than replicating a shared vault database across machines.
3. **E2EE & Pairing Primitives:** Standardized on Ed25519 identity keys, X25519 ECDH, HKDF-SHA256, 6-digit Short Authentication String (SAS) out-of-band verification, and ChaCha20-Poly1305 frame AEAD with monotonic per-direction nonces.
4. **Wire Protocol Tooling:** Protobuf v3 defined in `protocol/schema/muxport.proto`, compiled to Rust via `prost` and Dart via `package:protobuf`.
5. **Flutter Architecture:** Feature-first modular structure (`onboarding`, `hosts`, `sessions`, `approvals`, `accounts`, `rotation`, `settings`, `diagnostics`) using Flutter `ChangeNotifier` state management and native secure storage.
6. **Vendor Profile Isolation:** Candidate designs use separate supported state/config roots and one process per profile. The exact OpenCode and Codex mechanisms remain provisional until the Phase 0 isolation spikes pass. Muxport must not copy, rewrite, or share vendor token databases to implement isolation.
7. **Quota-Triggered Rotation Policy:** Automatic rotation is feature-gated by default and requires explicit user opt-in, strict cooldowns, and compliance with vendor terms of service.
8. **Adopted vs. Managed Sessions:** Managed sessions started by Muxport connector support full interactive real-time control, streaming, and approvals. Externally started live sessions are read-only best-effort.
9. **Relay Infrastructure:** Opaque WebSockets with zero decryption access, minimal presence tracking, and strict rate/frame limits.
10. **Minimum Platform Support:** Candidate OS floors are Windows 10+, macOS 12+, Linux with glibc 2.31+, iOS 15+, and Android 8+ (API 26+). OpenCode and Codex version floors remain undecided until contract and recovery fixtures establish a support window.

## Consequences

- Establishes defaults for implementation spikes while preserving explicit uncertainty around vendor isolation, compatibility, and release support.
