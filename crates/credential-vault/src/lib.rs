mod host_identity;

pub use host_identity::{
    HostIdentityError, HostIdentityManager, HostIdentitySecretStore,
    LoadedHostIdentity, OsHostIdentityStore,
};

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use muxport_protocol::CredentialStatus;
use rand::rngs::OsRng;
use rand::{RngCore, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const VAULT_FORMAT_VERSION: u32 = 1;
const VAULT_ENVELOPE_AAD: &[u8] = b"muxport-vault-envelope-v1";

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
    #[error("Vault I/O failed: {0}")]
    Io(String),
    #[error("Vault serialization failed")]
    Serialization,
    #[error("Unsupported vault format version: {0}")]
    UnsupportedVersion(u32),
    #[error("Invalid credential operation: {0}")]
    InvalidOperation(String),
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
    pub status: i32,
    pub encrypted_payload_hex: String,
    pub nonce_hex: String,
    pub staged_payload_hex: Option<String>,
    pub staged_nonce_hex: Option<String>,
    pub staged_validated_at_ms: Option<i64>,
    pub previous_payload_hex: Option<String>,
    pub previous_nonce_hex: Option<String>,
    pub previous_validated_at_ms: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct VaultPayload {
    version: u32,
    records: Vec<CredentialRecord>,
}

#[derive(Serialize, Deserialize)]
struct SealedVaultFile {
    version: u32,
    nonce_hex: String,
    ciphertext_hex: String,
}

pub struct PersistentVault {
    file_path: PathBuf,
    kek: KeyEncryptionKey,
    records: HashMap<String, CredentialRecord>,
}

impl PersistentVault {
    pub fn open_or_create(
        path: impl AsRef<Path>,
        kek: KeyEncryptionKey,
    ) -> Result<Self, VaultError> {
        let file_path = path.as_ref().to_path_buf();
        let records = if file_path.exists() {
            let data = std::fs::read(&file_path)
                .map_err(|error| VaultError::Io(error.to_string()))?;
            if data.is_empty() {
                return Err(VaultError::DecryptionFailed);
            }
            Self::open_sealed_payload(&data, &kek)?
        } else {
            HashMap::new()
        };

        Ok(Self {
            file_path,
            kek,
            records,
        })
    }

    fn open_sealed_payload(
        data: &[u8],
        kek: &KeyEncryptionKey,
    ) -> Result<HashMap<String, CredentialRecord>, VaultError> {
        let envelope: SealedVaultFile =
            serde_json::from_slice(data).map_err(|_| VaultError::DecryptionFailed)?;
        if envelope.version != VAULT_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(envelope.version));
        }

        let plaintext = kek.decrypt_with_aad(
            &envelope.ciphertext_hex,
            &envelope.nonce_hex,
            VAULT_ENVELOPE_AAD,
        )?;
        let payload: VaultPayload = serde_json::from_slice(&plaintext.inner)
            .map_err(|_| VaultError::DecryptionFailed)?;
        if payload.version != VAULT_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(payload.version));
        }

        let mut records = HashMap::new();
        for record in payload.records {
            Self::validate_record(&record)?;
            if records.insert(record.profile_id.clone(), record).is_some() {
                return Err(VaultError::DecryptionFailed);
            }
        }
        Ok(records)
    }

    fn validate_record(record: &CredentialRecord) -> Result<(), VaultError> {
        if record.profile_id.trim().is_empty()
            || record.provider.trim().is_empty()
            || record.credential_type.trim().is_empty()
            || record.encrypted_payload_hex.is_empty()
            || record.nonce_hex.is_empty()
        {
            return Err(VaultError::InvalidOperation(
                "credential record has missing required fields".into(),
            ));
        }
        CredentialStatus::try_from(record.status).map_err(|_| {
            VaultError::InvalidOperation("credential record has invalid status".into())
        })?;
        if record.staged_payload_hex.is_some() != record.staged_nonce_hex.is_some()
            || record.previous_payload_hex.is_some() != record.previous_nonce_hex.is_some()
        {
            return Err(VaultError::InvalidOperation(
                "credential record contains a partial encrypted payload".into(),
            ));
        }
        if record.staged_payload_hex.is_none() && record.staged_validated_at_ms.is_some() {
            return Err(VaultError::InvalidOperation(
                "credential record validates a missing staged payload".into(),
            ));
        }
        Ok(())
    }

    fn sealed_bytes(&self) -> Result<Vec<u8>, VaultError> {
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        let payload = VaultPayload {
            version: VAULT_FORMAT_VERSION,
            records,
        };
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|_| VaultError::Serialization)?;
        let (ciphertext_hex, nonce_hex) =
            self.kek.encrypt_with_aad(&payload_bytes, VAULT_ENVELOPE_AAD)?;
        let envelope = SealedVaultFile {
            version: VAULT_FORMAT_VERSION,
            nonce_hex,
            ciphertext_hex,
        };
        serde_json::to_vec(&envelope).map_err(|_| VaultError::Serialization)
    }

    pub fn save(&self) -> Result<(), VaultError> {
        let data = self.sealed_bytes()?;
        atomic_write(&self.file_path, &data)
    }

    pub fn store_record(&mut self, record: CredentialRecord) -> Result<(), VaultError> {
        Self::validate_record(&record)?;
        if self.records.contains_key(&record.profile_id) {
            return Err(VaultError::InvalidOperation(format!(
                "credential profile already exists: {}",
                record.profile_id
            )));
        }

        let profile_id = record.profile_id.clone();
        self.records.insert(profile_id.clone(), record);
        if let Err(error) = self.save() {
            self.records.remove(&profile_id);
            return Err(error);
        }
        Ok(())
    }

    pub fn get_record(&self, profile_id: &str) -> Option<&CredentialRecord> {
        self.records.get(profile_id)
    }

    pub fn list_records(&self) -> Vec<CredentialRecord> {
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        records
    }

    pub fn stage_credential(
        &mut self,
        profile_id: &str,
        plaintext: &[u8],
    ) -> Result<(), VaultError> {
        if plaintext.is_empty() {
            return Err(VaultError::InvalidOperation(
                "staged credential must not be empty".into(),
            ));
        }
        let (staged_hex, nonce_hex) = self.kek.encrypt_secret(plaintext, profile_id)?;
        let previous = self
            .records
            .get(profile_id)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;

        {
            let record = self.records.get_mut(profile_id).expect("record exists");
            record.staged_payload_hex = Some(staged_hex);
            record.staged_nonce_hex = Some(nonce_hex);
            record.staged_validated_at_ms = None;
            record.status = CredentialStatus::Staged as i32;
        }
        if let Err(error) = self.save() {
            self.records.insert(profile_id.into(), previous);
            return Err(error);
        }
        Ok(())
    }

    /// Records successful provider validation of the staged secret.
    ///
    /// The adapter must perform validation before calling this method. Merely
    /// decrypting the staged value is not provider validation.
    pub fn mark_staged_validated(
        &mut self,
        profile_id: &str,
        validated_at_ms: i64,
    ) -> Result<(), VaultError> {
        if validated_at_ms <= 0 {
            return Err(VaultError::InvalidOperation(
                "validation timestamp must be positive".into(),
            ));
        }
        let previous = self
            .records
            .get(profile_id)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        if previous.staged_payload_hex.is_none() || previous.staged_nonce_hex.is_none() {
            return Err(VaultError::InvalidOperation(
                "no staged credential is available for validation".into(),
            ));
        }

        self.records
            .get_mut(profile_id)
            .expect("record exists")
            .staged_validated_at_ms = Some(validated_at_ms);
        if let Err(error) = self.save() {
            self.records.insert(profile_id.into(), previous);
            return Err(error);
        }
        Ok(())
    }

    pub fn activate_credential(&mut self, profile_id: &str) -> Result<(), VaultError> {
        let previous = self
            .records
            .get(profile_id)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        let staged = previous.staged_payload_hex.clone().ok_or_else(|| {
            VaultError::InvalidOperation("no staged credential is available".into())
        })?;
        let staged_nonce = previous.staged_nonce_hex.clone().ok_or_else(|| {
            VaultError::InvalidOperation("staged credential nonce is missing".into())
        })?;
        let validated_at = previous.staged_validated_at_ms.ok_or_else(|| {
            VaultError::InvalidOperation(
                "staged credential has not passed provider validation".into(),
            )
        })?;

        {
            let record = self.records.get_mut(profile_id).expect("record exists");
            record.previous_payload_hex = Some(record.encrypted_payload_hex.clone());
            record.previous_nonce_hex = Some(record.nonce_hex.clone());
            record.previous_validated_at_ms = Some(record.last_validated_at_ms);
            record.encrypted_payload_hex = staged;
            record.nonce_hex = staged_nonce;
            record.last_validated_at_ms = validated_at;
            record.staged_payload_hex = None;
            record.staged_nonce_hex = None;
            record.staged_validated_at_ms = None;
            record.status = CredentialStatus::Active as i32;
        }
        if let Err(error) = self.save() {
            self.records.insert(profile_id.into(), previous);
            return Err(error);
        }
        Ok(())
    }

    pub fn rollback_credential(&mut self, profile_id: &str) -> Result<(), VaultError> {
        let previous = self
            .records
            .get(profile_id)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        let rollback_payload = previous.previous_payload_hex.clone().ok_or_else(|| {
            VaultError::InvalidOperation("no previous credential is available".into())
        })?;
        let rollback_nonce = previous.previous_nonce_hex.clone().ok_or_else(|| {
            VaultError::InvalidOperation("previous credential nonce is missing".into())
        })?;

        {
            let record = self.records.get_mut(profile_id).expect("record exists");
            record.encrypted_payload_hex = rollback_payload;
            record.nonce_hex = rollback_nonce;
            record.last_validated_at_ms =
                record.previous_validated_at_ms.unwrap_or(record.last_validated_at_ms);
            record.previous_payload_hex = None;
            record.previous_nonce_hex = None;
            record.previous_validated_at_ms = None;
            record.staged_payload_hex = None;
            record.staged_nonce_hex = None;
            record.staged_validated_at_ms = None;
            record.status = CredentialStatus::Active as i32;
        }
        if let Err(error) = self.save() {
            self.records.insert(profile_id.into(), previous);
            return Err(error);
        }
        Ok(())
    }

    pub fn decrypt_active_secret(&self, profile_id: &str) -> Result<SecretBuffer, VaultError> {
        let record = self
            .records
            .get(profile_id)
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        self.kek.decrypt_secret(
            &record.encrypted_payload_hex,
            &record.nonce_hex,
            profile_id,
        )
    }

    /// Returns a newly sealed backup envelope. Metadata and ciphertext records
    /// are both protected by the vault KEK.
    pub fn export_encrypted_backup(&self) -> Result<Vec<u8>, VaultError> {
        self.sealed_bytes()
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
    pub fn derive_from_passphrase(
        passphrase: &[u8],
        salt: &[u8; 16],
    ) -> Result<Self, VaultError> {
        let mut key = [0u8; 32];
        Argon2::default()
            .hash_password_into(passphrase, salt, &mut key)
            .map_err(|_| VaultError::DerivationFailed)?;
        Ok(Self { key })
    }

    pub fn encrypt_secret(
        &self,
        plaintext: &[u8],
        profile_id: &str,
    ) -> Result<(String, String), VaultError> {
        self.encrypt_with_aad(plaintext, profile_id.as_bytes())
    }

    pub fn decrypt_secret(
        &self,
        ciphertext_hex: &str,
        nonce_hex: &str,
        profile_id: &str,
    ) -> Result<SecretBuffer, VaultError> {
        self.decrypt_with_aad(ciphertext_hex, nonce_hex, profile_id.as_bytes())
    }

    fn encrypt_with_aad(
        &self,
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<(String, String), VaultError> {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|_| VaultError::EncryptionFailed)?;
        let ciphertext = cipher
            .encrypt(
                &nonce.into(),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| VaultError::EncryptionFailed)?;
        Ok((hex::encode(ciphertext), hex::encode(nonce)))
    }

    fn decrypt_with_aad(
        &self,
        ciphertext_hex: &str,
        nonce_hex: &str,
        aad: &[u8],
    ) -> Result<SecretBuffer, VaultError> {
        let ciphertext =
            hex::decode(ciphertext_hex).map_err(|_| VaultError::DecryptionFailed)?;
        let nonce = hex::decode(nonce_hex).map_err(|_| VaultError::DecryptionFailed)?;
        if nonce.len() != 12 {
            return Err(VaultError::DecryptionFailed);
        }

        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|_| VaultError::DecryptionFailed)?;
        let plaintext = cipher
            .decrypt(
                nonce.as_slice().into(),
                Payload {
                    msg: &ciphertext,
                    aad,
                },
            )
            .map_err(|_| VaultError::DecryptionFailed)?;
        Ok(SecretBuffer { inner: plaintext })
    }
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<(), VaultError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| VaultError::Io(error.to_string()))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| VaultError::Io("vault path has no valid file name".into()))?;
    let random_suffix: u64 = OsRng.gen();
    let temporary_path = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        random_suffix
    ));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let write_result = (|| -> Result<(), VaultError> {
        let mut file = options
            .open(&temporary_path)
            .map_err(|error| VaultError::Io(error.to_string()))?;
        file.write_all(data)
            .map_err(|error| VaultError::Io(error.to_string()))?;
        file.sync_all()
            .map_err(|error| VaultError::Io(error.to_string()))?;
        std::fs::rename(&temporary_path, path)
            .map_err(|error| VaultError::Io(error.to_string()))?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| VaultError::Io(error.to_string()))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    write_result
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn decode(hex_str: &str) -> Result<Vec<u8>, ()> {
        if hex_str.len() % 2 != 0 {
            return Err(());
        }
        (0..hex_str.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex_str[index..index + 2], 16).map_err(|_| ()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "muxport-{label}-{}-{}.sealed",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn record(kek: &KeyEncryptionKey) -> CredentialRecord {
        let (ciphertext, nonce) = kek.encrypt_secret(b"secret-v1", "prof-1").unwrap();
        CredentialRecord {
            profile_id: "prof-1".into(),
            display_name: "Test Profile".into(),
            provider: "opencode_go".into(),
            credential_type: "api_key".into(),
            account_fingerprint: "TEST_FINGERPRINT".into(),
            created_at_ms: 1000,
            last_validated_at_ms: 1000,
            status: CredentialStatus::Active as i32,
            encrypted_payload_hex: ciphertext,
            nonce_hex: nonce,
            staged_payload_hex: None,
            staged_nonce_hex: None,
            staged_validated_at_ms: None,
            previous_payload_hex: None,
            previous_nonce_hex: None,
            previous_validated_at_ms: None,
        }
    }

    #[test]
    fn argon2_derivation_and_secret_encryption_round_trip() {
        let key =
            KeyEncryptionKey::derive_from_passphrase(b"TEST_PASSPHRASE", &[42u8; 16]).unwrap();
        let (ciphertext, nonce) = key
            .encrypt_secret(b"TEST_CREDENTIAL_PLACEHOLDER", "profile-1")
            .unwrap();
        let plaintext = key
            .decrypt_secret(&ciphertext, &nonce, "profile-1")
            .unwrap();
        assert_eq!(plaintext.inner, b"TEST_CREDENTIAL_PLACEHOLDER");
    }

    #[test]
    fn vault_rejects_wrong_context_key_and_tampered_ciphertext() {
        let salt = [7u8; 16];
        let key = KeyEncryptionKey::derive_from_passphrase(b"correct passphrase", &salt).unwrap();
        let wrong_key =
            KeyEncryptionKey::derive_from_passphrase(b"different passphrase", &salt).unwrap();
        let (ciphertext, nonce) = key
            .encrypt_secret(b"TEST_CREDENTIAL_PLACEHOLDER", "profile-a")
            .unwrap();

        assert!(matches!(
            key.decrypt_secret(&ciphertext, &nonce, "profile-b"),
            Err(VaultError::DecryptionFailed)
        ));
        assert!(matches!(
            wrong_key.decrypt_secret(&ciphertext, &nonce, "profile-a"),
            Err(VaultError::DecryptionFailed)
        ));

        let mut tampered = hex::decode(&ciphertext).unwrap();
        tampered[0] ^= 1;
        assert!(matches!(
            key.decrypt_secret(&hex::encode(tampered), &nonce, "profile-a"),
            Err(VaultError::DecryptionFailed)
        ));
    }

    #[test]
    fn persistent_vault_requires_validation_and_recovers_after_reopen() {
        let path = unique_path("vault");
        let salt = [9u8; 16];

        {
            let key =
                KeyEncryptionKey::derive_from_passphrase(b"TEST_PASSPHRASE", &salt).unwrap();
            let initial_record = record(&key);
            let mut vault = PersistentVault::open_or_create(&path, key).unwrap();
            vault.store_record(initial_record).unwrap();
            vault.stage_credential("prof-1", b"secret-v2").unwrap();

            assert!(matches!(
                vault.activate_credential("prof-1"),
                Err(VaultError::InvalidOperation(_))
            ));
            assert_eq!(
                vault.decrypt_active_secret("prof-1").unwrap().inner,
                b"secret-v1"
            );

            vault.mark_staged_validated("prof-1", 2000).unwrap();
            vault.activate_credential("prof-1").unwrap();
            assert_eq!(
                vault.decrypt_active_secret("prof-1").unwrap().inner,
                b"secret-v2"
            );
            vault.rollback_credential("prof-1").unwrap();
            assert_eq!(
                vault.decrypt_active_secret("prof-1").unwrap().inner,
                b"secret-v1"
            );

            let backup = vault.export_encrypted_backup().unwrap();
            let backup_text = String::from_utf8(backup).unwrap();
            assert!(!backup_text.contains("Test Profile"));
            assert!(!backup_text.contains("prof-1"));
        }

        let file_text = std::fs::read_to_string(&path).unwrap();
        assert!(!file_text.contains("Test Profile"));
        assert!(!file_text.contains("secret-v1"));

        let key = KeyEncryptionKey::derive_from_passphrase(b"TEST_PASSPHRASE", &salt).unwrap();
        let reopened = PersistentVault::open_or_create(&path, key).unwrap();
        assert_eq!(
            reopened.decrypt_active_secret("prof-1").unwrap().inner,
            b"secret-v1"
        );

        let wrong_key =
            KeyEncryptionKey::derive_from_passphrase(b"WRONG_PASSPHRASE", &salt).unwrap();
        assert!(matches!(
            PersistentVault::open_or_create(&path, wrong_key),
            Err(VaultError::DecryptionFailed)
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_persistence_rolls_back_in_memory_state() {
        let parent_file = unique_path("not-a-directory");
        std::fs::write(&parent_file, b"blocking file").unwrap();
        let path = parent_file.join("vault.sealed");
        let key =
            KeyEncryptionKey::derive_from_passphrase(b"TEST_PASSPHRASE", &[3u8; 16]).unwrap();
        let initial_record = record(&key);
        let mut vault = PersistentVault::open_or_create(&path, key).unwrap();

        assert!(matches!(
            vault.store_record(initial_record),
            Err(VaultError::Io(_))
        ));
        assert!(vault.get_record("prof-1").is_none());
        let _ = std::fs::remove_file(parent_file);
    }
}
