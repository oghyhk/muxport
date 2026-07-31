# Credential provisioning

Provider credentials are not a normal Muxport command field. A phone can add a
credential only through a live, paired direct session to one selected host.
The phone first requires device authentication, then derives a dedicated
`muxport-secret-provisioning-v1` key from the authenticated X25519 handshake.

The provider secret is encrypted with ChaCha20-Poly1305 under that key before
it is added to `ProvisionCredentialCmd`. The inner associated data binds the
host ID, paired device ID, command ID, idempotency key, profile ID, label,
provider, credential type, and account fingerprint. The ordinary encrypted
transport envelope remains in place as a separate layer.

The connector accepts a provisioning command only through
`dispatch_authenticated`; untrusted or offline command paths cannot supply the
per-session provisioning key. It opens the inner envelope into a zeroizing
buffer and immediately enrolls the secret in the host vault. The command
ledger keeps only a SHA-256 fingerprint of the serialized command and a
redacted result; it never stores the secret plaintext.

The mobile field is registered with the sensitive-input registry, erased when
the app is backgrounded or closed, and cleared before network work begins. The
consumed byte buffer, inner ciphertext, nonce, and derived key are cleared on
completion. The app makes a best-effort clipboard clear after a provisioning
attempt. The provider key is never saved in the phone cache, diagnostics,
push payload, relay, or repository.

Provisioning only stores a host-local credential profile. It does not assign a
runtime, restart an agent, or activate a replacement credential. Assignment
continues to use the existing validation and transactional switch path.
