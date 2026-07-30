# Signed pairing offer

- **Status:** Rust creation and Dart verification implemented; live flow pending
- **Reviewed:** 2026-07-30

The QR bootstrap payload is a self-signed JSON object containing:

- protocol version, host ID, and display hostname;
- the host's long-term Ed25519 public identity;
- a concrete direct endpoint;
- a random 256-bit, single-use rendezvous token;
- the host's ephemeral X25519 public key;
- issue and expiry timestamps; and
- an Ed25519 signature over every field.

The connector pairing store can create this payload only for a concrete,
non-unspecified IP socket address. It persists only a hash of the rendezvous
token. Offers live for at most ten minutes and the durable store enforces
single-use claim and expiry independently of the phone.

The Flutter verifier rejects unknown fields, non-canonical key/token encoding,
invalid endpoints, excessive validity windows, expired offers, and signature
tampering. A successfully verified host key is still only a candidate pin.
The app must not persist it as trusted until the phone and host display and
confirm the same transcript-derived six-digit SAS.

The self-signature detects corruption and binds the endpoint, but it cannot by
itself defeat replacement of the entire first-trust QR. Physical QR provenance
and the out-of-band SAS comparison remain required.

## Missing live path

The connector does not yet expose offer creation, pairing claim, or
confirmation over its listener. The app does not yet scan or render the QR/SAS
screens. The in-memory host ephemeral secret, claim protocol, dual
confirmation, registry refresh, cancellation, and rate limiting must be wired
as one lifecycle before pairing is usable.
