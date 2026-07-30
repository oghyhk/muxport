use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use muxport_protocol::CredentialStatus;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const LEGACY_VAULT_FORMAT_VERSION: u32 = 1;
const VAULT_FORMAT_VERSION: u32 = 2;
const LEGACY_VAULT_ENVELOPE_AAD: &[u8] = b"muxport-vault-envelope-v1";
const WRAPPED_DEK_AAD_DOMAIN: &[u8] = b"muxport-vault-wrapped-dek-v2";
const PAYLOAD_AAD_DOMAIN: &[u8] = b"muxport-vault-payload-v2";
const RECORD_AAD_DOMAIN: &[u8] = b"muxport-vault-record-v2";
const MAX_SECRET_BYTES: usize = 64 * 1024;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Decryption failed: invalid key, context, or corrupted envelope")]
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
    #[error("Vault is bound to a different host")]
    HostContextMismatch,
    #[error("Invalid credential operation: {0}")]
    InvalidOperation(String),
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretBuffer {
    inner: Vec<u8>,
}

impl SecretBuffer {
    pub fn expose_secret(&self) -> &[u8] {
        &self.inner
    }
}

#[derive(Clone, Debug)]
pub struct CredentialEnrollment {
    pub profile_id: String,
    pub display_name: String,
    pub provider: String,
    pub credential_type: String,
    pub account_fingerprint: String,
    pub created_at_ms: i64,
    pub last_validated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CredentialSummary {
    pub profile_id: String,
    pub display_name: String,
    pub provider: String,
    pub credential_type: String,
    pub account_fingerprint: String,
    pub created_at_ms: i64,
    pub last_validated_at_ms: i64,
    pub status: CredentialStatus,
    pub has_staged_credential: bool,
    pub has_rollback_credential: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct StoredCredentialRecord {
    profile_id: String,
    display_name: String,
    provider: String,
    credential_type: String,
    account_fingerprint: String,
    created_at_ms: i64,
    last_validated_at_ms: i64,
    status: i32,
    encrypted_payload_hex: String,
    nonce_hex: String,
    staged_payload_hex: Option<String>,
    staged_nonce_hex: Option<String>,
    staged_validated_at_ms: Option<i64>,
    previous_payload_hex: Option<String>,
    previous_nonce_hex: Option<String>,
    previous_validated_at_ms: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct VaultPayload {
    version: u32,
    records: Vec<StoredCredentialRecord>,
}

#[derive(Serialize, Deserialize)]
struct SealedVaultFile {
    version: u32,
    host_id: String,
    wrapped_dek_hex: String,
    wrapped_dek_nonce_hex: String,
    payload_ciphertext_hex: String,
    payload_nonce_hex: String,
}

#[derive(Deserialize)]
struct FormatProbe {
    version: u32,
}

#[derive(Serialize, Deserialize)]
struct LegacySealedVaultFile {
    version: u32,
    nonce_hex: String,
    ciphertext_hex: String,
}

#[derive(Serialize, Deserialize)]
struct LegacyVaultPayload {
    version: u32,
    records: Vec<LegacyCredentialRecord>,
}

#[derive(Serialize, Deserialize)]
struct LegacyCredentialRecord {
    profile_id: String,
    display_name: String,
    provider: String,
    credential_type: String,
    account_fingerprint: String,
    created_at_ms: i64,
    last_validated_at_ms: i64,
    status: i32,
    encrypted_payload_hex: String,
    nonce_hex: String,
    staged_payload_hex: Option<String>,
    staged_nonce_hex: Option<String>,
    staged_validated_at_ms: Option<i64>,
    previous_payload_hex: Option<String>,
    previous_nonce_hex: Option<String>,
    previous_validated_at_ms: Option<i64>,
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct DataEncryptionKey {
    key: [u8; 32],
}

impl DataEncryptionKey {
    fn generate() -> Self {
        let mut key = [0_u8; 32];
        OsRng.fill_bytes(&mut key);
        Self { key }
    }
}

pub struct PersistentVault {
    file_path: PathBuf,
    host_id: String,
    kek: KeyEncryptionKey,
    dek: DataEncryptionKey,
    records: HashMap<String, StoredCredentialRecord>,
}

impl PersistentVault {
    pub fn open_or_create(
        path: impl AsRef<Path>,
        host_id: &str,
        kek: KeyEncryptionKey,
    ) -> Result<Self, VaultError> {
        validate_host_id(host_id)?;
        let file_path = path.as_ref().to_path_buf();
        if !file_path.exists() {
            let vault = Self {
                file_path,
                host_id: host_id.to_owned(),
                kek,
                dek: DataEncryptionKey::generate(),
                records: HashMap::new(),
            };
            vault.save()?;
            return Ok(vault);
        }

        let data =
            std::fs::read(&file_path).map_err(|error| VaultError::Io(error.to_string()))?;
        if data.is_empty() {
            return Err(VaultError::DecryptionFailed);
        }
        let probe: FormatProbe =
            serde_json::from_slice(&data).map_err(|_| VaultError::DecryptionFailed)?;
        match probe.version {
            VAULT_FORMAT_VERSION => Self::open_v2(file_path, host_id, kek, &data),
            LEGACY_VAULT_FORMAT_VERSION => {
                Self::migrate_v1(file_path, host_id, kek, &data)
            }
            version => Err(VaultError::UnsupportedVersion(version)),
        }
    }

    fn open_v2(
        file_path: PathBuf,
        host_id: &str,
        kek: KeyEncryptionKey,
        data: &[u8],
    ) -> Result<Self, VaultError> {
        let envelope: SealedVaultFile =
            serde_json::from_slice(data).map_err(|_| VaultError::DecryptionFailed)?;
        if envelope.version != VAULT_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(envelope.version));
        }
        if envelope.host_id != host_id {
            return Err(VaultError::HostContextMismatch);
        }

        let wrapped_aad = wrapped_dek_aad(host_id);
        let mut dek_plaintext = decrypt(
            &kek.key,
            &envelope.wrapped_dek_hex,
            &envelope.wrapped_dek_nonce_hex,
            &wrapped_aad,
        )?;
        if dek_plaintext.inner.len() != 32 {
            return Err(VaultError::DecryptionFailed);
        }
        let mut dek_bytes = [0_u8; 32];
        dek_bytes.copy_from_slice(&dek_plaintext.inner);
        dek_plaintext.inner.zeroize();
        let dek = DataEncryptionKey { key: dek_bytes };

        let payload_aad = payload_aad(host_id);
        let payload_plaintext = decrypt(
            &dek.key,
            &envelope.payload_ciphertext_hex,
            &envelope.payload_nonce_hex,
            &payload_aad,
        )?;
        let payload: VaultPayload = serde_json::from_slice(payload_plaintext.expose_secret())
            .map_err(|_| VaultError::DecryptionFailed)?;
        if payload.version != VAULT_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(payload.version));
        }
        let records = collect_records(payload.records)?;
        Ok(Self {
            file_path,
            host_id: host_id.to_owned(),
            kek,
            dek,
            records,
        })
    }

    fn migrate_v1(
        file_path: PathBuf,
        host_id: &str,
        kek: KeyEncryptionKey,
        data: &[u8],
    ) -> Result<Self, VaultError> {
        let envelope: LegacySealedVaultFile =
            serde_json::from_slice(data).map_err(|_| VaultError::DecryptionFailed)?;
        if envelope.version != LEGACY_VAULT_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(envelope.version));
        }
        let legacy_payload = decrypt(
            &kek.key,
            &envelope.ciphertext_hex,
            &envelope.nonce_hex,
            LEGACY_VAULT_ENVELOPE_AAD,
        )?;
        let payload: LegacyVaultPayload =
            serde_json::from_slice(legacy_payload.expose_secret())
                .map_err(|_| VaultError::DecryptionFailed)?;
        if payload.version != LEGACY_VAULT_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(payload.version));
        }

        let dek = DataEncryptionKey::generate();
        let mut records = HashMap::new();
        for legacy in payload.records {
            validate_legacy_record(&legacy)?;
            let aad = record_aad(
                host_id,
                &legacy.profile_id,
                &legacy.provider,
                &legacy.credential_type,
            );
            let active = decrypt(
                &kek.key,
                &legacy.encrypted_payload_hex,
                &legacy.nonce_hex,
                legacy.profile_id.as_bytes(),
            )?;
            let (encrypted_payload_hex, nonce_hex) =
                encrypt(&dek.key, active.expose_secret(), &aad)?;
            let (staged_payload_hex, staged_nonce_hex) = migrate_legacy_slot(
                &kek,
                &dek,
                &aad,
                &legacy.profile_id,
                legacy.staged_payload_hex.as_deref(),
                legacy.staged_nonce_hex.as_deref(),
            )?;
            let (previous_payload_hex, previous_nonce_hex) = migrate_legacy_slot(
                &kek,
                &dek,
                &aad,
                &legacy.profile_id,
                legacy.previous_payload_hex.as_deref(),
                legacy.previous_nonce_hex.as_deref(),
            )?;
            let record = StoredCredentialRecord {
                profile_id: legacy.profile_id,
                display_name: legacy.display_name,
                provider: legacy.provider,
                credential_type: legacy.credential_type,
                account_fingerprint: legacy.account_fingerprint,
                created_at_ms: legacy.created_at_ms,
                last_validated_at_ms: legacy.last_validated_at_ms,
                status: legacy.status,
                encrypted_payload_hex,
                nonce_hex,
                staged_payload_hex,
                staged_nonce_hex,
                staged_validated_at_ms: legacy.staged_validated_at_ms,
                previous_payload_hex,
                previous_nonce_hex,
                previous_validated_at_ms: legacy.previous_validated_at_ms,
            };
            validate_record(&record)?;
            if records.insert(record.profile_id.clone(), record).is_some() {
                return Err(VaultError::DecryptionFailed);
            }
        }

        let vault = Self {
            file_path,
            host_id: host_id.to_owned(),
            kek,
            dek,
            records,
        };
        vault.save()?;
        Ok(vault)
    }

    fn sealed_bytes(&self) -> Result<Vec<u8>, VaultError> {
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        let payload = VaultPayload {
            version: VAULT_FORMAT_VERSION,
            records,
        };
        let mut payload_bytes =
            serde_json::to_vec(&payload).map_err(|_| VaultError::Serialization)?;
        let payload_aad = payload_aad(&self.host_id);
        let encrypted_payload = encrypt(&self.dek.key, &payload_bytes, &payload_aad);
        payload_bytes.zeroize();
        let (payload_ciphertext_hex, payload_nonce_hex) = encrypted_payload?;

        let wrapped_aad = wrapped_dek_aad(&self.host_id);
        let (wrapped_dek_hex, wrapped_dek_nonce_hex) =
            encrypt(&self.kek.key, &self.dek.key, &wrapped_aad)?;
        let envelope = SealedVaultFile {
            version: VAULT_FORMAT_VERSION,
            host_id: self.host_id.clone(),
            wrapped_dek_hex,
            wrapped_dek_nonce_hex,
            payload_ciphertext_hex,
            payload_nonce_hex,
        };
        serde_json::to_vec(&envelope).map_err(|_| VaultError::Serialization)
    }

    pub fn save(&self) -> Result<(), VaultError> {
        atomic_write(&self.file_path, &self.sealed_bytes()?)
    }

    pub fn enroll_credential(
        &mut self,
        enrollment: CredentialEnrollment,
        plaintext: &[u8],
    ) -> Result<(), VaultError> {
        validate_enrollment(&enrollment)?;
        validate_secret(plaintext)?;
        if self.records.contains_key(&enrollment.profile_id) {
            return Err(VaultError::InvalidOperation(format!(
                "credential profile already exists: {}",
                enrollment.profile_id
            )));
        }
        let aad = record_aad(
            &self.host_id,
            &enrollment.profile_id,
            &enrollment.provider,
            &enrollment.credential_type,
        );
        let (encrypted_payload_hex, nonce_hex) =
            encrypt(&self.dek.key, plaintext, &aad)?;
        let record = StoredCredentialRecord {
            profile_id: enrollment.profile_id,
            display_name: enrollment.display_name,
            provider: enrollment.provider,
            credential_type: enrollment.credential_type,
            account_fingerprint: enrollment.account_fingerprint,
            created_at_ms: enrollment.created_at_ms,
            last_validated_at_ms: enrollment.last_validated_at_ms,
            status: CredentialStatus::Active as i32,
            encrypted_payload_hex,
            nonce_hex,
            staged_payload_hex: None,
            staged_nonce_hex: None,
            staged_validated_at_ms: None,
            previous_payload_hex: None,
            previous_nonce_hex: None,
            previous_validated_at_ms: None,
        };
        validate_record(&record)?;
        let profile_id = record.profile_id.clone();
        self.records.insert(profile_id.clone(), record);
        if let Err(error) = self.save() {
            self.records.remove(&profile_id);
            return Err(error);
        }
        Ok(())
    }

    pub fn get_profile(&self, profile_id: &str) -> Option<CredentialSummary> {
        self.records.get(profile_id).and_then(record_summary)
    }

    pub fn list_profiles(&self) -> Vec<CredentialSummary> {
        let mut records = self
            .records
            .values()
            .filter_map(record_summary)
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        records
    }

    pub fn stage_credential(
        &mut self,
        profile_id: &str,
        plaintext: &[u8],
    ) -> Result<(), VaultError> {
        validate_secret(plaintext)?;
        let previous = self
            .records
            .get(profile_id)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        let previous_status = CredentialStatus::try_from(previous.status).map_err(|_| {
            VaultError::InvalidOperation("credential has invalid status".into())
        })?;
        if previous_status != CredentialStatus::Active
            || previous.staged_payload_hex.is_some()
        {
            return Err(VaultError::InvalidOperation(
                "credential is not eligible for staging".into(),
            ));
        }
        let aad = record_aad(
            &self.host_id,
            &previous.profile_id,
            &previous.provider,
            &previous.credential_type,
        );
        let (staged_payload_hex, staged_nonce_hex) =
            encrypt(&self.dek.key, plaintext, &aad)?;
        let record = self
            .records
            .get_mut(profile_id)
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        record.staged_payload_hex = Some(staged_payload_hex);
        record.staged_nonce_hex = Some(staged_nonce_hex);
        record.staged_validated_at_ms = None;
        record.status = CredentialStatus::Staged as i32;
        if let Err(error) = self.save() {
            self.records.insert(profile_id.into(), previous);
            return Err(error);
        }
        Ok(())
    }

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
        let record = self
            .records
            .get_mut(profile_id)
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        record.staged_validated_at_ms = Some(validated_at_ms);
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
        let record = self
            .records
            .get_mut(profile_id)
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
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
        let record = self
            .records
            .get_mut(profile_id)
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
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
        if let Err(error) = self.save() {
            self.records.insert(profile_id.into(), previous);
            return Err(error);
        }
        Ok(())
    }

    pub fn discard_staged_credential(&mut self, profile_id: &str) -> Result<(), VaultError> {
        let previous = self
            .records
            .get(profile_id)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        if previous.status != CredentialStatus::Staged as i32
            || previous.staged_payload_hex.is_none()
            || previous.staged_nonce_hex.is_none()
        {
            return Err(VaultError::InvalidOperation(
                "no staged credential is available to discard".into(),
            ));
        }
        let record = self
            .records
            .get_mut(profile_id)
            .ok_or_else(|| VaultError::NotFound(profile_id.into()))?;
        record.staged_payload_hex = None;
        record.staged_nonce_hex = None;
        record.staged_validated_at_ms = None;
        record.status = CredentialStatus::Active as i32;
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
        let status = CredentialStatus::try_from(record.status).map_err(|_| {
            VaultError::InvalidOperation("credential has invalid status".into())
        })?;
        if !matches!(
            status,
            CredentialStatus::Active
                | CredentialStatus::Staged
                | CredentialStatus::CoolingDown
        ) {
            return Err(VaultError::InvalidOperation(
                "credential is not eligible for use".into(),
            ));
        }
        let aad = record_aad(
            &self.host_id,
            &record.profile_id,
            &record.provider,
            &record.credential_type,
        );
        decrypt(
            &self.dek.key,
            &record.encrypted_payload_hex,
            &record.nonce_hex,
            &aad,
        )
    }

    pub fn export_encrypted_backup(&self) -> Result<Vec<u8>, VaultError> {
        self.sealed_bytes()
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct KeyEncryptionKey {
    key: [u8; 32],
}

impl KeyEncryptionKey {
    pub fn derive_from_passphrase(
        passphrase: &[u8],
        salt: &[u8; 16],
    ) -> Result<Self, VaultError> {
        let mut key = [0_u8; 32];
        Argon2::default()
            .hash_password_into(passphrase, salt, &mut key)
            .map_err(|_| VaultError::DerivationFailed)?;
        Ok(Self { key })
    }
}

fn collect_records(
    records: Vec<StoredCredentialRecord>,
) -> Result<HashMap<String, StoredCredentialRecord>, VaultError> {
    let mut result = HashMap::new();
    for record in records {
        validate_record(&record)?;
        if result.insert(record.profile_id.clone(), record).is_some() {
            return Err(VaultError::DecryptionFailed);
        }
    }
    Ok(result)
}

fn validate_host_id(host_id: &str) -> Result<(), VaultError> {
    if host_id.trim().is_empty() || host_id.len() > 256 {
        Err(VaultError::InvalidOperation(
            "host id must contain 1 to 256 bytes".into(),
        ))
    } else {
        Ok(())
    }
}

fn validate_enrollment(enrollment: &CredentialEnrollment) -> Result<(), VaultError> {
    if !valid_field(&enrollment.profile_id, 256)
        || !valid_field(&enrollment.display_name, 256)
        || !valid_field(&enrollment.provider, 128)
        || !valid_field(&enrollment.credential_type, 128)
        || !valid_field(&enrollment.account_fingerprint, 256)
        || enrollment.created_at_ms <= 0
        || enrollment.last_validated_at_ms <= 0
    {
        return Err(VaultError::InvalidOperation(
            "credential enrollment metadata is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_record(record: &StoredCredentialRecord) -> Result<(), VaultError> {
    let enrollment = CredentialEnrollment {
        profile_id: record.profile_id.clone(),
        display_name: record.display_name.clone(),
        provider: record.provider.clone(),
        credential_type: record.credential_type.clone(),
        account_fingerprint: record.account_fingerprint.clone(),
        created_at_ms: record.created_at_ms,
        last_validated_at_ms: record.last_validated_at_ms,
    };
    validate_enrollment(&enrollment)?;
    let status = CredentialStatus::try_from(record.status)
        .map_err(|_| VaultError::InvalidOperation("invalid credential status".into()))?;
    let has_staged = record.staged_payload_hex.is_some()
        && record.staged_nonce_hex.is_some();
    if status == CredentialStatus::Unspecified
        || (status == CredentialStatus::Staged) != has_staged
        || !valid_slot(&record.encrypted_payload_hex, &record.nonce_hex)
        || !valid_optional_slot(
            record.staged_payload_hex.as_deref(),
            record.staged_nonce_hex.as_deref(),
        )
        || !valid_optional_slot(
            record.previous_payload_hex.as_deref(),
            record.previous_nonce_hex.as_deref(),
        )
        || (record.staged_payload_hex.is_none() && record.staged_validated_at_ms.is_some())
        || (record.previous_payload_hex.is_none() && record.previous_validated_at_ms.is_some())
        || record.staged_validated_at_ms.is_some_and(|value| value <= 0)
        || record.previous_validated_at_ms.is_some_and(|value| value <= 0)
    {
        return Err(VaultError::DecryptionFailed);
    }
    Ok(())
}

fn validate_legacy_record(record: &LegacyCredentialRecord) -> Result<(), VaultError> {
    let stored = StoredCredentialRecord {
        profile_id: record.profile_id.clone(),
        display_name: record.display_name.clone(),
        provider: record.provider.clone(),
        credential_type: record.credential_type.clone(),
        account_fingerprint: record.account_fingerprint.clone(),
        created_at_ms: record.created_at_ms,
        last_validated_at_ms: record.last_validated_at_ms,
        status: record.status,
        encrypted_payload_hex: record.encrypted_payload_hex.clone(),
        nonce_hex: record.nonce_hex.clone(),
        staged_payload_hex: record.staged_payload_hex.clone(),
        staged_nonce_hex: record.staged_nonce_hex.clone(),
        staged_validated_at_ms: record.staged_validated_at_ms,
        previous_payload_hex: record.previous_payload_hex.clone(),
        previous_nonce_hex: record.previous_nonce_hex.clone(),
        previous_validated_at_ms: record.previous_validated_at_ms,
    };
    validate_record(&stored)
}

fn validate_secret(secret: &[u8]) -> Result<(), VaultError> {
    if secret.is_empty() || secret.len() > MAX_SECRET_BYTES {
        Err(VaultError::InvalidOperation(format!(
            "credential secret must contain 1 to {MAX_SECRET_BYTES} bytes"
        )))
    } else {
        Ok(())
    }
}

fn valid_field(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum
}

fn valid_slot(ciphertext_hex: &str, nonce_hex: &str) -> bool {
    !ciphertext_hex.is_empty()
        && ciphertext_hex.len() % 2 == 0
        && nonce_hex.len() == 24
        && ciphertext_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        && nonce_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_optional_slot(ciphertext: Option<&str>, nonce: Option<&str>) -> bool {
    match (ciphertext, nonce) {
        (None, None) => true,
        (Some(ciphertext), Some(nonce)) => valid_slot(ciphertext, nonce),
        _ => false,
    }
}

fn record_summary(record: &StoredCredentialRecord) -> Option<CredentialSummary> {
    Some(CredentialSummary {
        profile_id: record.profile_id.clone(),
        display_name: record.display_name.clone(),
        provider: record.provider.clone(),
        credential_type: record.credential_type.clone(),
        account_fingerprint: record.account_fingerprint.clone(),
        created_at_ms: record.created_at_ms,
        last_validated_at_ms: record.last_validated_at_ms,
        status: CredentialStatus::try_from(record.status).ok()?,
        has_staged_credential: record.staged_payload_hex.is_some(),
        has_rollback_credential: record.previous_payload_hex.is_some(),
    })
}

fn wrapped_dek_aad(host_id: &str) -> Vec<u8> {
    context_aad(WRAPPED_DEK_AAD_DOMAIN, &[host_id])
}

fn payload_aad(host_id: &str) -> Vec<u8> {
    context_aad(PAYLOAD_AAD_DOMAIN, &[host_id])
}

fn record_aad(
    host_id: &str,
    profile_id: &str,
    provider: &str,
    credential_type: &str,
) -> Vec<u8> {
    context_aad(
        RECORD_AAD_DOMAIN,
        &[host_id, profile_id, provider, credential_type],
    )
}

fn context_aad(domain: &[u8], fields: &[&str]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(
        domain.len() + fields.iter().map(|field| 8 + field.len()).sum::<usize>(),
    );
    aad.extend_from_slice(domain);
    aad.extend_from_slice(&VAULT_FORMAT_VERSION.to_be_bytes());
    for field in fields {
        aad.extend_from_slice(&(field.len() as u64).to_be_bytes());
        aad.extend_from_slice(field.as_bytes());
    }
    aad
}

fn migrate_legacy_slot(
    kek: &KeyEncryptionKey,
    dek: &DataEncryptionKey,
    aad: &[u8],
    profile_id: &str,
    ciphertext: Option<&str>,
    nonce: Option<&str>,
) -> Result<(Option<String>, Option<String>), VaultError> {
    match (ciphertext, nonce) {
        (None, None) => Ok((None, None)),
        (Some(ciphertext), Some(nonce)) => {
            let plaintext = decrypt(&kek.key, ciphertext, nonce, profile_id.as_bytes())?;
            let (ciphertext, nonce) = encrypt(&dek.key, plaintext.expose_secret(), aad)?;
            Ok((Some(ciphertext), Some(nonce)))
        }
        _ => Err(VaultError::DecryptionFailed),
    }
}

fn encrypt(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<(String, String), VaultError> {
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let cipher =
        ChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::EncryptionFailed)?;
    let ciphertext = cipher
        .encrypt(&nonce.into(), Payload { msg: plaintext, aad })
        .map_err(|_| VaultError::EncryptionFailed)?;
    Ok((hex::encode(ciphertext), hex::encode(nonce)))
}

fn decrypt(
    key: &[u8; 32],
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
    let cipher =
        ChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::DecryptionFailed)?;
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

fn atomic_write(path: &Path, data: &[u8]) -> Result<(), VaultError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| VaultError::Io(error.to_string()))?;

    let mut options = atomic_write_file::OpenOptions::new();
    #[cfg(unix)]
    {
        use atomic_write_file::unix::OpenOptionsExt as AtomicOpenOptionsExt;
        use std::os::unix::fs::OpenOptionsExt;
        AtomicOpenOptionsExt::preserve_mode(&mut options, false);
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| VaultError::Io(error.to_string()))?;
    file.write_all(data)
        .map_err(|error| VaultError::Io(error.to_string()))?;
    file.commit()
        .map_err(|error| VaultError::Io(error.to_string()))
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
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ))
    }

    fn kek(passphrase: &[u8]) -> KeyEncryptionKey {
        KeyEncryptionKey::derive_from_passphrase(passphrase, &[9_u8; 16]).unwrap()
    }

    fn enrollment(profile_id: &str, provider: &str) -> CredentialEnrollment {
        CredentialEnrollment {
            profile_id: profile_id.into(),
            display_name: format!("Profile {profile_id}"),
            provider: provider.into(),
            credential_type: "api_key".into(),
            account_fingerprint: format!("fingerprint-{profile_id}"),
            created_at_ms: 1000,
            last_validated_at_ms: 1000,
        }
    }

    #[test]
    fn v2_vault_lifecycle_reopens_without_plaintext_metadata() {
        let path = unique_path("vault-v2");
        {
            let mut vault =
                PersistentVault::open_or_create(&path, "host-1", kek(b"passphrase")).unwrap();
            vault
                .enroll_credential(
                    enrollment("profile-1", "opencode_go"),
                    b"secret-v1",
                )
                .unwrap();
            vault.stage_credential("profile-1", b"secret-v2").unwrap();
            assert!(matches!(
                vault.activate_credential("profile-1"),
                Err(VaultError::InvalidOperation(_))
            ));
            assert_eq!(
                vault
                    .decrypt_active_secret("profile-1")
                    .unwrap()
                    .expose_secret(),
                b"secret-v1"
            );
            vault.mark_staged_validated("profile-1", 2000).unwrap();
            vault.activate_credential("profile-1").unwrap();
            assert_eq!(
                vault
                    .decrypt_active_secret("profile-1")
                    .unwrap()
                    .expose_secret(),
                b"secret-v2"
            );
            let summary = vault.get_profile("profile-1").unwrap();
            assert_eq!(summary.status, CredentialStatus::Active);
            assert!(summary.has_rollback_credential);
            vault.rollback_credential("profile-1").unwrap();
            assert_eq!(
                vault
                    .decrypt_active_secret("profile-1")
                    .unwrap()
                    .expose_secret(),
                b"secret-v1"
            );
            vault
                .enroll_credential(
                    enrollment("profile-discard", "opencode_go"),
                    b"keep-me",
                )
                .unwrap();
            vault
                .stage_credential("profile-discard", b"discard-me")
                .unwrap();
            assert!(matches!(
                vault.stage_credential("profile-discard", b"replace-again"),
                Err(VaultError::InvalidOperation(_))
            ));
            vault
                .discard_staged_credential("profile-discard")
                .unwrap();
            assert_eq!(
                vault
                    .decrypt_active_secret("profile-discard")
                    .unwrap()
                    .expose_secret(),
                b"keep-me"
            );
            assert!(matches!(
                vault.discard_staged_credential("profile-discard"),
                Err(VaultError::InvalidOperation(_))
            ));
            let backup = String::from_utf8(vault.export_encrypted_backup().unwrap()).unwrap();
            assert!(!backup.contains("profile-1"));
            assert!(!backup.contains("secret-v1"));
        }

        let file_text = std::fs::read_to_string(&path).unwrap();
        assert!(file_text.contains("\"version\":2"));
        assert!(!file_text.contains("Profile profile-1"));
        assert!(!file_text.contains("fingerprint-profile-1"));
        assert!(!file_text.contains("secret-v1"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        let reopened =
            PersistentVault::open_or_create(&path, "host-1", kek(b"passphrase")).unwrap();
        assert_eq!(
            reopened
                .decrypt_active_secret("profile-1")
                .unwrap()
                .expose_secret(),
            b"secret-v1"
        );
        assert!(matches!(
            PersistentVault::open_or_create(&path, "host-2", kek(b"passphrase")),
            Err(VaultError::HostContextMismatch)
        ));
        assert!(matches!(
            PersistentVault::open_or_create(&path, "host-1", kek(b"wrong")),
            Err(VaultError::DecryptionFailed)
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn record_ciphertext_cannot_move_between_profiles() {
        let path = unique_path("vault-context");
        let mut vault =
            PersistentVault::open_or_create(&path, "host-1", kek(b"passphrase")).unwrap();
        vault
            .enroll_credential(enrollment("profile-a", "provider"), b"secret-a")
            .unwrap();
        vault
            .enroll_credential(enrollment("profile-b", "provider"), b"secret-b")
            .unwrap();
        let record_b = vault.records.get("profile-b").unwrap().clone();
        let record_a = vault.records.get_mut("profile-a").unwrap();
        record_a.encrypted_payload_hex = record_b.encrypted_payload_hex;
        record_a.nonce_hex = record_b.nonce_hex;

        assert!(matches!(
            vault.decrypt_active_secret("profile-a"),
            Err(VaultError::DecryptionFailed)
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn persistence_failure_rolls_back_enrollment_in_memory() {
        let path = unique_path("vault-write-failure");
        let mut vault =
            PersistentVault::open_or_create(&path, "host-1", kek(b"passphrase")).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();

        assert!(matches!(
            vault.enroll_credential(
                enrollment("profile-1", "provider"),
                b"secret"
            ),
            Err(VaultError::Io(_))
        ));
        assert!(vault.get_profile("profile-1").is_none());
        let _ = std::fs::remove_dir(path);
    }

    #[test]
    fn legacy_v1_file_is_atomically_migrated_to_host_bound_v2() {
        let path = unique_path("vault-migration");
        let legacy_kek = kek(b"passphrase");
        let (active_one, active_one_nonce) =
            encrypt(&legacy_kek.key, b"legacy-active", b"profile-1").unwrap();
        let (staged_one, staged_one_nonce) =
            encrypt(&legacy_kek.key, b"legacy-staged", b"profile-1").unwrap();
        let (active_two, active_two_nonce) =
            encrypt(&legacy_kek.key, b"legacy-current", b"profile-2").unwrap();
        let (previous_two, previous_two_nonce) =
            encrypt(&legacy_kek.key, b"legacy-previous", b"profile-2").unwrap();
        let legacy_payload = LegacyVaultPayload {
            version: LEGACY_VAULT_FORMAT_VERSION,
            records: vec![
                LegacyCredentialRecord {
                    profile_id: "profile-1".into(),
                    display_name: "Legacy Staged Profile".into(),
                    provider: "opencode_go".into(),
                    credential_type: "api_key".into(),
                    account_fingerprint: "legacy-fingerprint-1".into(),
                    created_at_ms: 1000,
                    last_validated_at_ms: 1000,
                    status: CredentialStatus::Staged as i32,
                    encrypted_payload_hex: active_one,
                    nonce_hex: active_one_nonce,
                    staged_payload_hex: Some(staged_one),
                    staged_nonce_hex: Some(staged_one_nonce),
                    staged_validated_at_ms: Some(2000),
                    previous_payload_hex: None,
                    previous_nonce_hex: None,
                    previous_validated_at_ms: None,
                },
                LegacyCredentialRecord {
                    profile_id: "profile-2".into(),
                    display_name: "Legacy Rollback Profile".into(),
                    provider: "opencode_go".into(),
                    credential_type: "api_key".into(),
                    account_fingerprint: "legacy-fingerprint-2".into(),
                    created_at_ms: 1000,
                    last_validated_at_ms: 1500,
                    status: CredentialStatus::Active as i32,
                    encrypted_payload_hex: active_two,
                    nonce_hex: active_two_nonce,
                    staged_payload_hex: None,
                    staged_nonce_hex: None,
                    staged_validated_at_ms: None,
                    previous_payload_hex: Some(previous_two),
                    previous_nonce_hex: Some(previous_two_nonce),
                    previous_validated_at_ms: Some(500),
                },
            ],
        };
        let mut payload_bytes = serde_json::to_vec(&legacy_payload).unwrap();
        let (ciphertext_hex, outer_nonce_hex) = encrypt(
            &legacy_kek.key,
            &payload_bytes,
            LEGACY_VAULT_ENVELOPE_AAD,
        )
        .unwrap();
        payload_bytes.zeroize();
        let legacy_envelope = LegacySealedVaultFile {
            version: LEGACY_VAULT_FORMAT_VERSION,
            nonce_hex: outer_nonce_hex,
            ciphertext_hex,
        };
        std::fs::write(&path, serde_json::to_vec(&legacy_envelope).unwrap()).unwrap();
        drop(legacy_kek);

        let mut migrated =
            PersistentVault::open_or_create(&path, "host-1", kek(b"passphrase")).unwrap();
        assert_eq!(
            migrated
                .decrypt_active_secret("profile-1")
                .unwrap()
                .expose_secret(),
            b"legacy-active"
        );
        migrated.activate_credential("profile-1").unwrap();
        assert_eq!(
            migrated
                .decrypt_active_secret("profile-1")
                .unwrap()
                .expose_secret(),
            b"legacy-staged"
        );
        migrated.rollback_credential("profile-2").unwrap();
        assert_eq!(
            migrated
                .decrypt_active_secret("profile-2")
                .unwrap()
                .expose_secret(),
            b"legacy-previous"
        );
        let migrated_file = std::fs::read_to_string(&path).unwrap();
        assert!(migrated_file.contains("\"version\":2"));
        assert!(!migrated_file.contains("Legacy Profile"));
        assert!(matches!(
            PersistentVault::open_or_create(&path, "host-2", kek(b"passphrase")),
            Err(VaultError::HostContextMismatch)
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_enrollment_and_duplicate_profile_are_rejected() {
        let path = unique_path("vault-validation");
        let mut vault =
            PersistentVault::open_or_create(&path, "host-1", kek(b"passphrase")).unwrap();
        assert!(matches!(
            vault.enroll_credential(enrollment("", "provider"), b"secret"),
            Err(VaultError::InvalidOperation(_))
        ));
        vault
            .enroll_credential(enrollment("profile-1", "provider"), b"secret")
            .unwrap();
        assert!(matches!(
            vault.enroll_credential(
                enrollment("profile-1", "provider"),
                b"different"
            ),
            Err(VaultError::InvalidOperation(_))
        ));
        assert!(matches!(
            vault.stage_credential("profile-1", &vec![0_u8; MAX_SECRET_BYTES + 1]),
            Err(VaultError::InvalidOperation(_))
        ));
        let _ = std::fs::remove_file(path);
    }
}
