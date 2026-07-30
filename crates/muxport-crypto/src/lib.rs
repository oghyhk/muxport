mod handshake;

pub use handshake::*;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use hkdf::Hkdf;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use x25519_dalek::{EphemeralSecret, PublicKey as XPublicKey};
use zeroize::Zeroize;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failure")]
    EncryptionFailed,
    #[error("Decryption failure or bad tag")]
    DecryptionFailed,
    #[error("Invalid key length")]
    InvalidKeyLength,
    #[error("Pairing token expired or invalid")]
    InvalidPairingToken,
    #[error("Signed pairing offer is invalid")]
    InvalidPairingOffer,
    #[error("Signed pairing offer has expired")]
    ExpiredPairingOffer,
    #[error("Encrypted frame sequence was replayed or arrived out of order")]
    ReplayDetected,
    #[error("Encrypted frame sequence exhausted")]
    SequenceExhausted,
    #[error("Encrypted frame encoding is invalid")]
    InvalidFrameEncoding,
    #[error("Encrypted frame exceeds the configured size limit")]
    FrameTooLarge,
    #[error("Paired device id must not be empty")]
    InvalidDeviceId,
    #[error("Paired device public key is not a valid Ed25519 key")]
    InvalidDevicePublicKey,
    #[error("Paired device id is already bound to a different or revoked identity")]
    DeviceIdentityConflict,
    #[error("Key exchange used a non-contributory public key")]
    NonContributoryKey,
    #[error("Handshake protocol or role is invalid")]
    InvalidHandshake,
    #[error("Handshake host, device, or challenge does not match")]
    HandshakeContextMismatch,
    #[error("Handshake identity signature is invalid")]
    InvalidHandshakeSignature,
    #[error("Handshake field is not valid hexadecimal data")]
    InvalidHandshakeEncoding,
    #[error("Device registry serialization failed")]
    RegistrySerialization,
    #[error("Device registry signature or signer is invalid")]
    InvalidRegistrySignature,
    #[error("Device registry host or version does not match")]
    RegistryContextMismatch,
    #[error("Device registry contains invalid or duplicate records")]
    InvalidRegistryRecords,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QrPairingPayload {
    pub host_id: String,
    pub hostname: String,
    pub rendezvous_token: String,
    pub ephemeral_pubkey_hex: String,
    pub expires_at_ms: i64,
}

pub struct KeyPair {
    pub secret: EphemeralSecret,
    pub public: XPublicKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EncryptedFrame {
    pub sequence: u64,
    pub ciphertext: Vec<u8>,
}

impl EncryptedFrame {
    pub fn encode_wire(&self) -> Result<Vec<u8>, CryptoError> {
        let ciphertext_len: u32 = self
            .ciphertext
            .len()
            .try_into()
            .map_err(|_| CryptoError::FrameTooLarge)?;
        let mut encoded = Vec::with_capacity(16 + self.ciphertext.len());
        encoded.extend_from_slice(b"MUX1");
        encoded.extend_from_slice(&self.sequence.to_be_bytes());
        encoded.extend_from_slice(&ciphertext_len.to_be_bytes());
        encoded.extend_from_slice(&self.ciphertext);
        Ok(encoded)
    }

    pub fn decode_wire(
        encoded: &[u8],
        max_ciphertext_bytes: usize,
    ) -> Result<Self, CryptoError> {
        if encoded.len() < 16 || &encoded[..4] != b"MUX1" {
            return Err(CryptoError::InvalidFrameEncoding);
        }
        let sequence = u64::from_be_bytes(
            encoded[4..12]
                .try_into()
                .map_err(|_| CryptoError::InvalidFrameEncoding)?,
        );
        if sequence == 0 {
            return Err(CryptoError::InvalidFrameEncoding);
        }
        let ciphertext_len = u32::from_be_bytes(
            encoded[12..16]
                .try_into()
                .map_err(|_| CryptoError::InvalidFrameEncoding)?,
        ) as usize;
        if ciphertext_len > max_ciphertext_bytes {
            return Err(CryptoError::FrameTooLarge);
        }
        if encoded.len() != 16 + ciphertext_len {
            return Err(CryptoError::InvalidFrameEncoding);
        }
        Ok(Self {
            sequence,
            ciphertext: encoded[16..].to_vec(),
        })
    }
}

/// Ordered transport cipher that derives every nonce from a per-direction
/// prefix and a monotonic sequence number.
///
/// The two peers must use opposite send/receive prefixes. WebSocket delivery is
/// ordered, so rejecting old sequence numbers prevents replay and accidental
/// nonce reuse within a session.
pub struct SessionCipher {
    send_key: [u8; 32],
    receive_key: [u8; 32],
    send_nonce_prefix: [u8; 4],
    receive_nonce_prefix: [u8; 4],
    send_sequence: u64,
    highest_received_sequence: u64,
}

impl Drop for SessionCipher {
    fn drop(&mut self) {
        self.send_key.zeroize();
        self.receive_key.zeroize();
    }
}

impl SessionCipher {
    pub fn new(
        send_key: [u8; 32],
        receive_key: [u8; 32],
        send_nonce_prefix: [u8; 4],
        receive_nonce_prefix: [u8; 4],
    ) -> Self {
        Self {
            send_key,
            receive_key,
            send_nonce_prefix,
            receive_nonce_prefix,
            send_sequence: 0,
            highest_received_sequence: 0,
        }
    }

    pub fn encrypt_next(
        &mut self,
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<EncryptedFrame, CryptoError> {
        let sequence = self
            .send_sequence
            .checked_add(1)
            .ok_or(CryptoError::SequenceExhausted)?;
        let nonce = frame_nonce(self.send_nonce_prefix, sequence);
        let bound_aad = frame_aad(sequence, aad);
        let ciphertext = encrypt_frame(&self.send_key, &nonce, plaintext, &bound_aad)?;
        self.send_sequence = sequence;
        Ok(EncryptedFrame {
            sequence,
            ciphertext,
        })
    }

    pub fn next_send_sequence(&self) -> Result<u64, CryptoError> {
        self.send_sequence
            .checked_add(1)
            .ok_or(CryptoError::SequenceExhausted)
    }

    pub fn decrypt_next(
        &mut self,
        frame: &EncryptedFrame,
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let expected = self
            .highest_received_sequence
            .checked_add(1)
            .ok_or(CryptoError::SequenceExhausted)?;
        if frame.sequence != expected {
            return Err(CryptoError::ReplayDetected);
        }

        let nonce = frame_nonce(self.receive_nonce_prefix, frame.sequence);
        let bound_aad = frame_aad(frame.sequence, aad);
        let plaintext =
            decrypt_frame(&self.receive_key, &nonce, &frame.ciphertext, &bound_aad)?;
        self.highest_received_sequence = frame.sequence;
        Ok(plaintext)
    }
}

fn frame_nonce(prefix: [u8; 4], sequence: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..4].copy_from_slice(&prefix);
    nonce[4..].copy_from_slice(&sequence.to_be_bytes());
    nonce
}

fn frame_aad(sequence: u64, aad: &[u8]) -> Vec<u8> {
    let mut bound = Vec::with_capacity(8 + aad.len());
    bound.extend_from_slice(&sequence.to_be_bytes());
    bound.extend_from_slice(aad);
    bound
}

impl KeyPair {
    pub fn generate() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = XPublicKey::from(&secret);
        Self { secret, public }
    }
}

/// Derives a pairing/session secret bound to the complete authenticated
/// handshake transcript.
pub fn derive_shared_secret(
    our_secret: EphemeralSecret,
    their_public: &XPublicKey,
    transcript: &[u8],
) -> Result<[u8; 32], CryptoError> {
    let dh_secret = our_secret.diffie_hellman(their_public);
    if dh_secret.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(CryptoError::NonContributoryKey);
    }
    let transcript_hash = Sha256::digest(transcript);
    let hk = Hkdf::<Sha256>::new(Some(&transcript_hash), dh_secret.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"muxport-shared-secret-v1", &mut okm)
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    Ok(okm)
}

pub fn generate_sas_code(
    shared_secret: &[u8; 32],
    transcript: &[u8],
) -> Result<String, CryptoError> {
    let transcript_hash = Sha256::digest(transcript);
    let hk = Hkdf::<Sha256>::new(Some(&transcript_hash), shared_secret);
    let mut sas_bytes = [0u8; 4];
    hk.expand(b"muxport-sas-v1", &mut sas_bytes)
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    let num = u32::from_be_bytes(sas_bytes) % 1_000_000;
    Ok(format!("{:06}", num))
}

pub struct DirectionalSessionKeys {
    pub initiator_to_responder_key: [u8; 32],
    pub responder_to_initiator_key: [u8; 32],
    pub initiator_nonce_prefix: [u8; 4],
    pub responder_nonce_prefix: [u8; 4],
}

impl Drop for DirectionalSessionKeys {
    fn drop(&mut self) {
        self.initiator_to_responder_key.zeroize();
        self.responder_to_initiator_key.zeroize();
        self.initiator_nonce_prefix.zeroize();
        self.responder_nonce_prefix.zeroize();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionRole {
    Initiator,
    Responder,
}

pub fn derive_session_keys(
    shared_secret: &[u8; 32],
    transcript: &[u8],
) -> Result<DirectionalSessionKeys, CryptoError> {
    let transcript_hash = Sha256::digest(transcript);
    let hk = Hkdf::<Sha256>::new(Some(&transcript_hash), shared_secret);
    let mut material = [0_u8; 72];
    hk.expand(b"muxport-directional-session-v1", &mut material)
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    let mut initiator_to_responder_key = [0_u8; 32];
    initiator_to_responder_key.copy_from_slice(&material[..32]);
    let mut responder_to_initiator_key = [0_u8; 32];
    responder_to_initiator_key.copy_from_slice(&material[32..64]);
    let mut initiator_nonce_prefix = [0_u8; 4];
    initiator_nonce_prefix.copy_from_slice(&material[64..68]);
    let mut responder_nonce_prefix = [0_u8; 4];
    responder_nonce_prefix.copy_from_slice(&material[68..72]);
    material.zeroize();
    Ok(DirectionalSessionKeys {
        initiator_to_responder_key,
        responder_to_initiator_key,
        initiator_nonce_prefix,
        responder_nonce_prefix,
    })
}

impl SessionCipher {
    pub fn from_directional_keys(
        keys: &DirectionalSessionKeys,
        role: SessionRole,
    ) -> Self {
        match role {
            SessionRole::Initiator => Self::new(
                keys.initiator_to_responder_key,
                keys.responder_to_initiator_key,
                keys.initiator_nonce_prefix,
                keys.responder_nonce_prefix,
            ),
            SessionRole::Responder => Self::new(
                keys.responder_to_initiator_key,
                keys.initiator_to_responder_key,
                keys.responder_nonce_prefix,
                keys.initiator_nonce_prefix,
            ),
        }
    }
}

pub fn encrypt_frame(key: &[u8; 32], nonce_bytes: &[u8; 12], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?;
    let payload = Payload {
        msg: plaintext,
        aad,
    };
    cipher.encrypt(nonce_bytes.into(), payload).map_err(|_| CryptoError::EncryptionFailed)
}

pub fn decrypt_frame(key: &[u8; 32], nonce_bytes: &[u8; 12], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?;
    let payload = Payload {
        msg: ciphertext,
        aad,
    };
    cipher.decrypt(nonce_bytes.into(), payload).map_err(|_| CryptoError::DecryptionFailed)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PairedDeviceRecord {
    pub device_id: String,
    pub device_name: String,
    pub public_key_hex: String,
    pub paired_at_ms: i64,
    pub last_seen_at_ms: i64,
    pub is_revoked: bool,
}

pub struct DeviceRegistry {
    devices: std::collections::HashMap<String, PairedDeviceRecord>,
}

#[derive(Serialize, Deserialize)]
struct DeviceRegistryPayload {
    version: u32,
    host_id: String,
    devices: Vec<PairedDeviceRecord>,
}

#[derive(Serialize, Deserialize)]
struct SignedDeviceRegistry {
    payload: DeviceRegistryPayload,
    host_identity_public_key_hex: String,
    signature_hex: String,
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: std::collections::HashMap::new(),
        }
    }

    pub fn register_device(&mut self, record: PairedDeviceRecord) -> Result<(), CryptoError> {
        if record.device_id.trim().is_empty() {
            return Err(CryptoError::InvalidDeviceId);
        }
        validate_device_public_key(&record.public_key_hex)?;

        if let Some(existing) = self.devices.get_mut(&record.device_id) {
            if existing.public_key_hex != record.public_key_hex
                || (existing.is_revoked && !record.is_revoked)
            {
                return Err(CryptoError::DeviceIdentityConflict);
            }
            existing.device_name = record.device_name;
            existing.last_seen_at_ms = existing.last_seen_at_ms.max(record.last_seen_at_ms);
            existing.is_revoked |= record.is_revoked;
            return Ok(());
        }

        self.devices.insert(record.device_id.clone(), record);
        Ok(())
    }

    pub fn revoke_device(&mut self, device_id: &str) -> bool {
        if let Some(dev) = self.devices.get_mut(device_id) {
            dev.is_revoked = true;
            true
        } else {
            false
        }
    }

    pub fn is_authorized(&self, device_id: &str, pubkey_hex: &str) -> bool {
        if let Some(dev) = self.devices.get(device_id) {
            !dev.is_revoked && dev.public_key_hex == pubkey_hex
        } else {
            false
        }
    }

    pub fn list_devices(&self) -> Vec<PairedDeviceRecord> {
        let mut devices = self.devices.values().cloned().collect::<Vec<_>>();
        devices.sort_by(|left, right| left.device_id.cmp(&right.device_id));
        devices
    }

    /// Serializes the non-secret registry with a host-identity signature.
    /// Callers must persist the returned bytes atomically.
    pub fn signed_snapshot(
        &self,
        host_id: &str,
        host_identity: &ed25519_dalek::SigningKey,
    ) -> Result<Vec<u8>, CryptoError> {
        use ed25519_dalek::Signer;

        if host_id.trim().is_empty() {
            return Err(CryptoError::RegistryContextMismatch);
        }
        let payload = DeviceRegistryPayload {
            version: 1,
            host_id: host_id.to_owned(),
            devices: self.list_devices(),
        };
        let payload_bytes = registry_payload_bytes(&payload)?;
        let signature = host_identity.sign(&payload_bytes);
        let envelope = SignedDeviceRegistry {
            payload,
            host_identity_public_key_hex: registry_encode_hex(
                &host_identity.verifying_key().to_bytes(),
            ),
            signature_hex: registry_encode_hex(&signature.to_bytes()),
        };
        serde_json::to_vec(&envelope)
            .map_err(|_| CryptoError::RegistrySerialization)
    }

    pub fn from_signed_snapshot(
        bytes: &[u8],
        expected_host_id: &str,
        expected_host_identity: &ed25519_dalek::VerifyingKey,
    ) -> Result<Self, CryptoError> {
        use ed25519_dalek::Verifier;

        let envelope: SignedDeviceRegistry = serde_json::from_slice(bytes)
            .map_err(|_| CryptoError::RegistrySerialization)?;
        if envelope.payload.version != 1
            || envelope.payload.host_id != expected_host_id
            || envelope.host_identity_public_key_hex
                != registry_encode_hex(&expected_host_identity.to_bytes())
        {
            return Err(CryptoError::RegistryContextMismatch);
        }
        let signature_bytes = registry_parse_hex::<64>(&envelope.signature_hex)
            .map_err(|_| CryptoError::InvalidRegistrySignature)?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);
        expected_host_identity
            .verify(
                &registry_payload_bytes(&envelope.payload)?,
                &signature,
            )
            .map_err(|_| CryptoError::InvalidRegistrySignature)?;

        let mut registry = Self::new();
        for record in envelope.payload.devices {
            if registry.devices.contains_key(&record.device_id) {
                return Err(CryptoError::InvalidRegistryRecords);
            }
            registry
                .register_device(record)
                .map_err(|_| CryptoError::InvalidRegistryRecords)?;
        }
        Ok(registry)
    }
}

fn registry_payload_bytes(
    payload: &DeviceRegistryPayload,
) -> Result<Vec<u8>, CryptoError> {
    let serialized =
        serde_json::to_vec(payload).map_err(|_| CryptoError::RegistrySerialization)?;
    let mut signed = b"muxport-device-registry-v1".to_vec();
    let length: u32 = serialized
        .len()
        .try_into()
        .map_err(|_| CryptoError::RegistrySerialization)?;
    signed.extend_from_slice(&length.to_be_bytes());
    signed.extend_from_slice(&serialized);
    Ok(signed)
}

fn registry_encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn registry_parse_hex<const N: usize>(
    value: &str,
) -> Result<[u8; N], CryptoError> {
    if value.len() != N * 2 {
        return Err(CryptoError::InvalidRegistrySignature);
    }
    let mut bytes = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(chunk)
            .map_err(|_| CryptoError::InvalidRegistrySignature)?;
        bytes[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| CryptoError::InvalidRegistrySignature)?;
    }
    Ok(bytes)
}

fn validate_device_public_key(public_key_hex: &str) -> Result<(), CryptoError> {
    if public_key_hex.len() != 64 {
        return Err(CryptoError::InvalidDevicePublicKey);
    }
    let mut bytes = [0u8; 32];
    for (index, chunk) in public_key_hex.as_bytes().chunks_exact(2).enumerate() {
        let pair =
            std::str::from_utf8(chunk).map_err(|_| CryptoError::InvalidDevicePublicKey)?;
        bytes[index] =
            u8::from_str_radix(pair, 16).map_err(|_| CryptoError::InvalidDevicePublicKey)?;
    }
    ed25519_dalek::VerifyingKey::from_bytes(&bytes)
        .map(|_| ())
        .map_err(|_| CryptoError::InvalidDevicePublicKey)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxport_protocol::{
        command, muxport_envelope, Command, EnvelopeHeader, MuxportEnvelope,
        ProbeHostCmd,
    };
    use prost::Message;

    #[test]
    fn test_ecdh_key_exchange_and_aead() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();

        let alice_pub = alice.public;
        let bob_pub = bob.public;

        let transcript = b"muxport-test-transcript";
        let alice_shared =
            derive_shared_secret(alice.secret, &bob_pub, transcript).unwrap();
        let bob_shared =
            derive_shared_secret(bob.secret, &alice_pub, transcript).unwrap();

        assert_eq!(alice_shared, bob_shared);

        let sas_a = generate_sas_code(&alice_shared, transcript).unwrap();
        let sas_b = generate_sas_code(&bob_shared, transcript).unwrap();
        assert_eq!(sas_a, sas_b);
        assert_eq!(sas_a.len(), 6);

        let nonce = [7u8; 12];
        let msg = b"hello encrypted world";
        let aad = b"header_aad_123";

        let ciphertext = encrypt_frame(&alice_shared, &nonce, msg, aad).unwrap();
        let decrypted = decrypt_frame(&bob_shared, &nonce, &ciphertext, aad).unwrap();

        assert_eq!(msg.to_vec(), decrypted);
    }

    #[test]
    fn key_exchange_rejects_non_contributory_public_key() {
        let key_pair = KeyPair::generate();
        let zero_public = XPublicKey::from([0_u8; 32]);
        assert!(matches!(
            derive_shared_secret(
                key_pair.secret,
                &zero_public,
                b"authenticated transcript"
            ),
            Err(CryptoError::NonContributoryKey)
        ));
    }

    #[test]
    fn session_cipher_uses_ordered_nonces_and_rejects_replay() {
        let alice_key = [9u8; 32];
        let bob_key = [10u8; 32];
        let alice_prefix = [1, 2, 3, 4];
        let bob_prefix = [5, 6, 7, 8];
        let mut alice =
            SessionCipher::new(alice_key, bob_key, alice_prefix, bob_prefix);
        let mut bob =
            SessionCipher::new(bob_key, alice_key, bob_prefix, alice_prefix);

        let frame = alice.encrypt_next(b"first", b"route-a").unwrap();
        assert_eq!(frame.sequence, 1);
        let wire = frame.encode_wire().unwrap();
        assert_eq!(
            EncryptedFrame::decode_wire(&wire, 1024).unwrap(),
            frame
        );
        assert!(matches!(
            EncryptedFrame::decode_wire(&wire, 1),
            Err(CryptoError::FrameTooLarge)
        ));
        assert_eq!(
            bob.decrypt_next(&frame, b"route-a").unwrap(),
            b"first".to_vec()
        );
        assert!(matches!(
            bob.decrypt_next(&frame, b"route-a"),
            Err(CryptoError::ReplayDetected)
        ));

        let second = alice.encrypt_next(b"second", b"route-a").unwrap();
        assert_eq!(second.sequence, 2);
        assert!(matches!(
            bob.decrypt_next(&second, b"wrong-route"),
            Err(CryptoError::DecryptionFailed)
        ));
        assert_eq!(
            bob.decrypt_next(&second, b"route-a").unwrap(),
            b"second".to_vec()
        );

        let third = alice.encrypt_next(b"third", b"route-a").unwrap();
        let fourth = alice.encrypt_next(b"fourth", b"route-a").unwrap();
        assert!(matches!(
            bob.decrypt_next(&fourth, b"route-a"),
            Err(CryptoError::ReplayDetected)
        ));
        assert_eq!(
            bob.decrypt_next(&third, b"route-a").unwrap(),
            b"third".to_vec()
        );
        assert_eq!(
            bob.decrypt_next(&fourth, b"route-a").unwrap(),
            b"fourth".to_vec()
        );
    }

    #[test]
    fn device_registry_pairing_and_revocation() {
        let signing_key = ed25519_dalek::SigningKey::generate(&mut OsRng);
        let public_key_hex = signing_key
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut registry = DeviceRegistry::new();
        let dev = PairedDeviceRecord {
            device_id: "phone-abc".into(),
            device_name: "User iPhone".into(),
            public_key_hex: public_key_hex.clone(),
            paired_at_ms: 1000,
            last_seen_at_ms: 1000,
            is_revoked: false,
        };
        registry.register_device(dev).unwrap();

        assert!(registry.is_authorized("phone-abc", &public_key_hex));
        assert!(!registry.is_authorized("phone-abc", "wrongpubkey"));
        assert!(!registry.is_authorized("unknown-phone", &public_key_hex));

        assert!(registry.revoke_device("phone-abc"));
        assert!(!registry.is_authorized("phone-abc", &public_key_hex));

        let replacement_key = ed25519_dalek::SigningKey::generate(&mut OsRng)
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(matches!(
            registry.register_device(PairedDeviceRecord {
                device_id: "phone-abc".into(),
                device_name: "Stolen identity".into(),
                public_key_hex: replacement_key,
                paired_at_ms: 2000,
                last_seen_at_ms: 2000,
                is_revoked: false,
            }),
            Err(CryptoError::DeviceIdentityConflict)
        ));
    }

    #[test]
    fn rust_protocol_matches_committed_cross_language_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../protocol/fixtures/direct_session_v1.json"
        ))
        .unwrap();
        assert_eq!(fixture["schemaVersion"].as_u64(), Some(1));
        let host_id = fixture["hostId"].as_str().unwrap();
        let device_id = fixture["deviceId"].as_str().unwrap();
        let shared_secret: [u8; 32] = fixture_hex(
            fixture["sharedSecretHex"].as_str().unwrap(),
        )
        .try_into()
        .unwrap();
        let transcript =
            fixture_hex(fixture["transcriptHex"].as_str().unwrap());
        let keys = derive_session_keys(&shared_secret, &transcript).unwrap();
        assert_eq!(
            fixture_hex(
                fixture["initiatorToResponderKeyHex"]
                    .as_str()
                    .unwrap()
            ),
            keys.initiator_to_responder_key
        );
        assert_eq!(
            fixture_hex(
                fixture["responderToInitiatorKeyHex"]
                    .as_str()
                    .unwrap()
            ),
            keys.responder_to_initiator_key
        );
        assert_eq!(
            fixture_hex(
                fixture["initiatorNoncePrefixHex"].as_str().unwrap()
            ),
            keys.initiator_nonce_prefix
        );
        assert_eq!(
            fixture_hex(
                fixture["responderNoncePrefixHex"].as_str().unwrap()
            ),
            keys.responder_nonce_prefix
        );

        let mut aad = b"muxport-secure-envelope-v1".to_vec();
        append_fixture_aad(&mut aad, host_id.as_bytes());
        append_fixture_aad(&mut aad, device_id.as_bytes());
        append_fixture_aad(&mut aad, &Sha256::digest(&transcript));
        assert_eq!(
            fixture_hex(fixture["sessionAadHex"].as_str().unwrap()),
            aad
        );

        let envelope = MuxportEnvelope {
            header: Some(EnvelopeHeader {
                protocol_version: 1,
                sender_id: device_id.to_owned(),
                recipient_id: host_id.to_owned(),
                boot_epoch: 72_623_859_790_382_856,
                sequence: 1,
                timestamp_ms: 1_700_000_000_000,
                idempotency_key: "fixture-idempotency-1".into(),
            }),
            payload: Some(muxport_envelope::Payload::Command(Command {
                command_id: "fixture-probe-1".into(),
                deadline_ms: 1_700_000_060_000,
                inner: Some(command::Inner::ProbeHost(ProbeHostCmd {})),
            })),
        };
        let plaintext = envelope.encode_to_vec();
        assert_eq!(
            fixture_hex(fixture["envelopeHex"].as_str().unwrap()),
            plaintext
        );

        let mut initiator = SessionCipher::from_directional_keys(
            &keys,
            SessionRole::Initiator,
        );
        let mut responder = SessionCipher::from_directional_keys(
            &keys,
            SessionRole::Responder,
        );
        let frame = initiator.encrypt_next(&plaintext, &aad).unwrap();
        assert_eq!(
            fixture_hex(fixture["encryptedFrameHex"].as_str().unwrap()),
            frame.encode_wire().unwrap()
        );
        assert_eq!(
            responder.decrypt_next(&frame, &aad).unwrap(),
            plaintext
        );
    }

    #[test]
    fn device_registry_rejects_malformed_keys() {
        let mut registry = DeviceRegistry::new();
        assert!(matches!(
            registry.register_device(PairedDeviceRecord {
                device_id: "phone-invalid".into(),
                device_name: "Invalid".into(),
                public_key_hex: "not-a-key".into(),
                paired_at_ms: 1000,
                last_seen_at_ms: 1000,
                is_revoked: false,
            }),
            Err(CryptoError::InvalidDevicePublicKey)
        ));
    }

    #[test]
    fn signed_device_registry_detects_tampering_and_wrong_host() {
        let host_identity = ed25519_dalek::SigningKey::generate(&mut OsRng);
        let device_identity =
            ed25519_dalek::SigningKey::generate(&mut OsRng);
        let public_key_hex = device_identity
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut registry = DeviceRegistry::new();
        registry
            .register_device(PairedDeviceRecord {
                device_id: "phone-1".into(),
                device_name: "Phone".into(),
                public_key_hex,
                paired_at_ms: 1,
                last_seen_at_ms: 2,
                is_revoked: false,
            })
            .unwrap();
        let signed = registry
            .signed_snapshot("host-1", &host_identity)
            .unwrap();
        let restored = DeviceRegistry::from_signed_snapshot(
            &signed,
            "host-1",
            &host_identity.verifying_key(),
        )
        .unwrap();
        assert_eq!(restored.list_devices().len(), 1);
        assert!(matches!(
            DeviceRegistry::from_signed_snapshot(
                &signed,
                "host-2",
                &host_identity.verifying_key()
            ),
            Err(CryptoError::RegistryContextMismatch)
        ));

        let mut tampered: serde_json::Value =
            serde_json::from_slice(&signed).unwrap();
        tampered["payload"]["devices"][0]["device_name"] =
            serde_json::Value::String("Attacker".into());
        let tampered = serde_json::to_vec(&tampered).unwrap();
        assert!(matches!(
            DeviceRegistry::from_signed_snapshot(
                &tampered,
                "host-1",
                &host_identity.verifying_key()
            ),
            Err(CryptoError::InvalidRegistrySignature)
        ));
    }

    fn fixture_hex(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
                    .unwrap()
            })
            .collect()
    }

    fn append_fixture_aad(output: &mut Vec<u8>, value: &[u8]) {
        output.extend_from_slice(&(value.len() as u64).to_be_bytes());
        output.extend_from_slice(value);
    }
}
