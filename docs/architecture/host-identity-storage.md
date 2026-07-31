# Host Identity Storage and Pinning

- **Status:** Implemented desktop foundation; headless unlock and recovery UI pending
- **Reviewed:** 2026-07-30

## Purpose

Every connector needs a stable Ed25519 identity before it can authenticate a
phone, sign its device registry, or participate in a transcript-bound session.
The private key must survive ordinary restarts without becoming a plaintext
file beside the connector databases.

## Protected storage

`credential-vault` exposes a narrow host-identity secret-store interface. The
desktop implementation stores one versioned 32-byte Ed25519 seed in the native
credential store:

- Windows Credential Manager
- macOS Keychain Services
- Linux Secret Service

The credential-store username is a domain-separated SHA-256 digest of the
logical host ID, so arbitrary host names are not used as native key names. A
new identity is generated with the operating-system RNG, written once, read
back, and compared by public key before use. Temporary encoded secret buffers
are zeroized. There is no compiled key, deterministic derivation, or plaintext
file fallback.

## Public-key pin

The pairing SQLite database stores the logical host ID and Ed25519 public key
as a non-secret pin. On every connector start:

1. Acquire the connector instance lock.
2. Restore the logical host ID from the event journal.
3. Load or create the private identity through the native credential store.
4. Match its public key and host ID against the pairing-database pin.
5. If migrating an older database with a signed device registry, verify that
   registry with the candidate key before writing the first pin.

A different key or host ID cannot silently replace the pin. This protects
paired phones from trusting a new connector identity after key-store loss.

## Locked and unavailable behavior

If the native store is locked, absent, corrupt, or unsupported, the connector
continues its non-secret local runtime mirror but keeps authenticated pairing
disabled. A pin mismatch behaves the same way and requires an explicit future
identity-recovery or destructive re-pair workflow. Other pairing-database
errors remain fatal so corruption is not misreported as a locked key.

The following remain required before public beta:

- reviewed systemd credential, TPM, external secret-manager, or boot-passphrase
  backends for headless Linux;
- an explicit identity-loss recovery flow that revokes old trust and requires
  every phone to re-pair;
- host identity rotation messages signed by the old key when it is still
  available;
- platform integration and installer tests on Windows, macOS, and Linux
  desktops.
