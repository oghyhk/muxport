# ADR-0004: Host Vault Encryption and OS Key Integration

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Host connector manages sensitive provider credentials (OpenCode Go keys, Codex API keys, vendor tokens). Credentials must be encrypted at rest and stored securely across operating systems.

## Decision

1. **Vault Envelope:**
   - Provider keys are stored in encrypted vault file `vault.sealed` using
     ChaCha20-Poly1305.
   - A random Data Encryption Key (DEK) encrypts the payload and individual
     secret slots; the Key Encryption Key (KEK) wraps the DEK.
   - Domain-separated authenticated data binds the envelope to its host and
     each secret to host, profile, provider, credential type, and format
     version.
2. **OS Key Store Integrations:**
   - macOS: Apple Keychain Services API.
   - Windows: Windows DPAPI (Data Protection API) / Credential Manager.
   - Linux Desktop: Secret Service API (Freedesktop SecretService / KWallet).
   - Linux Server / Headless / Container: Argon2id passphrase derivation, TPM2.0 / systemd credentials integration, or environment master key parameter.
3. **Memory Safety & Process Isolation:**
   - Decrypted credentials zeroized in memory immediately after use (`zeroize` crate in Rust).
   - Plaintext credentials strictly denied from entering logs, crash dumps, event journals, or remote mobile caches.
4. **Durable file replacement:**
   - Vault updates use a same-directory, cross-platform atomic replacement.
   - Unix vault files are recreated with owner-only `0600` permissions rather
     than inheriting a permissive mode.
5. **Migration:**
   - A valid v1 envelope is migrated by decrypting every active, staged, and
     rollback slot and re-encrypting it under a fresh v2 DEK.
   - Atomic replacement occurs only after the complete v2 envelope is sealed.

## Consequences

- Protection against offline file extraction on desktop/server hosts.
- Headless servers supported safely without hardcoding plain keys in repository files.
- An interrupted update preserves either the previous complete envelope or the
  new complete envelope; Windows updates do not depend on `std::fs::rename`
  overwriting an existing destination.
