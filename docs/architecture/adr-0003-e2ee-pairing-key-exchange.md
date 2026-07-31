# ADR-0003: E2EE Pairing, Identity, and Frame Encryption

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Muxport mobile clients connect to host connectors directly or via an opaque relay. The relay must never have access to application data, prompts, diffs, approvals, or host credentials.

## Decision

1. **Identity & Key Primitives:**
   - Every host and mobile device generates a long-term Ed25519 identity keypair and X25519 keypair for key exchange.
2. **Pairing Flow:**
   - Out-of-band short-lived QR code containing `host_id`, `rendezvous_token`, `ephemeral_public_key`, `expiry`, and `host_fingerprint`.
   - ECDH over X25519 + HKDF-SHA256 to establish initial pairing secret.
   - Display 6-digit human-verifiable SAS (Short Authentication String) code on both phone and host UI.
3. **Session Transport Framing:**
   - Session frames encrypted using ChaCha20-Poly1305 / AES-256-GCM.
   - Associated Data (AAD) binds frame sequence, protocol version, host ID, device ID, and direction to prevent replay, reordering, or cross-host reflection.
4. **Revocation:**
   - Host vault stores list of authorized paired device public keys.
   - Device revocation removes public key from host whitelist without invalidating host provider credentials.

## Consequences

- Opaque relay only sees routing metadata (`connection_id`, encrypted binary payload length).
- Replayed or out-of-sequence frames fail AEAD decryption immediately.
