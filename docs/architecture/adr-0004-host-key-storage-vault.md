# ADR-0004: Host Vault Encryption and OS Key Integration

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Host connector manages sensitive provider credentials (OpenCode Go keys, Codex API keys, vendor tokens). Credentials must be encrypted at rest and stored securely across operating systems.

## Decision

1. **Vault Envelope:**
   - Provider keys stored in encrypted vault file `vault.sealed` using AES-256-GCM.
   - Key Encryption Key (KEK) wraps Data Encryption Key (DEK).
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

## Consequences

- Protection against offline file extraction on desktop/server hosts.
- Headless servers supported safely without hardcoding plain keys in repository files.
- An interrupted update preserves either the previous complete envelope or the
  new complete envelope; Windows updates do not depend on `std::fs::rename`
  overwriting an existing destination.
