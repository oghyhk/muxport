# Credential Vault v2

- **Status:** Implemented encrypted storage foundation; runtime wiring pending
- **Reviewed:** 2026-07-30

## Envelope

Every new vault generates a random 256-bit data-encryption key (DEK). The caller
supplies a key-encryption key (KEK), currently derivable with Argon2id for the
reviewed passphrase/headless path. The v2 file contains only:

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

## Migration and restart

A valid v1 envelope is decrypted with its original KEK, and every active,
staged, and rollback slot is re-encrypted under a fresh v2 DEK and contextual
authenticated data. The v2 envelope atomically replaces v1 only after the
entire migration succeeds; failure leaves the original file intact.

V2 reopen tests cover the correct host and KEK, wrong-host rejection, wrong-KEK
rejection, metadata confidentiality, owner-only Unix permissions, lifecycle
recovery, and ciphertext context swapping.

## Remaining integration

- Wrap or obtain the provider-vault KEK from the native desktop store instead
  of relying only on the passphrase constructor.
- Add reviewed systemd credential, TPM, and external secret-manager KEK
  providers for headless hosts.
- Connect enrollment and stage/validate/activate/rollback to managed OpenCode
  and Codex profile directories.
- Add non-reversible keyed duplicate fingerprints, explicit local-disable
  metadata, upstream-revocation status, and audited deletion.
- Add native Windows/macOS/Linux crash and backup/restore tests.
