# Authenticated Session and Secure Envelope Foundation

- **Status:** Cryptographic/session codec and opt-in direct command listener implemented
- **Reviewed:** 2026-07-30

## Identity handshake

Normal reconnects use long-term Ed25519 host and device identities plus fresh
X25519 ephemeral keys. The initiating device signs its protocol version,
device ID, intended host ID, one-time challenge, identity key, ephemeral key,
and random nonce. The host verifies that signature and requires an exact,
non-revoked device-registry binding.

The host response signs its identity, ephemeral key, nonce, device/host route,
and the hash of the complete signed initiator message. The phone pins and
verifies the expected host identity key. Both peers then derive the same secret
from X25519 with an HKDF salt bound to the complete signed transcript.
Non-contributory X25519 public keys are rejected.

The pairing path may verify a self-asserted device signature before the device
is registered, but registration remains forbidden until the user confirms the
transcript-bound six-digit SAS out of band. Pairing-token generation,
single-use consumption, expiry, and confirmation state are persisted in a
dedicated SQLite/WAL store.

The QR model now self-signs the host identity, direct endpoint, single-use
token, ephemeral X25519 key, and validity window. The Flutter parser verifies
that signature before treating the included host key as a candidate pin.
Candidate trust is not persisted until the later SAS comparison. See
`signed-pairing-offer.md`.

Only a SHA-256 domain-separated hash of each 256-bit rendezvous token is stored.
Claiming uses an immediate transaction and accepts a signed
`VerifiedInitiator` exactly once. The phone and host confirmations are recorded
separately using a constant-time SAS-hash comparison; device registration
requires both.

Ephemeral X25519 secrets remain process-memory-only. On restart, unclaimed
offers and one-sided confirmations are cancelled instead of being resumed with
missing key material. A fully confirmed pairing can finalize after restart.
Finalization updates the host-signed device registry and the pairing state in
one SQLite transaction, so neither can commit without the other.

## Directional encryption

The transcript-bound shared secret expands to:

- an initiator-to-responder ChaCha20-Poly1305 key;
- a responder-to-initiator key;
- a unique four-byte nonce prefix for each direction.

Each direction starts at sequence one and accepts exactly the next sequence.
Replays, gaps, and out-of-order frames are rejected without advancing state.
The sequence is included in the AEAD associated data, and encrypted frame wire
encoding has an explicit magic, sequence, length, and caller-enforced size
limit.

## Secure protocol envelopes

After authentication, `SecureEnvelopeSession` binds AEAD to the host ID, device
ID, and handshake transcript. It validates:

- protocol version;
- authenticated sender and recipient IDs;
- nonzero and stable remote boot epoch;
- equality between encrypted-frame and protobuf-envelope sequences;
- configured ciphertext and plaintext limits;
- a nonempty idempotency key for every command;
- command-only payloads on the inbound command path.

Only an `AuthenticatedCommand` released by this codec may enter the durable
`CommandRouter`. Command results, snapshots, and events use the same encrypted
envelope codec in the opposite direction.

## Device registry integrity

The non-secret paired-device registry supports host-identity-signed snapshots.
Load verifies the expected host ID, pinned host public key, signature, schema
version, record key validity, and duplicate device IDs. Revoked identities
cannot be silently re-registered.

The signed registry is now stored atomically in the pairing database. The
long-term host Ed25519 seed is stored through Windows Credential Manager,
macOS Keychain Services, or Linux Secret Service and matched to a public-key
pin in the pairing database. A locked/unavailable store or pin mismatch disables
pairing without a plaintext fallback. Headless unlock/recovery backends and the
live offer/claim/confirmation transport plus QR/SAS UI remain pending.

## Direct listener boundary

The opt-in direct listener sends a signed, expiring server challenge before the
existing initiator/responder handshake. The signature binds the pinned host
identity, challenge, boot epoch, and validity window. Only a device already in
the non-revoked registry can establish directional encryption and dispatch a
durably idempotent command. Length, timeout, and concurrent-session bounds are
enforced before command decoding.

The listener is disabled by default and separately gates non-loopback binds.
It does not yet implement pairing, app-level connection management, snapshot
replay, live events, acknowledgements, online revocation propagation, or a
relay path. The Flutter command client is described in
`mobile-direct-client.md`; the connector listener is described in
`direct-command-transport.md`.
