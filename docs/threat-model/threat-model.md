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
| Malicious / Compromised Relay | End-to-end ChaCha20-Poly1305 encryption with AAD binding host ID, device ID, epoch, sequence, and direction. | Relay may drop, delay, reorder, replay, or inject random frames, but must not decrypt content or forge an authenticated command. |
| Replay Attacks | Monotonic sequence counters per direction and boot-epoch validation. The current ordered transport primitive rejects any sequence at or below the highest accepted value. | Replay, reflection, reconnect, and out-of-order behavior require dedicated tests before this control is complete. |
| Man-in-the-Middle (MITM) | Out-of-band QR pairing with X25519 ECDH + HKDF-SHA256 and mandatory SAS 6-digit verification code. | Pairing materials short-lived; identity keys pinned after initial pairing. |
| Host Memory / Disk Leakage | Credential and vault envelopes use ChaCha20-Poly1305. The KEK must ultimately be wrapped by Keychain, DPAPI, Secret Service, or a reviewed headless fallback; plaintext buffers are zeroized where supported. | OS key-store integration, log redaction, artifact scanning, and crash-report tests are still required. |
| Unintended Credential Switch | Immutable session bindings (`credential_profile_id`); account assignment changes apply to NEW sessions only. | Turn execution verifies session binding against profile ID before dispatch. |
| Malicious Mobile Approval Request | Approval IDs bound to session and action type; connector re-verifies source state before confirming approval. | Double approvals or stale approvals rejected at host level. |
| Unauthorized Stolen Mobile Device | Device revocation API on host vault removes device identity public key instantly. | Host rejects connection attempts from revoked public keys. |

## 3. Security Test Plan

- **Protocol Parser Fuzzing:** Malformed envelope, truncated payload, invalid protobuf tags.
- **Replay & Reflection Tests:** Resending recorded frames over relay or local sockets.
- **Vault Tamper Tests:** Bit flips in `vault.sealed` ciphertext or corrupt key metadata.
- **Secret Scan Automation:** Add repository and release-artifact scanning to CI after reviewing scanner scope and false-positive behavior.
