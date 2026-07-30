# Authenticated direct command transport

The connector has an opt-in TCP endpoint for paired-device command traffic.
It is disabled unless `MUXPORT_DIRECT_BIND` is set to an IP socket address.
Loopback addresses are accepted directly; a non-loopback address additionally
requires `MUXPORT_ALLOW_REMOTE_DIRECT=1`. This second flag is intentional: the
endpoint should normally be reached through a private LAN, VPN, or SSH tunnel,
not exposed indiscriminately to the public Internet.

## Handshake

1. The host sends a fresh random 256-bit challenge, host ID, public identity
   key, connector boot epoch, issue/expiry times, and an Ed25519 signature over
   all those fields.
2. The phone verifies that signature against its pinned host key and returns
   the existing signed initiator hello with a fresh X25519 key.
3. The connector verifies the exact non-revoked device ID/public-key binding in
   its host-signed registry.
4. The host returns its signed responder hello. Both sides derive directional
   ChaCha20-Poly1305 keys and nonce prefixes from the authenticated transcript.
5. Every later TCP record contains one ordered encrypted Muxport frame.

The handshake and encrypted records are length-prefixed and bounded. Handshake
I/O times out after 15 seconds, zero-length records are rejected, no more than
32 sessions are admitted at once, and unregistered or revoked devices are
closed before an encrypted session exists. No provider credential is sent by
this layer.

## Command boundary

Only a command released by `SecureEnvelopeSession` reaches `CommandRouter`.
The router durably reserves its idempotency key before adapter side effects,
returns stored results to duplicates, and converts a crash-interrupted
operation to reconciliation-required rather than replaying it. Results travel
back through the same authenticated encrypted session.

## Current limitations

- The Flutter client does not yet implement this wire protocol.
- Pairing offers and SAS confirmation are not exposed through a physical UI.
- A registry snapshot is loaded when the connector starts; online revocation
  propagation is not yet implemented.
- The endpoint currently carries commands/results only. Snapshot negotiation,
  journal replay, acknowledgements, and live event fan-out remain pending.
- There is a connection cap but no per-source rate limiter. Non-loopback use
  still requires a firewall/private-network policy and security review.
- Relay transport and network fuzz/chaos testing remain pending.

These limitations keep the connector in `degraded` state even when the direct
command listener is enabled.
