use crate::KeyEncryptionKey;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::Zeroize;

const VAULT_KEY_RECORD_VERSION: u8 = 1;
const VAULT_KEY_BYTES: usize = 32;
const VAULT_KEY_RECORD_BYTES: usize = 1 + VAULT_KEY_BYTES;
const OS_KEYRING_SERVICE: &str = "io.muxport.connector";

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultKeyError {
    #[error("vault key requires a non-empty host id of at most 256 bytes")]
    InvalidHostId,
    #[error("operating-system vault-key storage is unavailable or locked")]
    StoreUnavailable,
    #[error("operating-system vault-key storage failed")]
    StoreOperationFailed,
    #[error("stored vault key has an unsupported or corrupt format")]
    CorruptKey,
    #[error("new vault key could not be verified after persistence")]
    PersistenceVerificationFailed,
}

pub trait VaultKeySecretStore {
    fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultKeyError>;
    fn set_secret(&self, name: &str, value: &[u8]) -> Result<(), VaultKeyError>;
}

pub struct LoadedVaultKey {
    key: KeyEncryptionKey,
    created: bool,
}

impl LoadedVaultKey {
    pub fn was_created(&self) -> bool {
        self.created
    }

    pub fn into_key_encryption_key(self) -> KeyEncryptionKey {
        self.key
    }
}

pub struct VaultKeyManager<S> {
    store: S,
}

impl<S: VaultKeySecretStore> VaultKeyManager<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn load_or_create(&self, host_id: &str) -> Result<LoadedVaultKey, VaultKeyError> {
        validate_host_id(host_id)?;
        let entry_name = vault_key_entry_name(host_id);
        if let Some(mut record) = self.store.get_secret(&entry_name)? {
            let key = decode_key(&record);
            record.zeroize();
            return Ok(LoadedVaultKey {
                key: key?,
                created: false,
            });
        }

        let mut key_bytes = [0_u8; VAULT_KEY_BYTES];
        OsRng.fill_bytes(&mut key_bytes);
        let mut record = encode_key(&key_bytes);
        let persist_result = self.store.set_secret(&entry_name, &record);
        record.zeroize();
        if let Err(error) = persist_result {
            key_bytes.zeroize();
            return Err(error);
        }

        let persisted_result = self.store.get_secret(&entry_name);
        let Some(mut persisted) = (match persisted_result {
            Ok(value) => value,
            Err(error) => {
                key_bytes.zeroize();
                return Err(error);
            }
        }) else {
            key_bytes.zeroize();
            return Err(VaultKeyError::PersistenceVerificationFailed);
        };
        let persisted_bytes = decode_key_bytes(&persisted);
        persisted.zeroize();
        let mut persisted_bytes = match persisted_bytes {
            Ok(value) => value,
            Err(error) => {
                key_bytes.zeroize();
                return Err(error);
            }
        };
        if persisted_bytes.ct_eq(&key_bytes).unwrap_u8() != 1 {
            persisted_bytes.zeroize();
            key_bytes.zeroize();
            return Err(VaultKeyError::PersistenceVerificationFailed);
        }
        persisted_bytes.zeroize();
        Ok(LoadedVaultKey {
            key: KeyEncryptionKey::from_key_bytes(key_bytes),
            created: true,
        })
    }
}

#[derive(Default)]
pub struct OsVaultKeyStore;

impl OsVaultKeyStore {
    pub fn new() -> Self {
        Self
    }
}

impl VaultKeySecretStore for OsVaultKeyStore {
    fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultKeyError> {
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ))]
        {
            let entry = keyring::Entry::new(OS_KEYRING_SERVICE, name)
                .map_err(|_| VaultKeyError::StoreUnavailable)?;
            match entry.get_secret() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(VaultKeyError::StoreUnavailable),
            }
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            let _ = name;
            Err(VaultKeyError::StoreUnavailable)
        }
    }

    fn set_secret(&self, name: &str, value: &[u8]) -> Result<(), VaultKeyError> {
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ))]
        {
            let entry = keyring::Entry::new(OS_KEYRING_SERVICE, name)
                .map_err(|_| VaultKeyError::StoreUnavailable)?;
            entry
                .set_secret(value)
                .map_err(|_| VaultKeyError::StoreOperationFailed)
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            let _ = (name, value);
            Err(VaultKeyError::StoreUnavailable)
        }
    }
}

fn validate_host_id(host_id: &str) -> Result<(), VaultKeyError> {
    if host_id.trim().is_empty() || host_id.len() > 256 {
        Err(VaultKeyError::InvalidHostId)
    } else {
        Ok(())
    }
}

fn vault_key_entry_name(host_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"muxport-vault-key-entry-v1");
    hasher.update((host_id.len() as u64).to_be_bytes());
    hasher.update(host_id.as_bytes());
    format!("vault-{}", encode_hex(&hasher.finalize()))
}

fn encode_key(key: &[u8; VAULT_KEY_BYTES]) -> Vec<u8> {
    let mut record = Vec::with_capacity(VAULT_KEY_RECORD_BYTES);
    record.push(VAULT_KEY_RECORD_VERSION);
    record.extend_from_slice(key);
    record
}

fn decode_key(record: &[u8]) -> Result<KeyEncryptionKey, VaultKeyError> {
    Ok(KeyEncryptionKey::from_key_bytes(decode_key_bytes(record)?))
}

fn decode_key_bytes(record: &[u8]) -> Result<[u8; VAULT_KEY_BYTES], VaultKeyError> {
    if record.len() != VAULT_KEY_RECORD_BYTES || record[0] != VAULT_KEY_RECORD_VERSION {
        return Err(VaultKeyError::CorruptKey);
    }
    let mut key = [0_u8; VAULT_KEY_BYTES];
    key.copy_from_slice(&record[1..]);
    Ok(key)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CredentialEnrollment, PersistentVault};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MemorySecretStore {
        values: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        unavailable: bool,
    }

    impl VaultKeySecretStore for MemorySecretStore {
        fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultKeyError> {
            if self.unavailable {
                return Err(VaultKeyError::StoreUnavailable);
            }
            Ok(self.values.lock().unwrap().get(name).cloned())
        }

        fn set_secret(&self, name: &str, value: &[u8]) -> Result<(), VaultKeyError> {
            if self.unavailable {
                return Err(VaultKeyError::StoreUnavailable);
            }
            self.values
                .lock()
                .unwrap()
                .insert(name.to_owned(), value.to_vec());
            Ok(())
        }
    }

    #[test]
    fn creates_once_and_reopens_a_vault_with_the_same_protected_key() {
        let store = MemorySecretStore::default();
        let path = std::env::temp_dir().join(format!(
            "muxport-protected-vault-key-{}-{}.sealed",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let first = VaultKeyManager::new(store.clone())
            .load_or_create("host-1")
            .unwrap();
        assert!(first.was_created());
        {
            let mut vault = PersistentVault::open_or_create(
                &path,
                "host-1",
                first.into_key_encryption_key(),
            )
            .unwrap();
            vault
                .enroll_credential(
                    CredentialEnrollment {
                        profile_id: "profile-1".into(),
                        display_name: "Profile".into(),
                        provider: "provider".into(),
                        credential_type: "api_key".into(),
                        account_fingerprint: "fingerprint".into(),
                        created_at_ms: 1000,
                        last_validated_at_ms: 1000,
                    },
                    b"secret",
                )
                .unwrap();
        }
        let second = VaultKeyManager::new(store)
            .load_or_create("host-1")
            .unwrap();
        assert!(!second.was_created());
        let reopened = PersistentVault::open_or_create(
            &path,
            "host-1",
            second.into_key_encryption_key(),
        )
        .unwrap();
        assert_eq!(
            reopened
                .decrypt_active_secret("profile-1")
                .unwrap()
                .expose_secret(),
            b"secret"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn corrupt_or_unavailable_store_fails_closed() {
        let store = MemorySecretStore::default();
        store
            .values
            .lock()
            .unwrap()
            .insert(vault_key_entry_name("host-1"), vec![1, 2, 3]);
        assert!(matches!(
            VaultKeyManager::new(store).load_or_create("host-1"),
            Err(VaultKeyError::CorruptKey)
        ));
        let unavailable = MemorySecretStore {
            unavailable: true,
            ..MemorySecretStore::default()
        };
        assert!(matches!(
            VaultKeyManager::new(unavailable).load_or_create("host-1"),
            Err(VaultKeyError::StoreUnavailable)
        ));
    }
}
