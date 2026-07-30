use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Vault is locked")]
    VaultLocked,
    #[error("Decryption failed: invalid key or corrupted envelope")]
    DecryptionFailed,
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Secret record not found: {0}")]
    NotFound(String),
    #[error("Argon2id derivation failed")]
    DerivationFailed,
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretBuffer {
    pub inner: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CredentialRecord {
    pub profile_id: String,
    pub display_name: String,
    pub provider: String,
    pub credential_type: String,
    pub account_fingerprint: String,
    pub created_at_ms: i64,
    pub last_validated_at_ms: i64,
    pub status: i32, // CredentialStatus enum value
    pub encrypted_payload_hex: String,
    pub nonce_hex: String,
}

pub struct KeyEncryptionKey {
    key: [u8; 32],
}

impl Drop for KeyEncryptionKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl KeyEncryptionKey {
    pub fn derive_from_passphrase(passphrase: &[u8], salt: &[u8; 16]) -> Result<Self, VaultError> {
        let mut key = [0u8; 32];
        let argon2 = Argon2::default();
        argon2
            .hash_password_into(passphrase, salt, &mut key)
            .map_err(|_| VaultError::DerivationFailed)?;
        Ok(Self { key })
    }

    pub fn encrypt_secret(&self, plaintext: &[u8], profile_id: &str) -> Result<(String, String), VaultError> {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);

        let cipher = ChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| VaultError::EncryptionFailed)?;
        let payload = Payload {
            msg: plaintext,
            aad: profile_id.as_bytes(),
        };

        let ciphertext = cipher.encrypt(&nonce.into(), payload).map_err(|_| VaultError::EncryptionFailed)?;
        Ok((hex::encode(ciphertext), hex::encode(nonce)))
    }

    pub fn decrypt_secret(&self, ciphertext_hex: &str, nonce_hex: &str, profile_id: &str) -> Result<SecretBuffer, VaultError> {
        let ciphertext = hex::decode(ciphertext_hex).map_err(|_| VaultError::DecryptionFailed)?;
        let nonce = hex::decode(nonce_hex).map_err(|_| VaultError::DecryptionFailed)?;

        if nonce.len() != 12 {
            return Err(VaultError::DecryptionFailed);
        }

        let cipher = ChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| VaultError::DecryptionFailed)?;
        let payload = Payload {
            msg: &ciphertext,
            aad: profile_id.as_bytes(),
        };

        let plaintext = cipher.decrypt(nonce[..].into(), payload).map_err(|_| VaultError::DecryptionFailed)?;
        Ok(SecretBuffer { inner: plaintext })
    }
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn decode(hex_str: &str) -> Result<Vec<u8>, ()> {
        if hex_str.len() % 2 != 0 {
            return Err(());
        }
        (0..hex_str.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).map_err(|_| ()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_derivation_and_vault_encryption() {
        let passphrase = b"super_secret_master_password";
        let salt = [42u8; 16];

        let kek = KeyEncryptionKey::derive_from_passphrase(passphrase, &salt).unwrap();
        let secret = b"sk-opencode-go-api-key-test-12345";
        let profile_id = "opencode-go-profile-1";

        let (ciphertext_hex, nonce_hex) = kek.encrypt_secret(secret, profile_id).unwrap();
        let decrypted = kek.decrypt_secret(&ciphertext_hex, &nonce_hex, profile_id).unwrap();

        assert_eq!(&decrypted.inner[..], secret);
    }

    #[test]
    fn vault_rejects_wrong_context_key_and_tampered_ciphertext() {
        let salt = [7u8; 16];
        let key = KeyEncryptionKey::derive_from_passphrase(b"correct passphrase", &salt).unwrap();
        let wrong_key =
            KeyEncryptionKey::derive_from_passphrase(b"different passphrase", &salt).unwrap();
        let (ciphertext_hex, nonce_hex) = key
            .encrypt_secret(b"TEST_CREDENTIAL_PLACEHOLDER", "profile-a")
            .unwrap();

        assert!(matches!(
            key.decrypt_secret(&ciphertext_hex, &nonce_hex, "profile-b"),
            Err(VaultError::DecryptionFailed)
        ));
        assert!(matches!(
            wrong_key.decrypt_secret(&ciphertext_hex, &nonce_hex, "profile-a"),
            Err(VaultError::DecryptionFailed)
        ));

        let mut tampered = hex::decode(&ciphertext_hex).unwrap();
        tampered[0] ^= 1;
        assert!(matches!(
            key.decrypt_secret(&hex::encode(tampered), &nonce_hex, "profile-a"),
            Err(VaultError::DecryptionFailed)
        ));
    }
}
