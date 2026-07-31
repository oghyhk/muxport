use ed25519_dalek::{SigningKey, VerifyingKey};
use muxport_crypto::{
    CryptoError, DeviceRegistry, PairedDeviceRecord, QrPairingPayload,
    SignedPairingOffer, VerifiedInitiator,
};
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, TransactionBehavior,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Duration;
use subtle::ConstantTimeEq;
use thiserror::Error;

const STATE_OFFERED: i32 = 0;
const STATE_CLAIMED: i32 = 1;
const STATE_FINALIZED: i32 = 2;
const STATE_CANCELLED: i32 = 3;
const STATE_EXPIRED: i32 = 4;
const MAX_PAIRING_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Error, Debug)]
pub enum PairingStoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error("Pairing store integrity check failed: {0}")]
    IntegrityCheckFailed(String),
    #[error("Pairing store schema version {found} is newer than supported version {supported}")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },
    #[error("Pairing offer input is invalid")]
    InvalidOffer,
    #[error("Pairing token is unknown, expired, or already consumed")]
    InvalidOrConsumedToken,
    #[error("Pairing session is unknown or not in the required state")]
    InvalidPairingState,
    #[error("Pairing SAS must contain exactly six digits")]
    InvalidSas,
    #[error("Pairing SAS confirmation did not match")]
    SasMismatch,
    #[error("Pairing requires confirmation from both phone and host")]
    ConfirmationIncomplete,
    #[error("Persisted host identity does not match the protected signing key")]
    HostIdentityMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmingParty {
    Phone,
    Host,
}

pub struct PairingStore {
    conn: Connection,
}

impl PairingStore {
    const SCHEMA_VERSION: u32 = 1;

    pub fn open_sqlite(path: impl AsRef<Path>) -> Result<Self, PairingStoreError> {
        let path = path.as_ref();
        if path.exists() {
            let preflight = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            let integrity: String =
                preflight.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            if integrity != "ok" {
                return Err(PairingStoreError::IntegrityCheckFailed(integrity));
            }
            let schema_version: u32 =
                preflight.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            if schema_version > Self::SCHEMA_VERSION {
                return Err(PairingStoreError::UnsupportedSchemaVersion {
                    found: schema_version,
                    supported: Self::SCHEMA_VERSION,
                });
            }
        }
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, PairingStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self, PairingStoreError> {
        conn.pragma_update(None, "foreign_keys", true)?;
        let schema_version: u32 =
            conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if schema_version > Self::SCHEMA_VERSION {
            return Err(PairingStoreError::UnsupportedSchemaVersion {
                found: schema_version,
                supported: Self::SCHEMA_VERSION,
            });
        }
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        let transaction = conn.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS pairing_sessions (
                pairing_id TEXT PRIMARY KEY,
                host_id TEXT NOT NULL,
                hostname TEXT NOT NULL,
                token_hash BLOB NOT NULL UNIQUE,
                ephemeral_public_key_hex TEXT NOT NULL,
                expires_at_ms INTEGER NOT NULL,
                state_code INTEGER NOT NULL,
                device_id TEXT NOT NULL DEFAULT '',
                device_name TEXT NOT NULL DEFAULT '',
                device_public_key_hex TEXT NOT NULL DEFAULT '',
                sas_hash BLOB NOT NULL DEFAULT X'',
                phone_confirmed INTEGER NOT NULL DEFAULT 0,
                host_confirmed INTEGER NOT NULL DEFAULT 0,
                created_at_ms INTEGER NOT NULL,
                finalized_at_ms INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS signed_device_registry (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                signed_blob BLOB NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS host_identity_pin (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                host_id TEXT NOT NULL,
                public_key_hex TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );",
        )?;
        let now = chrono::Utc::now().timestamp_millis();
        transaction.execute(
            "UPDATE pairing_sessions
             SET state_code = ?1
             WHERE state_code IN (?2, ?3) AND expires_at_ms < ?4",
            params![STATE_EXPIRED, STATE_OFFERED, STATE_CLAIMED, now],
        )?;
        // Ephemeral X25519 secrets exist only in process memory. After a
        // connector restart, unfinished offers and one-sided confirmations
        // cannot safely continue. A fully confirmed record can still be
        // finalized transactionally because it no longer needs the ephemeral
        // secret.
        transaction.execute(
            "UPDATE pairing_sessions
             SET state_code = ?1
             WHERE state_code = ?2
                OR (state_code = ?3
                    AND NOT (phone_confirmed = 1 AND host_confirmed = 1))",
            params![STATE_CANCELLED, STATE_OFFERED, STATE_CLAIMED],
        )?;
        transaction.pragma_update(None, "user_version", Self::SCHEMA_VERSION)?;
        transaction.commit()?;
        Ok(Self { conn })
    }

    /// Pins the OS-protected host identity into non-secret local metadata.
    /// Existing registries are signature-verified before a legacy database can
    /// acquire its first pin, preventing a missing keyring entry from silently
    /// replacing an identity that already paired devices.
    pub fn bind_host_identity(
        &mut self,
        host_id: &str,
        verifying_key: &VerifyingKey,
    ) -> Result<bool, PairingStoreError> {
        if host_id.trim().is_empty() {
            return Err(PairingStoreError::HostIdentityMismatch);
        }
        let public_key_hex = encode_hex(verifying_key.as_bytes());
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT host_id, public_key_hex FROM host_identity_pin
                 WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((pinned_host_id, pinned_public_key)) = existing {
            if pinned_host_id != host_id || pinned_public_key != public_key_hex {
                return Err(PairingStoreError::HostIdentityMismatch);
            }
            return Ok(false);
        }

        let signed_registry: Option<Vec<u8>> = tx
            .query_row(
                "SELECT signed_blob FROM signed_device_registry
                 WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(signed_registry) = signed_registry {
            DeviceRegistry::from_signed_snapshot(
                &signed_registry,
                host_id,
                verifying_key,
            )
            .map_err(|_| PairingStoreError::HostIdentityMismatch)?;
        }

        tx.execute(
            "INSERT INTO host_identity_pin
                (singleton, host_id, public_key_hex, created_at_ms)
             VALUES (1, ?1, ?2, ?3)",
            params![
                host_id,
                public_key_hex,
                chrono::Utc::now().timestamp_millis()
            ],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn create_offer(
        &mut self,
        host_id: &str,
        hostname: &str,
        ephemeral_public_key_hex: &str,
        ttl: Duration,
    ) -> Result<QrPairingPayload, PairingStoreError> {
        if host_id.trim().is_empty()
            || host_id.len() > 256
            || hostname.trim().is_empty()
            || hostname.len() > 256
            || !valid_hex_32(ephemeral_public_key_hex)
            || ttl.is_zero()
            || ttl > MAX_PAIRING_TTL
        {
            return Err(PairingStoreError::InvalidOffer);
        }
        let ttl_ms: i64 = ttl
            .as_millis()
            .try_into()
            .map_err(|_| PairingStoreError::InvalidOffer)?;
        let now = chrono::Utc::now().timestamp_millis();
        let expires_at_ms = now
            .checked_add(ttl_ms)
            .ok_or(PairingStoreError::InvalidOffer)?;
        let pairing_id = uuid::Uuid::new_v4().to_string();
        let mut token_bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut token_bytes);
        let rendezvous_token = encode_hex(&token_bytes);
        let token_hash = token_hash(&rendezvous_token);
        self.conn.execute(
            "INSERT INTO pairing_sessions (
                pairing_id, host_id, hostname, token_hash,
                ephemeral_public_key_hex, expires_at_ms, state_code,
                created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                pairing_id,
                host_id,
                hostname,
                token_hash.as_slice(),
                ephemeral_public_key_hex,
                expires_at_ms,
                STATE_OFFERED,
                now
            ],
        )?;
        Ok(QrPairingPayload {
            host_id: host_id.to_owned(),
            hostname: hostname.to_owned(),
            rendezvous_token,
            ephemeral_pubkey_hex: ephemeral_public_key_hex.to_owned(),
            expires_at_ms,
        })
    }

    /// Creates the self-verifying payload that may be encoded into the host's
    /// pairing QR. The identity carried by this offer is only persisted as a
    /// phone-side pin after both devices compare and confirm the derived SAS.
    pub fn create_signed_offer(
        &mut self,
        host_id: &str,
        hostname: &str,
        direct_endpoint: &str,
        ephemeral_public_key_hex: &str,
        ttl: Duration,
        host_identity: &SigningKey,
    ) -> Result<SignedPairingOffer, PairingStoreError> {
        let endpoint: std::net::SocketAddr = direct_endpoint
            .parse()
            .map_err(|_| PairingStoreError::InvalidOffer)?;
        if direct_endpoint.len() > 512
            || endpoint.ip().is_unspecified()
            || endpoint.port() == 0
        {
            return Err(PairingStoreError::InvalidOffer);
        }
        let payload = self.create_offer(
            host_id,
            hostname,
            ephemeral_public_key_hex,
            ttl,
        )?;
        let ttl_ms: i64 = ttl
            .as_millis()
            .try_into()
            .map_err(|_| PairingStoreError::InvalidOffer)?;
        let issued_at_ms = payload
            .expires_at_ms
            .checked_sub(ttl_ms)
            .ok_or(PairingStoreError::InvalidOffer)?;
        Ok(SignedPairingOffer::create(
            host_identity,
            &payload.host_id,
            &payload.hostname,
            direct_endpoint,
            &payload.rendezvous_token,
            &payload.ephemeral_pubkey_hex,
            issued_at_ms,
            payload.expires_at_ms,
        )?)
    }

    /// Claims a token only after the caller has produced a
    /// `VerifiedInitiator` from the signed pairing hello.
    pub fn claim_verified_initiator(
        &mut self,
        rendezvous_token: &str,
        initiator: &VerifiedInitiator,
        device_name: &str,
        sas: &str,
    ) -> Result<String, PairingStoreError> {
        validate_sas(sas)?;
        if device_name.trim().is_empty() || device_name.len() > 128 {
            return Err(PairingStoreError::InvalidOffer);
        }
        let now = chrono::Utc::now().timestamp_millis();
        let token_hash = token_hash(rendezvous_token);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = tx
            .query_row(
                "SELECT pairing_id, host_id, expires_at_ms, state_code
                 FROM pairing_sessions WHERE token_hash = ?1",
                params![token_hash.as_slice()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i32>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((pairing_id, host_id, expires_at_ms, state)) = row else {
            return Err(PairingStoreError::InvalidOrConsumedToken);
        };
        if expires_at_ms < now {
            tx.execute(
                "UPDATE pairing_sessions SET state_code = ?1
                 WHERE pairing_id = ?2",
                params![STATE_EXPIRED, pairing_id],
            )?;
            tx.commit()?;
            return Err(PairingStoreError::InvalidOrConsumedToken);
        }
        if state != STATE_OFFERED || host_id != initiator.host_id() {
            return Err(PairingStoreError::InvalidOrConsumedToken);
        }
        let sas_hash = sas_hash(&pairing_id, sas);
        let changed = tx.execute(
            "UPDATE pairing_sessions
             SET state_code = ?1, device_id = ?2, device_name = ?3,
                 device_public_key_hex = ?4, sas_hash = ?5
             WHERE pairing_id = ?6 AND state_code = ?7",
            params![
                STATE_CLAIMED,
                initiator.device_id(),
                device_name,
                initiator.device_identity_public_key_hex(),
                sas_hash.as_slice(),
                pairing_id,
                STATE_OFFERED
            ],
        )?;
        if changed != 1 {
            return Err(PairingStoreError::InvalidOrConsumedToken);
        }
        tx.commit()?;
        Ok(pairing_id)
    }

    /// Records one side's SAS confirmation. Duplicate confirmation by the
    /// same side is idempotent.
    pub fn confirm_sas(
        &mut self,
        pairing_id: &str,
        sas: &str,
        party: ConfirmingParty,
    ) -> Result<bool, PairingStoreError> {
        validate_sas(sas)?;
        let now = chrono::Utc::now().timestamp_millis();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = tx
            .query_row(
                "SELECT expires_at_ms, state_code, sas_hash,
                        phone_confirmed, host_confirmed
                 FROM pairing_sessions WHERE pairing_id = ?1",
                params![pairing_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, bool>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            expires_at_ms,
            state,
            expected_sas_hash,
            mut phone_confirmed,
            mut host_confirmed,
        )) = row
        else {
            return Err(PairingStoreError::InvalidPairingState);
        };
        if expires_at_ms < now {
            tx.execute(
                "UPDATE pairing_sessions SET state_code = ?1
                 WHERE pairing_id = ?2",
                params![STATE_EXPIRED, pairing_id],
            )?;
            tx.commit()?;
            return Err(PairingStoreError::InvalidOrConsumedToken);
        }
        if state != STATE_CLAIMED {
            return Err(PairingStoreError::InvalidPairingState);
        }
        let actual_sas_hash = sas_hash(pairing_id, sas);
        if expected_sas_hash
            .as_slice()
            .ct_eq(actual_sas_hash.as_slice())
            .unwrap_u8()
            != 1
        {
            return Err(PairingStoreError::SasMismatch);
        }
        match party {
            ConfirmingParty::Phone => phone_confirmed = true,
            ConfirmingParty::Host => host_confirmed = true,
        }
        tx.execute(
            "UPDATE pairing_sessions
             SET phone_confirmed = ?1, host_confirmed = ?2
             WHERE pairing_id = ?3 AND state_code = ?4",
            params![
                phone_confirmed,
                host_confirmed,
                pairing_id,
                STATE_CLAIMED
            ],
        )?;
        tx.commit()?;
        Ok(phone_confirmed && host_confirmed)
    }

    /// Atomically updates the signed registry and final pairing state in one
    /// SQLite transaction. The host private key is supplied by the caller and
    /// is never stored in this database.
    pub fn finalize(
        &mut self,
        pairing_id: &str,
        expected_host_id: &str,
        host_identity: &SigningKey,
    ) -> Result<PairedDeviceRecord, PairingStoreError> {
        let now = chrono::Utc::now().timestamp_millis();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = tx
            .query_row(
                "SELECT host_id, expires_at_ms, state_code, device_id,
                        device_name, device_public_key_hex,
                        phone_confirmed, host_confirmed
                 FROM pairing_sessions WHERE pairing_id = ?1",
                params![pairing_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i32>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, bool>(6)?,
                        row.get::<_, bool>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            host_id,
            expires_at_ms,
            state,
            device_id,
            device_name,
            public_key_hex,
            phone_confirmed,
            host_confirmed,
        )) = row
        else {
            return Err(PairingStoreError::InvalidPairingState);
        };
        if host_id != expected_host_id {
            return Err(PairingStoreError::InvalidPairingState);
        }
        if state == STATE_FINALIZED {
            let registry =
                load_registry_tx(&tx, expected_host_id, host_identity)?;
            return registry
                .list_devices()
                .into_iter()
                .find(|device| device.device_id == device_id)
                .ok_or(PairingStoreError::InvalidPairingState);
        }
        if expires_at_ms < now {
            tx.execute(
                "UPDATE pairing_sessions SET state_code = ?1
                 WHERE pairing_id = ?2",
                params![STATE_EXPIRED, pairing_id],
            )?;
            tx.commit()?;
            return Err(PairingStoreError::InvalidPairingState);
        }
        if state != STATE_CLAIMED {
            return Err(PairingStoreError::InvalidPairingState);
        }
        if !phone_confirmed || !host_confirmed {
            return Err(PairingStoreError::ConfirmationIncomplete);
        }

        let record = PairedDeviceRecord {
            device_id,
            device_name,
            public_key_hex,
            paired_at_ms: now,
            last_seen_at_ms: now,
            is_revoked: false,
        };
        let mut registry =
            load_registry_tx(&tx, expected_host_id, host_identity)?;
        registry.register_device(record.clone())?;
        let signed = registry.signed_snapshot(expected_host_id, host_identity)?;
        tx.execute(
            "INSERT INTO signed_device_registry
                (singleton, signed_blob, updated_at_ms)
             VALUES (1, ?1, ?2)
             ON CONFLICT(singleton) DO UPDATE SET
                signed_blob = excluded.signed_blob,
                updated_at_ms = excluded.updated_at_ms",
            params![signed, now],
        )?;
        let changed = tx.execute(
            "UPDATE pairing_sessions
             SET state_code = ?1, finalized_at_ms = ?2
             WHERE pairing_id = ?3 AND state_code = ?4",
            params![
                STATE_FINALIZED,
                now,
                pairing_id,
                STATE_CLAIMED
            ],
        )?;
        if changed != 1 {
            return Err(PairingStoreError::InvalidPairingState);
        }
        tx.commit()?;
        Ok(record)
    }

    pub fn load_registry(
        &self,
        expected_host_id: &str,
        host_identity: &SigningKey,
    ) -> Result<DeviceRegistry, PairingStoreError> {
        load_registry_connection(
            &self.conn,
            expected_host_id,
            host_identity,
        )
    }

    /// Replaces the host-signed registry in one transaction. This does not
    /// alter provider credentials or the OS-protected host identity.
    pub fn revoke_device(
        &mut self,
        expected_host_id: &str,
        host_identity: &SigningKey,
        device_id: &str,
    ) -> Result<bool, PairingStoreError> {
        if expected_host_id.trim().is_empty()
            || device_id.trim().is_empty()
            || device_id.len() > 256
        {
            return Err(PairingStoreError::InvalidPairingState);
        }
        let now = chrono::Utc::now().timestamp_millis();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut registry = load_registry_tx(&tx, expected_host_id, host_identity)?;
        let is_active = registry
            .list_devices()
            .into_iter()
            .find(|device| device.device_id == device_id)
            .is_some_and(|device| !device.is_revoked);
        if !is_active {
            tx.commit()?;
            return Ok(false);
        }
        debug_assert!(registry.revoke_device(device_id));
        let signed = registry.signed_snapshot(expected_host_id, host_identity)?;
        tx.execute(
            "INSERT INTO signed_device_registry
                (singleton, signed_blob, updated_at_ms)
             VALUES (1, ?1, ?2)
             ON CONFLICT(singleton) DO UPDATE SET
                signed_blob = excluded.signed_blob,
                updated_at_ms = excluded.updated_at_ms",
            params![signed, now],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn cancel(&mut self, pairing_id: &str) -> Result<bool, PairingStoreError> {
        let changed = self.conn.execute(
            "UPDATE pairing_sessions SET state_code = ?1
             WHERE pairing_id = ?2 AND state_code IN (?3, ?4)",
            params![
                STATE_CANCELLED,
                pairing_id,
                STATE_OFFERED,
                STATE_CLAIMED
            ],
        )?;
        Ok(changed == 1)
    }

    pub fn cancel_by_token(
        &mut self,
        rendezvous_token: &str,
    ) -> Result<bool, PairingStoreError> {
        let hash = token_hash(rendezvous_token);
        let changed = self.conn.execute(
            "UPDATE pairing_sessions SET state_code = ?1
             WHERE token_hash = ?2 AND state_code IN (?3, ?4)",
            params![
                STATE_CANCELLED,
                hash.as_slice(),
                STATE_OFFERED,
                STATE_CLAIMED
            ],
        )?;
        Ok(changed == 1)
    }
}

fn load_registry_tx(
    tx: &rusqlite::Transaction<'_>,
    expected_host_id: &str,
    host_identity: &SigningKey,
) -> Result<DeviceRegistry, PairingStoreError> {
    load_registry_connection(tx, expected_host_id, host_identity)
}

fn load_registry_connection(
    conn: &Connection,
    expected_host_id: &str,
    host_identity: &SigningKey,
) -> Result<DeviceRegistry, PairingStoreError> {
    let signed: Option<Vec<u8>> = conn
        .query_row(
            "SELECT signed_blob FROM signed_device_registry
             WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match signed {
        Some(signed) => Ok(DeviceRegistry::from_signed_snapshot(
            &signed,
            expected_host_id,
            &host_identity.verifying_key(),
        )?),
        None => Ok(DeviceRegistry::new()),
    }
}

fn validate_sas(sas: &str) -> Result<(), PairingStoreError> {
    if sas.len() == 6 && sas.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(())
    } else {
        Err(PairingStoreError::InvalidSas)
    }
}

fn token_hash(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"muxport-pairing-token-v1");
    hasher.update((token.len() as u64).to_be_bytes());
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

fn sas_hash(pairing_id: &str, sas: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"muxport-pairing-sas-v1");
    hasher.update((pairing_id.len() as u64).to_be_bytes());
    hasher.update(pairing_id.as_bytes());
    hasher.update(sas.as_bytes());
    hasher.finalize().into()
}

fn valid_hex_32(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| {
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
        })
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
    use ed25519_dalek::SigningKey;
    use muxport_crypto::{
        create_initiator_hello, verify_pairing_initiator, KeyPair,
    };

    fn claim_pairing(
        store: &mut PairingStore,
        host_id: &str,
        device_identity: &SigningKey,
    ) -> (String, String) {
        let host_ephemeral = KeyPair::generate();
        let device_ephemeral = KeyPair::generate();
        let offer = store
            .create_offer(
                host_id,
                "Test host",
                &encode_hex(host_ephemeral.public.as_bytes()),
                Duration::from_secs(60),
            )
            .unwrap();
        let hello = create_initiator_hello(
            device_identity,
            "phone-1",
            host_id,
            &offer.rendezvous_token,
            &device_ephemeral.public,
        )
        .unwrap();
        let verified = verify_pairing_initiator(
            &hello,
            host_id,
            &offer.rendezvous_token,
        )
        .unwrap();
        let pairing_id = store
            .claim_verified_initiator(
                &offer.rendezvous_token,
                &verified,
                "Phone",
                "123456",
            )
            .unwrap();
        (pairing_id, offer.rendezvous_token)
    }

    #[test]
    fn token_is_single_use_and_both_sas_confirmations_are_required() {
        let mut store = PairingStore::open_in_memory().unwrap();
        let host_identity = SigningKey::generate(&mut OsRng);
        let device_identity = SigningKey::generate(&mut OsRng);
        let (pairing_id, token) =
            claim_pairing(&mut store, "host-1", &device_identity);

        let replay_ephemeral = KeyPair::generate();
        let replay = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            &token,
            &replay_ephemeral.public,
        )
        .unwrap();
        let replay =
            verify_pairing_initiator(&replay, "host-1", &token).unwrap();
        assert!(matches!(
            store.claim_verified_initiator(
                &token,
                &replay,
                "Phone",
                "123456"
            ),
            Err(PairingStoreError::InvalidOrConsumedToken)
        ));
        assert!(matches!(
            store.confirm_sas(
                &pairing_id,
                "654321",
                ConfirmingParty::Phone
            ),
            Err(PairingStoreError::SasMismatch)
        ));
        assert!(!store
            .confirm_sas(&pairing_id, "123456", ConfirmingParty::Phone)
            .unwrap());
        assert!(matches!(
            store.finalize(&pairing_id, "host-1", &host_identity),
            Err(PairingStoreError::ConfirmationIncomplete)
        ));
        assert!(store
            .confirm_sas(&pairing_id, "123456", ConfirmingParty::Host)
            .unwrap());
        let record = store
            .finalize(&pairing_id, "host-1", &host_identity)
            .unwrap();
        assert_eq!(record.device_id, "phone-1");
        assert!(store
            .load_registry("host-1", &host_identity)
            .unwrap()
            .is_authorized("phone-1", &record.public_key_hex));
        assert_eq!(
            store
                .finalize(&pairing_id, "host-1", &host_identity)
                .unwrap()
                .device_id,
            "phone-1"
        );
    }

    #[test]
    fn signed_offer_carries_verifiable_host_pin_and_reachable_endpoint() {
        let mut store = PairingStore::open_in_memory().unwrap();
        let host_identity = SigningKey::generate(&mut OsRng);
        let host_ephemeral = KeyPair::generate();
        let offer = store
            .create_signed_offer(
                "host-1",
                "Developer workstation",
                "192.0.2.10:45821",
                &encode_hex(host_ephemeral.public.as_bytes()),
                Duration::from_secs(60),
                &host_identity,
            )
            .unwrap();
        assert_eq!(offer.host_id, "host-1");
        assert_eq!(offer.direct_endpoint, "192.0.2.10:45821");
        assert_eq!(
            offer
                .verify(chrono::Utc::now().timestamp_millis())
                .unwrap(),
            host_identity.verifying_key()
        );
        assert!(matches!(
            store.create_signed_offer(
                "host-1",
                "Developer workstation",
                "0.0.0.0:45821",
                &encode_hex(host_ephemeral.public.as_bytes()),
                Duration::from_secs(60),
                &host_identity,
            ),
            Err(PairingStoreError::InvalidOffer)
        ));
    }

    #[test]
    fn confirmed_pairing_finalizes_after_store_restart() {
        let db_path = std::env::temp_dir().join(format!(
            "muxport-pairing-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let host_identity = SigningKey::generate(&mut OsRng);
        let device_identity = SigningKey::generate(&mut OsRng);
        let pairing_id;
        {
            let mut store = PairingStore::open_sqlite(&db_path).unwrap();
            assert_eq!(
                store
                    .conn
                    .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                    .unwrap(),
                PairingStore::SCHEMA_VERSION
            );
            (pairing_id, _) =
                claim_pairing(&mut store, "host-1", &device_identity);
            store
                .confirm_sas(
                    &pairing_id,
                    "123456",
                    ConfirmingParty::Phone,
                )
                .unwrap();
            store
                .confirm_sas(
                    &pairing_id,
                    "123456",
                    ConfirmingParty::Host,
                )
                .unwrap();
        }
        {
            let mut store = PairingStore::open_sqlite(&db_path).unwrap();
            let record = store
                .finalize(&pairing_id, "host-1", &host_identity)
                .unwrap();
            assert!(store
                .load_registry("host-1", &host_identity)
                .unwrap()
                .is_authorized("phone-1", &record.public_key_hex));
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn future_schema_is_rejected_without_modification() {
        let db_path = std::env::temp_dir().join(format!(
            "muxport-pairing-future-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let connection = Connection::open(&db_path).unwrap();
            connection
                .execute_batch(
                    "PRAGMA user_version = 999;
                     CREATE TABLE future_marker (value TEXT NOT NULL);
                     INSERT INTO future_marker VALUES ('preserve-me');",
                )
                .unwrap();
        }

        assert!(matches!(
            PairingStore::open_sqlite(&db_path),
            Err(PairingStoreError::UnsupportedSchemaVersion {
                found: 999,
                supported: 1
            })
        ));
        let connection = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        assert_eq!(
            connection
                .query_row("SELECT value FROM future_marker", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "preserve-me"
        );
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            999
        );

        drop(connection);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn restart_cancels_offer_without_both_confirmations() {
        let db_path = std::env::temp_dir().join(format!(
            "muxport-pairing-cancel-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let device_identity = SigningKey::generate(&mut OsRng);
        let offer;
        {
            let mut store = PairingStore::open_sqlite(&db_path).unwrap();
            let host_ephemeral = KeyPair::generate();
            offer = store
                .create_offer(
                    "host-1",
                    "Host",
                    &encode_hex(host_ephemeral.public.as_bytes()),
                    Duration::from_secs(60),
                )
                .unwrap();
        }
        let device_ephemeral = KeyPair::generate();
        let hello = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            &offer.rendezvous_token,
            &device_ephemeral.public,
        )
        .unwrap();
        let verified = verify_pairing_initiator(
            &hello,
            "host-1",
            &offer.rendezvous_token,
        )
        .unwrap();
        {
            let mut store = PairingStore::open_sqlite(&db_path).unwrap();
            assert!(matches!(
                store.claim_verified_initiator(
                    &offer.rendezvous_token,
                    &verified,
                    "Phone",
                    "123456"
                ),
                Err(PairingStoreError::InvalidOrConsumedToken)
            ));
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn cancelled_and_expired_offers_cannot_be_claimed() {
        let mut store = PairingStore::open_in_memory().unwrap();
        let device_identity = SigningKey::generate(&mut OsRng);
        let host_ephemeral = KeyPair::generate();
        let device_ephemeral = KeyPair::generate();
        let offer = store
            .create_offer(
                "host-1",
                "Host",
                &encode_hex(host_ephemeral.public.as_bytes()),
                Duration::from_secs(60),
            )
            .unwrap();
        let pairing_id: String = store
            .conn
            .query_row(
                "SELECT pairing_id FROM pairing_sessions
                 WHERE token_hash = ?1",
                params![token_hash(&offer.rendezvous_token).as_slice()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(store.cancel(&pairing_id).unwrap());
        let hello = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            &offer.rendezvous_token,
            &device_ephemeral.public,
        )
        .unwrap();
        let verified = verify_pairing_initiator(
            &hello,
            "host-1",
            &offer.rendezvous_token,
        )
        .unwrap();
        assert!(matches!(
            store.claim_verified_initiator(
                &offer.rendezvous_token,
                &verified,
                "Phone",
                "123456"
            ),
            Err(PairingStoreError::InvalidOrConsumedToken)
        ));

        let expired_host_ephemeral = KeyPair::generate();
        let expired_device_ephemeral = KeyPair::generate();
        let expired = store
            .create_offer(
                "host-1",
                "Host",
                &encode_hex(expired_host_ephemeral.public.as_bytes()),
                Duration::from_secs(60),
            )
            .unwrap();
        store
            .conn
            .execute(
                "UPDATE pairing_sessions SET expires_at_ms = 1
                 WHERE token_hash = ?1",
                params![token_hash(&expired.rendezvous_token).as_slice()],
            )
            .unwrap();
        let expired_hello = create_initiator_hello(
            &device_identity,
            "phone-2",
            "host-1",
            &expired.rendezvous_token,
            &expired_device_ephemeral.public,
        )
        .unwrap();
        let expired_verified = verify_pairing_initiator(
            &expired_hello,
            "host-1",
            &expired.rendezvous_token,
        )
        .unwrap();
        assert!(matches!(
            store.claim_verified_initiator(
                &expired.rendezvous_token,
                &expired_verified,
                "Phone 2",
                "123456"
            ),
            Err(PairingStoreError::InvalidOrConsumedToken)
        ));
    }

    #[test]
    fn host_identity_pin_rejects_replacement_or_host_change() {
        let mut store = PairingStore::open_in_memory().unwrap();
        let first = SigningKey::generate(&mut OsRng);
        let replacement = SigningKey::generate(&mut OsRng);

        assert!(store
            .bind_host_identity("host-1", &first.verifying_key())
            .unwrap());
        assert!(!store
            .bind_host_identity("host-1", &first.verifying_key())
            .unwrap());
        assert!(matches!(
            store.bind_host_identity("host-1", &replacement.verifying_key()),
            Err(PairingStoreError::HostIdentityMismatch)
        ));
        assert!(matches!(
            store.bind_host_identity("host-2", &first.verifying_key()),
            Err(PairingStoreError::HostIdentityMismatch)
        ));
    }

    #[test]
    fn device_revocation_is_signed_durable_and_rejected_after_restart() {
        let db_path = std::env::temp_dir().join(format!(
            "muxport-pairing-revoke-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let host_identity = SigningKey::generate(&mut OsRng);
        let device_identity = SigningKey::generate(&mut OsRng);
        {
            let mut store = PairingStore::open_sqlite(&db_path).unwrap();
            let (pairing_id, _) = claim_pairing(&mut store, "host-1", &device_identity);
            store
                .confirm_sas(&pairing_id, "123456", ConfirmingParty::Phone)
                .unwrap();
            store
                .confirm_sas(&pairing_id, "123456", ConfirmingParty::Host)
                .unwrap();
            let device = store
                .finalize(&pairing_id, "host-1", &host_identity)
                .unwrap();
            assert!(store
                .load_registry("host-1", &host_identity)
                .unwrap()
                .is_authorized(&device.device_id, &device.public_key_hex));
            assert!(store
                .revoke_device("host-1", &host_identity, &device.device_id)
                .unwrap());
            assert!(!store
                .revoke_device("host-1", &host_identity, &device.device_id)
                .unwrap());
        }
        {
            let store = PairingStore::open_sqlite(&db_path).unwrap();
            let registry = store.load_registry("host-1", &host_identity).unwrap();
            let device = registry.list_devices().into_iter().next().unwrap();
            assert!(device.is_revoked);
            assert!(!registry.is_authorized(&device.device_id, &device.public_key_hex));
        }
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn legacy_signed_registry_must_verify_before_first_identity_pin() {
        let mut store = PairingStore::open_in_memory().unwrap();
        let host_identity = SigningKey::generate(&mut OsRng);
        let replacement = SigningKey::generate(&mut OsRng);
        let device_identity = SigningKey::generate(&mut OsRng);
        let (pairing_id, _) =
            claim_pairing(&mut store, "host-1", &device_identity);
        store
            .confirm_sas(&pairing_id, "123456", ConfirmingParty::Phone)
            .unwrap();
        store
            .confirm_sas(&pairing_id, "123456", ConfirmingParty::Host)
            .unwrap();
        store
            .finalize(&pairing_id, "host-1", &host_identity)
            .unwrap();

        assert!(matches!(
            store.bind_host_identity("host-1", &replacement.verifying_key()),
            Err(PairingStoreError::HostIdentityMismatch)
        ));
        assert!(store
            .bind_host_identity("host-1", &host_identity.verifying_key())
            .unwrap());
    }
}
