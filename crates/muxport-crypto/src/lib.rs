use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use hkdf::Hkdf;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
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
    #[error("Encrypted frame sequence was replayed or arrived out of order")]
    ReplayDetected,
    #[error("Encrypted frame sequence exhausted")]
    SequenceExhausted,
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

/// Ordered transport cipher that derives every nonce from a per-direction
/// prefix and a monotonic sequence number.
///
/// The two peers must use opposite send/receive prefixes. WebSocket delivery is
/// ordered, so rejecting old sequence numbers prevents replay and accidental
/// nonce reuse within a session.
pub struct SessionCipher {
    key: [u8; 32],
    send_nonce_prefix: [u8; 4],
    receive_nonce_prefix: [u8; 4],
    send_sequence: u64,
    highest_received_sequence: u64,
}

impl Drop for SessionCipher {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl SessionCipher {
    pub fn new(
        key: [u8; 32],
        send_nonce_prefix: [u8; 4],
        receive_nonce_prefix: [u8; 4],
    ) -> Self {
        Self {
            key,
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
        let ciphertext = encrypt_frame(&self.key, &nonce, plaintext, &bound_aad)?;
        self.send_sequence = sequence;
        Ok(EncryptedFrame {
            sequence,
            ciphertext,
        })
    }

    pub fn decrypt_next(
        &mut self,
        frame: &EncryptedFrame,
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        if frame.sequence <= self.highest_received_sequence {
            return Err(CryptoError::ReplayDetected);
        }

        let nonce = frame_nonce(self.receive_nonce_prefix, frame.sequence);
        let bound_aad = frame_aad(frame.sequence, aad);
        let plaintext = decrypt_frame(&self.key, &nonce, &frame.ciphertext, &bound_aad)?;
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

pub fn derive_shared_secret(our_secret: EphemeralSecret, their_public: &XPublicKey, info: &[u8]) -> Result<[u8; 32], CryptoError> {
    let dh_secret = our_secret.diffie_hellman(their_public);
    let hk = Hkdf::<Sha256>::new(None, dh_secret.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(info, &mut okm).map_err(|_| CryptoError::InvalidKeyLength)?;
    Ok(okm)
}

pub fn generate_sas_code(shared_secret: &[u8; 32]) -> String {
    let hk = Hkdf::<Sha256>::new(None, shared_secret);
    let mut sas_bytes = [0u8; 4];
    hk.expand(b"muxport-sas-v1", &mut sas_bytes).unwrap();
    let num = u32::from_be_bytes(sas_bytes) % 1_000_000;
    format!("{:06}", num)
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

    pub fn register_device(&mut self, record: PairedDeviceRecord) {
        self.devices.insert(record.device_id.clone(), record);
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
        self.devices.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecdh_key_exchange_and_aead() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();

        let alice_pub = alice.public;
        let bob_pub = bob.public;

        let alice_shared = derive_shared_secret(alice.secret, &bob_pub, b"muxport-test").unwrap();
        let bob_shared = derive_shared_secret(bob.secret, &alice_pub, b"muxport-test").unwrap();

        assert_eq!(alice_shared, bob_shared);

        let sas_a = generate_sas_code(&alice_shared);
        let sas_b = generate_sas_code(&bob_shared);
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
    fn session_cipher_uses_ordered_nonces_and_rejects_replay() {
        let key = [9u8; 32];
        let alice_prefix = [1, 2, 3, 4];
        let bob_prefix = [5, 6, 7, 8];
        let mut alice = SessionCipher::new(key, alice_prefix, bob_prefix);
        let mut bob = SessionCipher::new(key, bob_prefix, alice_prefix);

        let frame = alice.encrypt_next(b"first", b"route-a").unwrap();
        assert_eq!(frame.sequence, 1);
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
    }

    #[test]
    fn device_registry_pairing_and_revocation() {
        let mut registry = DeviceRegistry::new();
        let dev = PairedDeviceRecord {
            device_id: "phone-abc".into(),
            device_name: "User iPhone".into(),
            public_key_hex: "0102030405".into(),
            paired_at_ms: 1000,
            last_seen_at_ms: 1000,
            is_revoked: false,
        };
        registry.register_device(dev);

        assert!(registry.is_authorized("phone-abc", "0102030405"));
        assert!(!registry.is_authorized("phone-abc", "wrongpubkey"));
        assert!(!registry.is_authorized("unknown-phone", "0102030405"));

        assert!(registry.revoke_device("phone-abc"));
        assert!(!registry.is_authorized("phone-abc", "0102030405"));
    }
}
