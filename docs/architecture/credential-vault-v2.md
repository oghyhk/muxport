# Credential Vault v2

- **Status:** Implemented encrypted storage foundation; runtime wiring pending
- **Reviewed:** 2026-07-30

## Envelope

Every new vault generates a random 256-bit data-encryption key (DEK). Desktop
connector startup loads or creates a separate random KEK in Windows Credential
Manager, macOS Keychain Services, or Linux Secret Service. The library also
supports Argon2id derivation for a future reviewed passphrase/headless path. The
v2 file contains only:

- format version and non-secret logical host ID;
- the DEK encrypted with ChaCha20-Poly1305 under the KEK;
- the complete credential payload encrypted with ChaCha20-Poly1305 under the
  DEK.

The DEK wrapping and payload use separate domain-separated authenticated-data
contexts that include the format version and length-prefixed host ID. Opening a
vault with a different logical host fails before credential data is released.
Both KEK and DEK buffers zeroize on drop.

## Credential records

Each active, staged, and rollback secret is separately encrypted with the DEK.
Its authenticated data binds:

- vault format version;
- host ID;
- profile ID;
- provider;
- credential type.

Moving ciphertext to another profile, provider, type, or host therefore fails
authentication. Secret size is bounded to 64 KiB. Public callers receive
`CredentialSummary` values only; ciphertext, nonces, and stored secret bytes are
not exposed through the listing API.

Stage, validate, activate, discard, and rollback mutations update memory first,
atomically replace the sealed file, and restore the prior in-memory record if
persistence fails. Activation is forbidden until the staged secret has an
explicit positive provider-validation timestamp.

Before enrollment or staging, the vault derives a dedicated fingerprint key
from the DEK with HKDF-SHA-256 and compares HMAC-SHA-256 values in constant
time. Active, staged, and rollback slots in the same provider/credential-type
scope are checked. The keyed fingerprints are computed only in memory and are
not persisted or displayed; identical bytes used for a different provider
scope are not treated as the same credential.

## Migration and restart

A valid v1 envelope is decrypted with its original KEK, and every active,
staged, and rollback slot is re-encrypted under a fresh v2 DEK and contextual
authenticated data. The v2 envelope atomically replaces v1 only after the
entire migration succeeds; failure leaves the original file intact.

V2 reopen tests cover the correct host and KEK, wrong-host rejection, wrong-KEK
rejection, metadata confidentiality, owner-only Unix permissions, lifecycle
recovery, and ciphertext context swapping.

If the native store is unavailable, corrupt, or contains a key that cannot open
the sealed vault, daemon startup continues in a credential-locked state unless
the operator explicitly configures the headless unlock source below. Credential
operations remain disabled and no plaintext or generated-file fallback is used.

### Headless Linux and container unlock

For a headless server, set `MUXPORT_VAULT_PASSPHRASE_FILE` to an **absolute,
regular, owner-private** credential file. This is intended for a systemd
`LoadCredential=` file, an equivalent TPM/external-secret-manager materialized
file, or a carefully managed container secret mount. The connector rejects
symlinks, empty files, files over 4 KiB, and (on Unix) any group/world-readable
file. It never reads a vault passphrase from a plaintext environment variable.

The complete file bytes are passed to Argon2id using a host-bound salt to derive
the KEK. The passphrase buffer is zeroized after derivation. Removing this
setting does not create a substitute OS key for an existing vault; the existing
vault simply remains locked, preserving evidence and preventing accidental key
replacement.

## Remaining integration

- Connect enrollment and stage/validate/activate/rollback to managed OpenCode
  and Codex profile directories.
- Add audited deletion and provider-specific upstream revocation workflows.
- Add native Windows/macOS/Linux crash and backup/restore tests.
