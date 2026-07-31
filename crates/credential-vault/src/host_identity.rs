use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::Zeroize;

const IDENTITY_RECORD_VERSION: u8 = 1;
const IDENTITY_SECRET_BYTES: usize = 32;
const IDENTITY_RECORD_BYTES: usize = 1 + IDENTITY_SECRET_BYTES;
const OS_KEYRING_SERVICE: &str = "io.muxport.connector";

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostIdentityError {
    #[error("host identity requires a non-empty host id of at most 256 bytes")]
    InvalidHostId,
    #[error("operating-system host identity storage is unavailable or locked")]
    StoreUnavailable,
    #[error("operating-system host identity storage failed")]
    StoreOperationFailed,
    #[error("stored host identity has an unsupported or corrupt format")]
    CorruptIdentity,
    #[error("new host identity could not be verified after persistence")]
    PersistenceVerificationFailed,
}

/// Narrow interface for the OS-protected secret holding the long-term host
/// identity. Implementations must never substitute a plaintext file.
pub trait HostIdentitySecretStore {
    fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, HostIdentityError>;
    fn set_secret(&self, name: &str, value: &[u8]) -> Result<(), HostIdentityError>;
}

pub struct LoadedHostIdentity {
    signing_key: SigningKey,
    created: bool,
}

impl LoadedHostIdentity {
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    pub fn was_created(&self) -> bool {
        self.created
    }

    pub fn public_key_hex(&self) -> String {
        encode_hex(self.signing_key.verifying_key().as_bytes())
    }

    pub fn into_signing_key(self) -> SigningKey {
        self.signing_key
    }
}

pub struct HostIdentityManager<S> {
    store: S,
}

impl<S: HostIdentitySecretStore> HostIdentityManager<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Loads the stable host signing key or creates it once in the protected
    /// store. A failed/unavailable store leaves pairing disabled; there is no
    /// plaintext or deterministic-key fallback.
    pub fn load_or_create(
        &self,
        host_id: &str,
    ) -> Result<LoadedHostIdentity, HostIdentityError> {
        validate_host_id(host_id)?;
        let entry_name = identity_entry_name(host_id);
        if let Some(mut record) = self.store.get_secret(&entry_name)? {
            let signing_key = decode_identity(&record);
            record.zeroize();
            return Ok(LoadedHostIdentity {
                signing_key: signing_key?,
                created: false,
            });
        }

        let signing_key = SigningKey::generate(&mut OsRng);
        let mut record = encode_identity(&signing_key);
        let persist_result = self.store.set_secret(&entry_name, &record);
        record.zeroize();
        persist_result?;

        let Some(mut persisted) = self.store.get_secret(&entry_name)? else {
            return Err(HostIdentityError::PersistenceVerificationFailed);
        };
        let persisted_key = decode_identity(&persisted);
        persisted.zeroize();
        let persisted_key = persisted_key?;
        if persisted_key.verifying_key() != signing_key.verifying_key() {
            return Err(HostIdentityError::PersistenceVerificationFailed);
        }

        Ok(LoadedHostIdentity {
            signing_key,
            created: true,
        })
    }

    /// Loads an existing protected identity without creating a replacement.
    /// Recovery and administrative commands use this so a typo in a host id
    /// cannot mint an unrelated long-term identity.
    pub fn load_existing(
        &self,
        host_id: &str,
    ) -> Result<Option<LoadedHostIdentity>, HostIdentityError> {
        validate_host_id(host_id)?;
        let entry_name = identity_entry_name(host_id);
        let Some(mut record) = self.store.get_secret(&entry_name)? else {
            return Ok(None);
        };
        let signing_key = decode_identity(&record);
        record.zeroize();
        Ok(Some(LoadedHostIdentity {
            signing_key: signing_key?,
            created: false,
        }))
    }
}

#[derive(Default)]
pub struct OsHostIdentityStore;

impl OsHostIdentityStore {
    pub fn new() -> Self {
        Self
    }
}

impl HostIdentitySecretStore for OsHostIdentityStore {
    fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, HostIdentityError> {
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ))]
        {
            let entry = keyring::Entry::new(OS_KEYRING_SERVICE, name)
                .map_err(|_| HostIdentityError::StoreUnavailable)?;
            match entry.get_secret() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(HostIdentityError::StoreUnavailable),
            }
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            let _ = name;
            Err(HostIdentityError::StoreUnavailable)
        }
    }

    fn set_secret(&self, name: &str, value: &[u8]) -> Result<(), HostIdentityError> {
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ))]
        {
            let entry = keyring::Entry::new(OS_KEYRING_SERVICE, name)
                .map_err(|_| HostIdentityError::StoreUnavailable)?;
            entry
                .set_secret(value)
                .map_err(|_| HostIdentityError::StoreOperationFailed)
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            let _ = (name, value);
            Err(HostIdentityError::StoreUnavailable)
        }
    }
}

fn validate_host_id(host_id: &str) -> Result<(), HostIdentityError> {
    if host_id.trim().is_empty() || host_id.len() > 256 {
        Err(HostIdentityError::InvalidHostId)
    } else {
        Ok(())
    }
}

fn identity_entry_name(host_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"muxport-host-identity-entry-v1");
    hasher.update((host_id.len() as u64).to_be_bytes());
    hasher.update(host_id.as_bytes());
    format!("host-{}", encode_hex(&hasher.finalize()))
}

fn encode_identity(signing_key: &SigningKey) -> Vec<u8> {
    let mut record = Vec::with_capacity(IDENTITY_RECORD_BYTES);
    record.push(IDENTITY_RECORD_VERSION);
    record.extend_from_slice(&signing_key.to_bytes());
    record
}

fn decode_identity(record: &[u8]) -> Result<SigningKey, HostIdentityError> {
    if record.len() != IDENTITY_RECORD_BYTES
        || record[0] != IDENTITY_RECORD_VERSION
    {
        return Err(HostIdentityError::CorruptIdentity);
    }
    let mut secret = [0_u8; IDENTITY_SECRET_BYTES];
    secret.copy_from_slice(&record[1..]);
    let signing_key = SigningKey::from_bytes(&secret);
    secret.zeroize();
    Ok(signing_key)
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
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MemorySecretStore {
        values: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        unavailable: bool,
    }

    impl HostIdentitySecretStore for MemorySecretStore {
        fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, HostIdentityError> {
            if self.unavailable {
                return Err(HostIdentityError::StoreUnavailable);
            }
            Ok(self.values.lock().unwrap().get(name).cloned())
        }

        fn set_secret(&self, name: &str, value: &[u8]) -> Result<(), HostIdentityError> {
            if self.unavailable {
                return Err(HostIdentityError::StoreUnavailable);
            }
            self.values
                .lock()
                .unwrap()
                .insert(name.to_owned(), value.to_vec());
            Ok(())
        }
    }

    #[test]
    fn creates_once_and_reloads_the_same_identity() {
        let store = MemorySecretStore::default();
        let first = HostIdentityManager::new(store.clone())
            .load_or_create("host-1")
            .unwrap();
        assert!(first.was_created());
        let second = HostIdentityManager::new(store.clone())
            .load_or_create("host-1")
            .unwrap();
        assert!(!second.was_created());
        assert_eq!(first.public_key_hex(), second.public_key_hex());
        assert_eq!(store.values.lock().unwrap().len(), 1);
    }

    #[test]
    fn different_hosts_have_independent_identities() {
        let store = MemorySecretStore::default();
        let first = HostIdentityManager::new(store.clone())
            .load_or_create("host-1")
            .unwrap();
        let second = HostIdentityManager::new(store)
            .load_or_create("host-2")
            .unwrap();
        assert_ne!(first.public_key_hex(), second.public_key_hex());
    }

    #[test]
    fn load_existing_never_creates_a_missing_identity() {
        let store = MemorySecretStore::default();
        let manager = HostIdentityManager::new(store.clone());

        assert!(manager.load_existing("host-missing").unwrap().is_none());
        assert!(store.values.lock().unwrap().is_empty());

        manager.load_or_create("host-1").unwrap();
        let loaded = manager.load_existing("host-1").unwrap().unwrap();
        assert!(!loaded.was_created());
        assert_eq!(store.values.lock().unwrap().len(), 1);
    }

    #[test]
    fn corrupt_record_and_unavailable_store_fail_closed() {
        let store = MemorySecretStore::default();
        let entry_name = identity_entry_name("host-1");
        store
            .values
            .lock()
            .unwrap()
            .insert(entry_name, vec![IDENTITY_RECORD_VERSION, 1, 2, 3]);
        assert!(matches!(
            HostIdentityManager::new(store)
                .load_or_create("host-1"),
            Err(HostIdentityError::CorruptIdentity)
        ));

        let unavailable = MemorySecretStore {
            unavailable: true,
            ..MemorySecretStore::default()
        };
        assert!(matches!(
            HostIdentityManager::new(unavailable)
                .load_or_create("host-1"),
            Err(HostIdentityError::StoreUnavailable)
        ));
    }

    #[test]
    fn invalid_host_ids_are_rejected_before_store_access() {
        let store = MemorySecretStore::default();
        assert!(matches!(
            HostIdentityManager::new(store).load_or_create(" "),
            Err(HostIdentityError::InvalidHostId)
        ));
    }
}
