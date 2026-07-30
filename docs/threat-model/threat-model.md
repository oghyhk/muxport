# Muxport Threat Model and Security Architecture

## 1. System Boundaries & Assets

### Assets
1. **Provider Credentials:** OpenCode Go keys, Codex API keys, ChatGPT session credentials.
2. **Agent Context & Data:** Source code, diffs, terminal outputs, prompts, project files.
3. **Control Flow:** Approvals for tool calls, code modifications, system commands.
4. **Host & Mobile Identity Keys:** Long-term Ed25519 identity keypairs and paired device registries.

### Trust Zones
- **Host Vault (Trusted):** Stores encrypted provider secrets and local SQLite databases (`metadata.db`, `events.db`, `audit.db`).
- **Mobile Device (Trusted):** Stores device identity key in Keychain/Keystore; handles UI, biometric gates, and user approvals.
- **Relay (Untrusted / Semi-trusted for routing):** Opaque WebSocket relay routing encrypted binary frames. Has ZERO decryption capability for session payloads or secrets.
- **Agent Runtimes (Host Local):** Supervised child processes (OpenCode HTTP/SSE server, Codex App Server stdio JSON-RPC).

## 2. Threat Analysis & Mitigations

| Threat Vector | Mitigation Strategy | Verification / Controls |
|---|---|---|
| Malicious / Compromised Relay | End-to-end ChaCha20-Poly1305 encryption with AAD binding host ID, device ID, epoch, sequence, and direction. | Relay receives only opaque binary envelopes. Cannot decrypt payloads or inject commands. |
| Replay Attacks | Monotonic sequence counters per direction, boot epoch validation, small sliding replay window. | Replayed or out-of-sequence frames fail AEAD decryption immediately. |
| Man-in-the-Middle (MITM) | Out-of-band QR pairing with X25519 ECDH + HKDF-SHA256 and mandatory SAS 6-digit verification code. | Pairing materials short-lived; identity keys pinned after initial pairing. |
| Host Memory / Disk Leakage | Credentials encrypted at rest with AES-256-GCM (KEK from OS Keychain/DPAPI/SecretService); memory buffers zeroized after use. | Secret scanner in CI; logs redact auth headers and tokens. |
| Unintended Credential Switch | Immutable session bindings (`credential_profile_id`); account assignment changes apply to NEW sessions only. | Turn execution verifies session binding against profile ID before dispatch. |
| Malicious Mobile Approval Request | Approval IDs bound to session and action type; connector re-verifies source state before confirming approval. | Double approvals or stale approvals rejected at host level. |
| Unauthorized Stolen Mobile Device | Device revocation API on host vault removes device identity public key instantly. | Host rejects connection attempts from revoked public keys. |

## 3. Security Test Plan

- **Protocol Parser Fuzzing:** Malformed envelope, truncated payload, invalid protobuf tags.
- **Replay & Reflection Tests:** Resending recorded frames over relay or local sockets.
- **Vault Tamper Tests:** Bit flips in `vault.sealed` ciphertext or corrupt key metadata.
- **Secret Scan Automation:** `scripts/scan_secrets.sh` execution in CI pipeline.
