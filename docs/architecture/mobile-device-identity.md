# Mobile device identity storage

Each phone has a versioned Ed25519 identity separate from reconstructible host
cache data and provider credentials. The device ID is the SHA-256 digest of the
public key. Only the public key and device ID may enter pairing or cache
metadata.

The 32-byte private seed is serialized only through `flutter_secure_storage`:

- Android uses a dedicated storage namespace, RSA-OAEP key wrapping, and
  AES-GCM storage. Automatic destructive reset is disabled, migration backup is
  enabled, and Android application backup is disabled.
- iOS uses a non-synchronizing, this-device-only Keychain item available after
  the first unlock. Secure Enclave wrapping is requested when the device
  supports it, with the plugin's documented Keychain fallback.

The identity is read back and compared after first creation. A locked native
store, malformed record, wrong seed length, or failed read-back disables the
identity. Muxport does not write a plaintext fallback or silently replace the
key, because replacement would impersonate a new phone and invalidate host
pins. Recovery must be explicit and requires re-pairing.

Transport identity access is not gated by a biometric prompt on every read so
that authenticated reconnect and event replay can resume after background
wake. High-impact commands and credential changes require a separate biometric
step-up policy; possession of the transport key alone is not sufficient
authorization for those operations.

Private seed buffers created during decoding and enrollment are overwritten
after the cryptographic key object is constructed. Dart strings returned by the
secure-storage plugin cannot be zeroized, so they are never logged, cached, or
returned through the public API.

Unit tests use an injected secure-store boundary to prove restart stability,
signature verification, serialized concurrent enrollment, read-back failure,
locked-store behavior, and refusal to replace corrupt records. Native
Keychain/Keystore and physical-device tests remain required.
