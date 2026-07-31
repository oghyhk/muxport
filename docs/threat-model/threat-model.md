# Muxport Threat Model and Security Architecture

Status: draft; controls listed below are requirements unless accompanied by verified test evidence.

## 1. System Boundaries & Assets

### Assets
1. **Provider Credentials:** OpenCode Go keys, Codex API keys, ChatGPT session credentials.
2. **Agent Context & Data:** Source code, diffs, terminal outputs, prompts, project files.
3. **Control Flow:** Approvals for tool calls, code modifications, system commands.
4. **Host & Mobile Identity Keys:** Long-term Ed25519 identity keypairs and paired device registries.

### Trust Zones
- **Host Vault (Trusted for normal operation):** Stores encrypted provider secrets and local SQLite databases (`metadata.db`, `events.db`, `audit.db`). A fully compromised host can access credentials while they are in use; Muxport cannot cryptographically protect a secret from the runtime that must consume it.
- **Mobile Device (Trusted):** Stores device identity key in Keychain/Keystore; handles UI, biometric gates, and user approvals.
- **Relay (Untrusted / Semi-trusted for routing):** Opaque WebSocket relay routing encrypted binary frames. Has ZERO decryption capability for session payloads or secrets.
- **Agent Runtimes (Host Local):** Supervised child processes (OpenCode HTTP/SSE server, Codex App Server stdio JSON-RPC).

## 2. Threat Analysis & Mitigations

| Threat Vector | Mitigation Strategy | Verification / Controls |
|---|---|---|
| Malicious / Compromised Relay | Directional ChaCha20-Poly1305 keys with AAD binding the host ID, device ID, signed handshake transcript, and exact frame sequence. The encrypted protobuf header independently validates sender, recipient, boot epoch, and sequence. | Unit tests reject replay, sequence gaps, wrong AAD, route mismatch, and boot-epoch changes. Relay/network end-to-end tests are still required. |
| Replay Attacks | Each direction accepts exactly the next sequence. A fresh signed ephemeral handshake creates new keys; the encrypted envelope fixes the remote boot epoch for that session. | Ordered-cipher and secure-envelope replay/out-of-order tests pass. Reconnect and persisted challenge-consumption tests remain required. |
| Man-in-the-Middle (MITM) | Ed25519-signed device/host handshake claims bind fresh X25519 keys, identities, route, nonce, challenge, and prior-message hash. The direct endpoint's initial signed challenge additionally binds the pinned host identity, connector boot epoch, and validity window. The first-trust QR self-signs the candidate host identity, endpoint, rendezvous token, ephemeral key, and expiry; that key is not trusted until the out-of-band SAS succeeds. HKDF and the SAS are bound to the complete signed transcript. Rendezvous tokens are 256-bit, stored only as hashes, claimed once transactionally, and require phone plus host SAS confirmation. | Offer endpoint/expiry tamper, signed-challenge epoch/expiry, wrong-challenge, token replay/expiry/cancellation, one-sided confirmation, restart finalization, revoked/unregistered-device, wrong-host, non-contributory-key, and encrypted direct-probe tests pass. Live QR/SAS UI and network fuzzing remain unimplemented. |
| Direct endpoint denial of service | Listener is disabled without an explicit bind address, non-loopback binding needs a second opt-in, records and handshake time are bounded, and at most 32 sessions are admitted. | Capacity and oversize framing tests exist. Per-source rate limiting, hostile-network soak tests, and operational firewall verification remain required. |
| Host Memory / Disk Leakage | Credential vault v2 uses a random zeroizing DEK wrapped by a separate random KEK. The KEK and long-term host Ed25519 seed are separate versioned entries in Windows Credential Manager, macOS Keychain Services, or Linux Secret Service with no plaintext fallback. Domain-separated ChaCha20-Poly1305 contexts bind the envelope to the host and each secret to host/profile/provider/type. | Wrong-host/KEK, context-swap, v1 migration, rollback, permissions, mock protected-store reopen, and locked-store daemon tests pass. Headless unlock, native platform integration, log redaction, artifact scanning, and crash-report tests remain required. |
| Unintended Credential Switch | Immutable session bindings (`credential_profile_id`); account assignment changes apply to NEW sessions only. | Turn execution verifies session binding against profile ID before dispatch. |
| Accidental Duplicate Enrollment | A vault-local key is derived from the random DEK with HKDF-SHA-256. Active, staged, and rollback secrets are compared with domain-separated HMAC-SHA-256 in constant time within the provider/type scope; no duplicate fingerprint is persisted. | Tests cover active, staged, rollback, same-profile, and cross-provider cases. Managed-runtime enrollment UI integration remains required. |
| Malicious Mobile Approval Request | Approval IDs bound to session and action type; connector re-verifies source state before confirming approval. | Double approvals or stale approvals rejected at host level. |
| Unauthorized Stolen Mobile Device | Signed device-registry snapshots retain pinned identities and revocation state; normal handshakes require an exact non-revoked binding. Pairing finalization and the signed registry update share one full-sync SQLite transaction. The OS-protected host key must also match a persisted public-key/host-ID pin. | Revocation, identity replacement, malformed key, registry tamper, wrong-host signature, host-pin mismatch, and pairing restart tests pass. Physical revocation UI and platform key-store tests remain required. |

## 3. Security Test Plan

- **Protocol Parser Fuzzing:** Malformed envelope, truncated payload, invalid protobuf tags.
- **Replay & Reflection Tests:** Resending recorded frames over relay or local sockets.
- **Vault Tamper Tests:** Bit flips in `vault.sealed` ciphertext or corrupt key metadata.
- **Secret Scan Automation:** Add repository and release-artifact scanning to CI after reviewing scanner scope and false-positive behavior.
