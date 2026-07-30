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
    pub staged_payload_hex: Option<String>,
    pub staged_nonce_hex: Option<String>,
    pub previous_payload_hex: Option<String>,
    pub previous_nonce_hex: Option<String>,
}

pub struct PersistentVault {
    file_path: std::path::PathBuf,
    kek: KeyEncryptionKey,
    records: std::collections::HashMap<String, CredentialRecord>,
}

impl PersistentVault {
    pub fn open_or_create(
        path: impl AsRef<std::path::Path>,
        kek: KeyEncryptionKey,
    ) -> Result<Self, VaultError> {
        let file_path = path.as_ref().to_path_buf();
        let mut records = std::collections::HashMap::new();

        if file_path.exists() {
            let data = std::fs::read(&file_path).map_err(|_| VaultError::DecryptionFailed)?;
            if !data.is_empty() {
                let rec_list: Vec<CredentialRecord> =
                    serde_json::from_slice(&data).map_err(|_| VaultError::DecryptionFailed)?;
                for rec in rec_list {
                    records.insert(rec.profile_id.clone(), rec);
                }
            }
        }

        Ok(Self {
            file_path,
            kek,
            records,
        })
    }

    pub fn save(&self) -> Result<(), VaultError> {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let list: Vec<&CredentialRecord> = self.records.values().collect();
        let data = serde_json::to_vec_pretty(&list).map_err(|_| VaultError::EncryptionFailed)?;
        std::fs::write(&self.file_path, data).map_err(|_| VaultError::EncryptionFailed)?;
        Ok(())
    }

    pub fn store_record(&mut self, record: CredentialRecord) -> Result<(), VaultError> {
        self.records.insert(record.profile_id.clone(), record);
        self.save()
    }

    pub fn get_record(&self, profile_id: &str) -> Option<&CredentialRecord> {
        self.records.get(profile_id)
    }

    pub fn list_records(&self) -> Vec<CredentialRecord> {
        self.records.values().cloned().collect()
    }

    pub fn stage_credential(
        &mut self,
        profile_id: &str,
        plaintext: &[u8],
    ) -> Result<(), VaultError> {
        let (staged_hex, nonce_hex) = self.kek.encrypt_secret(plaintext, profile_id)?;
        if let Some(rec) = self.records.get_mut(profile_id) {
            rec.staged_payload_hex = Some(staged_hex);
            rec.staged_nonce_hex = Some(nonce_hex);
            rec.status = 1; // STAGED
        }
        self.save()
    }

    pub fn activate_credential(&mut self, profile_id: &str) -> Result<(), VaultError> {
        if let Some(rec) = self.records.get_mut(profile_id) {
            if let (Some(staged), Some(nonce)) = (rec.staged_payload_hex.take(), rec.staged_nonce_hex.take()) {
                rec.previous_payload_hex = Some(rec.encrypted_payload_hex.clone());
                rec.previous_nonce_hex = Some(rec.nonce_hex.clone());
                rec.encrypted_payload_hex = staged;
                rec.nonce_hex = nonce;
                rec.status = 2; // ACTIVE
                rec.last_validated_at_ms = chrono::Utc::now().timestamp_millis();
            }
        }
        self.save()
    }

    pub fn rollback_credential(&mut self, profile_id: &str) -> Result<(), VaultError> {
        if let Some(rec) = self.records.get_mut(profile_id) {
            if let (Some(prev), Some(nonce)) = (rec.previous_payload_hex.take(), rec.previous_nonce_hex.take()) {
                rec.encrypted_payload_hex = prev;
                rec.nonce_hex = nonce;
                rec.status = 2; // ACTIVE
            }
        }
        self.save()
    }

    pub fn decrypt_active_secret(&self, profile_id: &str) -> Result<SecretBuffer, VaultError> {
        let rec = self.records.get(profile_id).ok_or_else(|| VaultError::NotFound(profile_id.to_string()))?;
        self.kek.decrypt_secret(&rec.encrypted_payload_hex, &rec.nonce_hex, profile_id)
    }

    pub fn export_encrypted_backup(&self) -> Result<Vec<u8>, VaultError> {
        let list: Vec<&CredentialRecord> = self.records.values().collect();
        serde_json::to_vec(&list).map_err(|_| VaultError::EncryptionFailed)
    }
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

    #[test]
    fn persistent_vault_staging_activation_and_rollback() {
        let temp_dir = std::env::temp_dir();
        let vault_file = temp_dir.join(format!("test_vault_{}.sealed", std::process::id()));

        let salt = [9u8; 16];
        let kek = KeyEncryptionKey::derive_from_passphrase(b"passphrase123", &salt).unwrap();
        let (ct_hex, nonce_hex) = kek.encrypt_secret(b"secret-v1", "prof-1").unwrap();

        {
            let mut vault = PersistentVault::open_or_create(&vault_file, kek).unwrap();
            let rec = CredentialRecord {
                profile_id: "prof-1".into(),
                display_name: "Test Profile".into(),
                provider: "opencode_go".into(),
                credential_type: "api_key".into(),
                account_fingerprint: "fp123".into(),
                created_at_ms: 1000,
                last_validated_at_ms: 1000,
                status: 2, // ACTIVE
                encrypted_payload_hex: ct_hex,
                nonce_hex,
                staged_payload_hex: None,
                staged_nonce_hex: None,
                previous_payload_hex: None,
                previous_nonce_hex: None,
            };
            vault.store_record(rec).unwrap();
            assert_eq!(&vault.decrypt_active_secret("prof-1").unwrap().inner[..], b"secret-v1");

            // Stage secret-v2
            vault.stage_credential("prof-1", b"secret-v2").unwrap();
            // Active is still secret-v1
            assert_eq!(&vault.decrypt_active_secret("prof-1").unwrap().inner[..], b"secret-v1");

            // Activate secret-v2
            vault.activate_credential("prof-1").unwrap();
            assert_eq!(&vault.decrypt_active_secret("prof-1").unwrap().inner[..], b"secret-v2");

            // Rollback back to secret-v1
            vault.rollback_credential("prof-1").unwrap();
            assert_eq!(&vault.decrypt_active_secret("prof-1").unwrap().inner[..], b"secret-v1");
        }

        let _ = std::fs::remove_file(&vault_file);
    }
}
