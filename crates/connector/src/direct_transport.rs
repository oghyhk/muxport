use crate::{
    CommandDispatchError, CommandRouter, ConfirmingParty,
    PairingCoordinator, PairingCoordinatorError, SecureEnvelopeSession,
    SecureSessionError, DEFAULT_MAX_PLAINTEXT_BYTES,
};
use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use event_journal::{EventJournal, JournalError};
use muxport_crypto::{
    authenticated_transcript, create_responder_hello, derive_session_keys,
    derive_shared_secret, pairing_connection_challenge,
    verify_authorized_initiator, CryptoError, DeviceRegistry, EncryptedFrame,
    InitiatorHello, KeyPair, ResponderHello, SessionCipher, SessionRole,
    SecretProvisioningKey, HANDSHAKE_PROTOCOL_VERSION,
};
use muxport_protocol::{muxport_envelope, Ack};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::{debug, info, warn};
use zeroize::Zeroize;

const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;
const MAX_ENCRYPTED_RECORD_BYTES: usize =
    DEFAULT_MAX_PLAINTEXT_BYTES + 16 + 16;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const PAIRING_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const CHALLENGE_CLOCK_SKEW: Duration = Duration::from_secs(60);
const DEFAULT_MAX_CONNECTIONS: usize = 32;

#[derive(Error, Debug)]
pub enum DirectTransportError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    SecureSession(#[from] SecureSessionError),
    #[error(transparent)]
    CommandDispatch(#[from] CommandDispatchError),
    #[error("direct transport handshake timed out")]
    HandshakeTimeout,
    #[error("direct transport record exceeded its configured size")]
    RecordTooLarge,
    #[error("direct transport configuration is invalid")]
    InvalidConfiguration,
    #[error("server challenge signature or context is invalid")]
    InvalidServerChallenge,
    #[error(transparent)]
    Pairing(#[from] PairingCoordinatorError),
    #[error("pairing coordinator lock is unavailable")]
    PairingLockUnavailable,
    #[error("pairing request or confirmation is invalid")]
    InvalidPairingRequest,
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error("sync transport has no configured event journal")]
    SyncUnavailable,
    #[error("sync transport received an invalid acknowledgement")]
    InvalidSyncAcknowledgement,
    #[error("sync recovery requires a persisted host snapshot")]
    MissingSyncSnapshot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerChallenge {
    pub protocol_version: u32,
    pub host_id: String,
    pub host_identity_public_key_hex: String,
    pub boot_epoch: u64,
    pub challenge: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub signature_hex: String,
}

impl ServerChallenge {
    fn create(
        host_id: &str,
        boot_epoch: u64,
        challenge: String,
        host_identity: &SigningKey,
    ) -> Result<Self, DirectTransportError> {
        let issued_at_ms = chrono::Utc::now().timestamp_millis();
        let expires_at_ms = issued_at_ms
            .checked_add(HANDSHAKE_TIMEOUT.as_millis() as i64)
            .ok_or(DirectTransportError::InvalidServerChallenge)?;
        let mut value = Self {
            protocol_version: HANDSHAKE_PROTOCOL_VERSION,
            host_id: host_id.to_owned(),
            host_identity_public_key_hex: encode_hex(
                host_identity.verifying_key().as_bytes(),
            ),
            boot_epoch,
            challenge,
            issued_at_ms,
            expires_at_ms,
            signature_hex: String::new(),
        };
        value.signature_hex =
            encode_hex(&host_identity.sign(&value.signed_bytes()?).to_bytes());
        Ok(value)
    }

    pub fn verify(
        &self,
        expected_host_id: &str,
        expected_host_identity: &VerifyingKey,
        now_ms: i64,
    ) -> Result<(), DirectTransportError> {
        if self.protocol_version != HANDSHAKE_PROTOCOL_VERSION
            || self.host_id != expected_host_id
            || self.host_identity_public_key_hex
                != encode_hex(expected_host_identity.as_bytes())
            || self.boot_epoch == 0
            || self.challenge.len() != 64
            || self.expires_at_ms.checked_sub(self.issued_at_ms)
                != Some(HANDSHAKE_TIMEOUT.as_millis() as i64)
            || now_ms
                < self
                    .issued_at_ms
                    .saturating_sub(CHALLENGE_CLOCK_SKEW.as_millis() as i64)
            || now_ms
                > self
                    .expires_at_ms
                    .saturating_add(CHALLENGE_CLOCK_SKEW.as_millis() as i64)
        {
            return Err(DirectTransportError::InvalidServerChallenge);
        }
        let challenge_bytes = decode_hex::<32>(&self.challenge)?;
        if challenge_bytes.iter().all(|byte| *byte == 0) {
            return Err(DirectTransportError::InvalidServerChallenge);
        }
        let signature_bytes = decode_hex::<64>(&self.signature_hex)?;
        let signature = Signature::from_bytes(&signature_bytes);
        expected_host_identity
            .verify(&self.signed_bytes()?, &signature)
            .map_err(|_| DirectTransportError::InvalidServerChallenge)
    }

    fn signed_bytes(&self) -> Result<Vec<u8>, DirectTransportError> {
        let mut output = b"muxport-server-challenge-v1".to_vec();
        append_signed_field(
            &mut output,
            &self.protocol_version.to_be_bytes(),
        )?;
        append_signed_field(&mut output, self.host_id.as_bytes())?;
        append_signed_field(
            &mut output,
            self.host_identity_public_key_hex.as_bytes(),
        )?;
        append_signed_field(&mut output, &self.boot_epoch.to_be_bytes())?;
        append_signed_field(&mut output, self.challenge.as_bytes())?;
        append_signed_field(&mut output, &self.issued_at_ms.to_be_bytes())?;
        append_signed_field(&mut output, &self.expires_at_ms.to_be_bytes())?;
        Ok(output)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingHandshakeRequest {
    pub kind: String,
    pub protocol_version: u32,
    pub rendezvous_token: String,
    pub device_name: String,
    pub initiator: InitiatorHello,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingHandshakeResponse {
    pub protocol_version: u32,
    pub pairing_id: String,
    pub responder: ResponderHello,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingPhoneConfirmation {
    pub protocol_version: u32,
    pub pairing_id: String,
    pub confirmed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingPhoneAcknowledgement {
    pub protocol_version: u32,
    pub pairing_id: String,
    pub awaiting_host_confirmation: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncHandshakeRequest {
    pub kind: String,
    pub protocol_version: u32,
    pub after_sequence: u64,
    pub known_boot_epoch: u64,
    pub initiator: InitiatorHello,
}

#[derive(Clone)]
pub struct DirectTransportService {
    host_id: String,
    boot_epoch: u64,
    host_identity: Arc<SigningKey>,
    registry: Arc<DeviceRegistry>,
    pairing: Option<Arc<Mutex<PairingCoordinator>>>,
    command_router: Arc<CommandRouter>,
    journal_path: Option<PathBuf>,
    max_connections: usize,
}

impl DirectTransportService {
    pub fn new(
        host_id: impl Into<String>,
        boot_epoch: u64,
        host_identity: Arc<SigningKey>,
        registry: Arc<DeviceRegistry>,
        command_router: Arc<CommandRouter>,
    ) -> Result<Self, DirectTransportError> {
        let host_id = host_id.into();
        if host_id.trim().is_empty() || boot_epoch == 0 {
            return Err(DirectTransportError::InvalidConfiguration);
        }
        Ok(Self {
            host_id,
            boot_epoch,
            host_identity,
            registry,
            pairing: None,
            command_router,
            journal_path: None,
            max_connections: DEFAULT_MAX_CONNECTIONS,
        })
    }

    pub fn with_event_journal(
        mut self,
        path: impl Into<PathBuf>,
    ) -> Self {
        self.journal_path = Some(path.into());
        self
    }

    pub fn new_with_pairing(
        host_id: impl Into<String>,
        boot_epoch: u64,
        host_identity: Arc<SigningKey>,
        pairing: Arc<Mutex<PairingCoordinator>>,
        command_router: Arc<CommandRouter>,
    ) -> Result<Self, DirectTransportError> {
        let registry = {
            let coordinator = pairing
                .lock()
                .map_err(|_| DirectTransportError::PairingLockUnavailable)?;
            Arc::new(coordinator.load_registry()?)
        };
        let mut service = Self::new(
            host_id,
            boot_epoch,
            host_identity,
            registry,
            command_router,
        )?;
        service.pairing = Some(pairing);
        Ok(service)
    }

    #[cfg(test)]
    fn with_max_connections(mut self, max_connections: usize) -> Self {
        self.max_connections = max_connections;
        self
    }

    pub async fn serve(
        self,
        listener: TcpListener,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), DirectTransportError> {
        let service = Arc::new(self);
        let capacity = Arc::new(Semaphore::new(service.max_connections));
        let mut sessions = JoinSet::new();

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                accepted = listener.accept() => {
                    let (stream, peer) = accepted?;
                    let Ok(permit) = Arc::clone(&capacity).try_acquire_owned() else {
                        warn!(%peer, "rejecting direct transport connection at capacity");
                        drop(stream);
                        continue;
                    };
                    let session_service = Arc::clone(&service);
                    let session_shutdown = shutdown.clone();
                    sessions.spawn(async move {
                        let _permit = permit;
                        if let Err(error) = session_service
                            .handle_connection(stream, session_shutdown)
                            .await
                        {
                            debug!(%peer, %error, "direct transport session closed");
                        }
                    });
                }
                completed = sessions.join_next(), if !sessions.is_empty() => {
                    if let Some(Err(error)) = completed {
                        warn!(%error, "direct transport session task failed");
                    }
                }
            }
        }

        sessions.abort_all();
        while sessions.join_next().await.is_some() {}
        Ok(())
    }

    async fn handle_connection(
        &self,
        mut stream: TcpStream,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), DirectTransportError> {
        stream.set_nodelay(true)?;
        let mut challenge_bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut challenge_bytes);
        let challenge = encode_hex(&challenge_bytes);
        let server_challenge = ServerChallenge::create(
            &self.host_id,
            self.boot_epoch,
            challenge.clone(),
            &self.host_identity,
        )?;
        handshake_write(
            &mut stream,
            &serde_json::to_vec(&server_challenge)?,
        )
        .await?;

        let initiator_bytes = handshake_read(&mut stream).await?;
        if serde_json::from_slice::<serde_json::Value>(&initiator_bytes)?
            .get("kind")
            .and_then(serde_json::Value::as_str)
            == Some("pairing")
        {
            return self
                .handle_pairing_connection(
                    &mut stream,
                    &challenge,
                    &initiator_bytes,
                )
                .await;
        }
        let parsed = serde_json::from_slice::<serde_json::Value>(
            &initiator_bytes,
        )?;
        let (initiator, sync_cursor) =
            if parsed.get("kind").and_then(serde_json::Value::as_str)
                == Some("sync")
            {
                let request: SyncHandshakeRequest =
                    serde_json::from_value(parsed)?;
                if request.kind != "sync"
                    || request.protocol_version
                        != HANDSHAKE_PROTOCOL_VERSION
                {
                    return Err(
                        DirectTransportError::InvalidSyncAcknowledgement,
                    );
                }
                (
                    request.initiator,
                    Some((
                        request.after_sequence,
                        request.known_boot_epoch,
                    )),
                )
            } else {
                (
                    serde_json::from_slice::<InitiatorHello>(
                        &initiator_bytes,
                    )?,
                    None,
                )
            };
        let verified = if let Some(pairing) = &self.pairing {
            let coordinator = pairing
                .lock()
                .map_err(|_| DirectTransportError::PairingLockUnavailable)?;
            let registry = coordinator.load_registry()?;
            verify_authorized_initiator(
                &initiator,
                &self.host_id,
                &challenge,
                &registry,
            )?
        } else {
            verify_authorized_initiator(
                &initiator,
                &self.host_id,
                &challenge,
                &self.registry,
            )?
        };

        let responder_key = KeyPair::generate();
        let responder = create_responder_hello(
            &self.host_identity,
            &self.host_id,
            &initiator,
            &responder_key.public,
        )?;
        let transcript = authenticated_transcript(&initiator, &responder)?;
        let mut shared_secret = derive_shared_secret(
            responder_key.secret,
            verified.ephemeral_public(),
            &transcript,
        )?;
        let directional_keys =
            derive_session_keys(&shared_secret, &transcript)?;
        let provisioning_key =
            SecretProvisioningKey::derive(&shared_secret, &transcript)?;
        shared_secret.zeroize();
        let cipher = SessionCipher::from_directional_keys(
            &directional_keys,
            SessionRole::Responder,
        );
        let mut session = SecureEnvelopeSession::new(
            cipher,
            &self.host_id,
            verified.device_id(),
            self.boot_epoch,
            &self.host_id,
            verified.device_id(),
            &transcript,
            DEFAULT_MAX_PLAINTEXT_BYTES,
        )?;

        handshake_write(&mut stream, &serde_json::to_vec(&responder)?).await?;

        if let Some((after_sequence, known_boot_epoch)) = sync_cursor {
            return self
                .handle_sync_connection(
                    &mut stream,
                    &mut shutdown,
                    session,
                    verified.device_id(),
                    after_sequence,
                    known_boot_epoch,
                )
                .await;
        }

        loop {
            let encoded = tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                    continue;
                }
                record = read_record(&mut stream, MAX_ENCRYPTED_RECORD_BYTES) => {
                    match record {
                        Ok(record) => record,
                        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                            return Ok(());
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
            };
            let frame = EncryptedFrame::decode_wire(
                &encoded,
                DEFAULT_MAX_PLAINTEXT_BYTES + 16,
            )?;
            let authenticated = session.decrypt_command(&frame)?;
            let result = self
                .command_router
                .dispatch_authenticated(
                    &authenticated.idempotency_key,
                    &authenticated.command,
                    &authenticated.sender_id,
                    &provisioning_key,
                )
                .await?;
            let response = session.encrypt_payload(
                muxport_envelope::Payload::CommandResult(result),
                "",
            )?;
            write_record(&mut stream, &response.encode_wire()?).await?;
        }
    }

    async fn handle_pairing_connection(
        &self,
        stream: &mut TcpStream,
        server_challenge: &str,
        request_bytes: &[u8],
    ) -> Result<(), DirectTransportError> {
        let pairing = self
            .pairing
            .as_ref()
            .ok_or(DirectTransportError::InvalidPairingRequest)?;
        let request: PairingHandshakeRequest =
            serde_json::from_slice(request_bytes)?;
        if request.kind != "pairing"
            || request.protocol_version != HANDSHAKE_PROTOCOL_VERSION
            || request.device_name.trim().is_empty()
            || request.device_name.len() > 128
        {
            return Err(DirectTransportError::InvalidPairingRequest);
        }
        let expected_challenge = pairing_connection_challenge(
            &request.rendezvous_token,
            server_challenge,
        )?;
        let mut claim = {
            let mut coordinator = pairing
                .lock()
                .map_err(|_| DirectTransportError::PairingLockUnavailable)?;
            coordinator.claim(
                &request.rendezvous_token,
                &expected_challenge,
                &request.initiator,
                &request.device_name,
            )?
        };
        info!(
            pairing_id = %claim.pairing_id(),
            device_id = %request.initiator.device_id,
            sas = %claim.host_sas(),
            "pairing claim received; compare this SAS on the trusted host display"
        );
        let response = PairingHandshakeResponse {
            protocol_version: HANDSHAKE_PROTOCOL_VERSION,
            pairing_id: claim.pairing_id().to_owned(),
            responder: claim.responder().clone(),
        };
        handshake_write(stream, &serde_json::to_vec(&response)?).await?;

        let confirmation_wire = timeout(
            PAIRING_CONFIRMATION_TIMEOUT,
            read_record(stream, MAX_HANDSHAKE_BYTES),
        )
        .await
        .map_err(|_| DirectTransportError::HandshakeTimeout)??;
        let confirmation_frame = EncryptedFrame::decode_wire(
            &confirmation_wire,
            MAX_HANDSHAKE_BYTES,
        )?;
        let confirmation_plaintext =
            claim.decrypt_phone_record(&confirmation_frame)?;
        let confirmation: PairingPhoneConfirmation =
            serde_json::from_slice(&confirmation_plaintext)?;
        if confirmation.protocol_version != HANDSHAKE_PROTOCOL_VERSION
            || confirmation.pairing_id != claim.pairing_id()
            || !confirmation.confirmed
        {
            return Err(DirectTransportError::InvalidPairingRequest);
        }
        let fully_confirmed = {
            let mut coordinator = pairing
                .lock()
                .map_err(|_| DirectTransportError::PairingLockUnavailable)?;
            coordinator.confirm(
                claim.pairing_id(),
                claim.host_sas(),
                ConfirmingParty::Phone,
            )?
        };
        let acknowledgement = PairingPhoneAcknowledgement {
            protocol_version: HANDSHAKE_PROTOCOL_VERSION,
            pairing_id: claim.pairing_id().to_owned(),
            awaiting_host_confirmation: !fully_confirmed,
        };
        let encrypted_ack = claim.encrypt_host_record(
            &serde_json::to_vec(&acknowledgement)?,
        )?;
        write_record(stream, &encrypted_ack.encode_wire()?).await?;
        Ok(())
    }

    async fn handle_sync_connection(
        &self,
        stream: &mut TcpStream,
        shutdown: &mut watch::Receiver<bool>,
        mut session: SecureEnvelopeSession,
        device_id: &str,
        after_sequence: u64,
        known_boot_epoch: u64,
    ) -> Result<(), DirectTransportError> {
        let journal_path = self
            .journal_path
            .as_ref()
            .ok_or(DirectTransportError::SyncUnavailable)?;
        let mut journal =
            EventJournal::open_file(journal_path, self.boot_epoch)?;
        self.send_sync_batch(
            stream,
            &mut session,
            &mut journal,
            after_sequence,
            known_boot_epoch != self.boot_epoch,
        )
        .await?;

        loop {
            let encoded = tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                    continue;
                }
                record = read_record(stream, MAX_ENCRYPTED_RECORD_BYTES) => {
                    match record {
                        Ok(record) => record,
                        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                            return Ok(());
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
            };
            let frame = EncryptedFrame::decode_wire(
                &encoded,
                DEFAULT_MAX_PLAINTEXT_BYTES + 16,
            )?;
            let envelope = session.decrypt_envelope(&frame)?;
            let Some(muxport_envelope::Payload::Ack(ack)) =
                envelope.payload
            else {
                return Err(
                    DirectTransportError::InvalidSyncAcknowledgement,
                );
            };
            journal.refresh_current_sequence()?;
            if ack.boot_epoch != self.boot_epoch
                || ack.sequence_acknowledged
                    > journal.current_sequence()
            {
                return Err(
                    DirectTransportError::InvalidSyncAcknowledgement,
                );
            }
            journal.record_cursor_ack(
                device_id,
                ack.sequence_acknowledged,
            )?;
            self.send_sync_batch(
                stream,
                &mut session,
                &mut journal,
                ack.sequence_acknowledged,
                false,
            )
            .await?;
        }
    }

    async fn send_sync_batch(
        &self,
        stream: &mut TcpStream,
        session: &mut SecureEnvelopeSession,
        journal: &mut EventJournal,
        requested_after_sequence: u64,
        force_snapshot: bool,
    ) -> Result<u64, DirectTransportError> {
        journal.refresh_current_sequence()?;
        let current = journal.current_sequence();
        let requires_snapshot = if force_snapshot
            || requested_after_sequence == 0
            || requested_after_sequence > current
        {
            true
        } else {
            match journal.get_events_after(requested_after_sequence, 1) {
                Ok(_) => false,
                Err(JournalError::GapDetected { .. })
                | Err(JournalError::CursorBeyondCurrent { .. }) => true,
                Err(error) => return Err(error.into()),
            }
        };
        let mut cursor = requested_after_sequence;
        if requires_snapshot {
            let snapshot = journal
                .latest_snapshot()?
                .ok_or(DirectTransportError::MissingSyncSnapshot)?;
            cursor = snapshot.snapshot_sequence;
            self.write_secure_payload(
                stream,
                session,
                muxport_envelope::Payload::Snapshot(snapshot),
                "",
            )
            .await?;
        }

        loop {
            journal.refresh_current_sequence()?;
            let events = journal.get_events_after(cursor, 128)?;
            if events.is_empty() {
                break;
            }
            for (sequence, event) in events {
                self.write_secure_payload(
                    stream,
                    session,
                    muxport_envelope::Payload::Event(event),
                    &sequence.to_string(),
                )
                .await?;
                cursor = sequence;
            }
        }

        journal.refresh_current_sequence()?;
        let boundary = journal.current_sequence();
        self.write_secure_payload(
            stream,
            session,
            muxport_envelope::Payload::Ack(Ack {
                sequence_acknowledged: boundary,
                boot_epoch: self.boot_epoch,
            }),
            "",
        )
        .await?;
        Ok(boundary)
    }

    async fn write_secure_payload(
        &self,
        stream: &mut TcpStream,
        session: &mut SecureEnvelopeSession,
        payload: muxport_envelope::Payload,
        cursor_metadata: &str,
    ) -> Result<(), DirectTransportError> {
        let frame = session.encrypt_payload(payload, cursor_metadata)?;
        write_record(stream, &frame.encode_wire()?).await?;
        Ok(())
    }
}

async fn handshake_read(
    stream: &mut TcpStream,
) -> Result<Vec<u8>, DirectTransportError> {
    timeout(
        HANDSHAKE_TIMEOUT,
        read_record(stream, MAX_HANDSHAKE_BYTES),
    )
    .await
    .map_err(|_| DirectTransportError::HandshakeTimeout)?
    .map_err(Into::into)
}

async fn handshake_write(
    stream: &mut TcpStream,
    bytes: &[u8],
) -> Result<(), DirectTransportError> {
    timeout(HANDSHAKE_TIMEOUT, write_record(stream, bytes))
        .await
        .map_err(|_| DirectTransportError::HandshakeTimeout)?
        .map_err(Into::into)
}

async fn read_record(
    stream: &mut TcpStream,
    maximum_bytes: usize,
) -> io::Result<Vec<u8>> {
    let mut length_bytes = [0_u8; 4];
    stream.read_exact(&mut length_bytes).await?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length == 0 || length > maximum_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            DirectTransportError::RecordTooLarge,
        ));
    }
    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}

async fn write_record(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    let length: u32 = bytes
        .len()
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "record too large"))?;
    if length == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "empty transport record",
        ));
    }
    stream.write_all(&length.to_be_bytes()).await?;
    stream.write_all(bytes).await?;
    stream.flush().await
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

fn decode_hex<const N: usize>(
    value: &str,
) -> Result<[u8; N], DirectTransportError> {
    if value.len() != N * 2 {
        return Err(DirectTransportError::InvalidServerChallenge);
    }
    let mut output = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair)
            .map_err(|_| DirectTransportError::InvalidServerChallenge)?;
        output[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| DirectTransportError::InvalidServerChallenge)?;
    }
    Ok(output)
}

fn append_signed_field(
    output: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), DirectTransportError> {
    let length: u32 = value
        .len()
        .try_into()
        .map_err(|_| DirectTransportError::InvalidServerChallenge)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandRouter, PairingCoordinator, PairingStore};
    use connector_core::CommandLedger;
    use ed25519_dalek::SigningKey;
    use muxport_crypto::{
        create_initiator_hello, generate_sas_code,
        pairing_connection_challenge, verify_responder_hello,
        PairedDeviceRecord, ResponderHello,
    };
    use muxport_protocol::{
        command, Command, ConnectorState, Event, HostSnapshot,
        ProbeHostCmd, RemoteOpState,
    };
    use rand::rngs::OsRng;
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::net::TcpStream;

    fn service(
        phone_identity: &SigningKey,
    ) -> (DirectTransportService, Arc<SigningKey>) {
        let host_identity = Arc::new(SigningKey::generate(&mut OsRng));
        let mut registry = DeviceRegistry::new();
        registry
            .register_device(PairedDeviceRecord {
                device_id: "phone-1".into(),
                device_name: "Phone".into(),
                public_key_hex: encode_hex(
                    phone_identity.verifying_key().as_bytes(),
                ),
                paired_at_ms: 1,
                last_seen_at_ms: 1,
                is_revoked: false,
            })
            .unwrap();
        let router = Arc::new(CommandRouter::new(
            CommandLedger::new(),
            HashMap::new(),
        ));
        (
            DirectTransportService::new(
                "host-1",
                22,
                Arc::clone(&host_identity),
                Arc::new(registry),
                router,
            )
            .unwrap(),
            host_identity,
        )
    }

    async fn read_secure_envelope(
        stream: &mut TcpStream,
        session: &mut SecureEnvelopeSession,
    ) -> muxport_protocol::MuxportEnvelope {
        let wire =
            read_record(stream, MAX_ENCRYPTED_RECORD_BYTES)
                .await
                .unwrap();
        let frame = EncryptedFrame::decode_wire(
            &wire,
            DEFAULT_MAX_PLAINTEXT_BYTES + 16,
        )
        .unwrap();
        session.decrypt_envelope(&frame).unwrap()
    }

    #[tokio::test]
    async fn authorized_client_dispatches_encrypted_probe() {
        let phone_identity = SigningKey::generate(&mut OsRng);
        let (service, host_identity) = service(&phone_identity);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(service.serve(listener, shutdown_rx));

        let mut stream = TcpStream::connect(address).await.unwrap();
        let challenge: ServerChallenge =
            serde_json::from_slice(&handshake_read(&mut stream).await.unwrap())
                .unwrap();
        assert_eq!(challenge.host_id, "host-1");
        assert_eq!(
            challenge.host_identity_public_key_hex,
            encode_hex(host_identity.verifying_key().as_bytes())
        );
        challenge
            .verify(
                "host-1",
                &host_identity.verifying_key(),
                chrono::Utc::now().timestamp_millis(),
            )
            .unwrap();

        let phone_ephemeral = KeyPair::generate();
        let initiator = create_initiator_hello(
            &phone_identity,
            "phone-1",
            "host-1",
            &challenge.challenge,
            &phone_ephemeral.public,
        )
        .unwrap();
        handshake_write(
            &mut stream,
            &serde_json::to_vec(&initiator).unwrap(),
        )
        .await
        .unwrap();
        let responder: ResponderHello =
            serde_json::from_slice(&handshake_read(&mut stream).await.unwrap())
                .unwrap();
        let responder_ephemeral = verify_responder_hello(
            &responder,
            &initiator,
            "host-1",
            &challenge.host_identity_public_key_hex,
        )
        .unwrap();
        let transcript =
            authenticated_transcript(&initiator, &responder).unwrap();
        let shared = derive_shared_secret(
            phone_ephemeral.secret,
            &responder_ephemeral,
            &transcript,
        )
        .unwrap();
        let keys = derive_session_keys(&shared, &transcript).unwrap();
        let cipher = SessionCipher::from_directional_keys(
            &keys,
            SessionRole::Initiator,
        );
        let mut phone_session = SecureEnvelopeSession::new(
            cipher,
            "phone-1",
            "host-1",
            11,
            "host-1",
            "phone-1",
            &transcript,
            DEFAULT_MAX_PLAINTEXT_BYTES,
        )
        .unwrap();

        let command = Command {
            command_id: "probe-1".into(),
            deadline_ms: chrono::Utc::now().timestamp_millis() + 60_000,
            inner: Some(command::Inner::ProbeHost(ProbeHostCmd {})),
        };
        let encrypted = phone_session
            .encrypt_payload(
                muxport_envelope::Payload::Command(command),
                "probe-idempotency-1",
            )
            .unwrap();
        write_record(&mut stream, &encrypted.encode_wire().unwrap())
            .await
            .unwrap();

        let result_wire =
            read_record(&mut stream, MAX_ENCRYPTED_RECORD_BYTES)
                .await
                .unwrap();
        let result_frame = EncryptedFrame::decode_wire(
            &result_wire,
            DEFAULT_MAX_PLAINTEXT_BYTES + 16,
        )
        .unwrap();
        let result = phone_session.decrypt_envelope(&result_frame).unwrap();
        let Some(muxport_envelope::Payload::CommandResult(result)) =
            result.payload
        else {
            panic!("expected encrypted command result");
        };
        assert!(result.success);
        assert_eq!(
            RemoteOpState::try_from(result.state).unwrap(),
            RemoteOpState::Succeeded
        );
        assert_eq!(result.result_json, r#"{"reachable":true}"#);

        shutdown_tx.send(true).unwrap();
        server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn unregistered_client_is_rejected_before_encrypted_session() {
        let registered = SigningKey::generate(&mut OsRng);
        let unregistered = SigningKey::generate(&mut OsRng);
        let (service, _) = service(&registered);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(service.serve(listener, shutdown_rx));

        let mut stream = TcpStream::connect(address).await.unwrap();
        let challenge: ServerChallenge =
            serde_json::from_slice(&handshake_read(&mut stream).await.unwrap())
                .unwrap();
        let ephemeral = KeyPair::generate();
        let initiator = create_initiator_hello(
            &unregistered,
            "phone-1",
            "host-1",
            &challenge.challenge,
            &ephemeral.public,
        )
        .unwrap();
        handshake_write(
            &mut stream,
            &serde_json::to_vec(&initiator).unwrap(),
        )
        .await
        .unwrap();

        let response = handshake_read(&mut stream).await;
        assert!(matches!(
            response,
            Err(DirectTransportError::Io(ref error))
                if error.kind() == io::ErrorKind::UnexpectedEof
        ));

        shutdown_tx.send(true).unwrap();
        server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn live_pairing_enrolls_device_and_refreshes_authorization() {
        let phone_identity = SigningKey::generate(&mut OsRng);
        let host_identity = Arc::new(SigningKey::generate(&mut OsRng));
        let coordinator = Arc::new(Mutex::new(
            PairingCoordinator::new(
                PairingStore::open_in_memory().unwrap(),
                Arc::clone(&host_identity),
                "host-1",
                "Test host",
                "127.0.0.1:45821",
            )
            .unwrap(),
        ));
        let offer = coordinator
            .lock()
            .unwrap()
            .create_offer(Duration::from_secs(60))
            .unwrap();
        let router = Arc::new(CommandRouter::new(
            CommandLedger::new(),
            HashMap::new(),
        ));
        let service = DirectTransportService::new_with_pairing(
            "host-1",
            22,
            Arc::clone(&host_identity),
            Arc::clone(&coordinator),
            router,
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(service.serve(listener, shutdown_rx));

        let mut stream = TcpStream::connect(address).await.unwrap();
        let challenge: ServerChallenge =
            serde_json::from_slice(&handshake_read(&mut stream).await.unwrap())
                .unwrap();
        challenge
            .verify(
                "host-1",
                &host_identity.verifying_key(),
                chrono::Utc::now().timestamp_millis(),
            )
            .unwrap();
        let connection_challenge = pairing_connection_challenge(
            &offer.rendezvous_token,
            &challenge.challenge,
        )
        .unwrap();
        let phone_ephemeral = KeyPair::generate();
        let initiator = create_initiator_hello(
            &phone_identity,
            "phone-live",
            "host-1",
            &connection_challenge,
            &phone_ephemeral.public,
        )
        .unwrap();
        let request = PairingHandshakeRequest {
            kind: "pairing".into(),
            protocol_version: HANDSHAKE_PROTOCOL_VERSION,
            rendezvous_token: offer.rendezvous_token.clone(),
            device_name: "Live phone".into(),
            initiator: initiator.clone(),
        };
        handshake_write(
            &mut stream,
            &serde_json::to_vec(&request).unwrap(),
        )
        .await
        .unwrap();
        let response: PairingHandshakeResponse =
            serde_json::from_slice(&handshake_read(&mut stream).await.unwrap())
                .unwrap();
        let host_ephemeral = verify_responder_hello(
            &response.responder,
            &initiator,
            "host-1",
            &offer.host_identity_public_key_hex,
        )
        .unwrap();
        assert_eq!(
            encode_hex(host_ephemeral.as_bytes()),
            offer.ephemeral_public_key_hex
        );
        let transcript =
            authenticated_transcript(&initiator, &response.responder).unwrap();
        let shared_secret = derive_shared_secret(
            phone_ephemeral.secret,
            &host_ephemeral,
            &transcript,
        )
        .unwrap();
        let sas = generate_sas_code(&shared_secret, &transcript).unwrap();
        let keys = derive_session_keys(&shared_secret, &transcript).unwrap();
        let mut phone_cipher = SessionCipher::from_directional_keys(
            &keys,
            SessionRole::Initiator,
        );

        assert!(!coordinator
            .lock()
            .unwrap()
            .confirm(
                &response.pairing_id,
                &sas,
                ConfirmingParty::Host,
            )
            .unwrap());
        let confirmation = PairingPhoneConfirmation {
            protocol_version: HANDSHAKE_PROTOCOL_VERSION,
            pairing_id: response.pairing_id.clone(),
            confirmed: true,
        };
        let encrypted_confirmation = phone_cipher
            .encrypt_next(
                &serde_json::to_vec(&confirmation).unwrap(),
                &transcript,
            )
            .unwrap();
        write_record(
            &mut stream,
            &encrypted_confirmation.encode_wire().unwrap(),
        )
        .await
        .unwrap();
        let acknowledgement_wire =
            handshake_read(&mut stream).await.unwrap();
        let acknowledgement_frame = EncryptedFrame::decode_wire(
            &acknowledgement_wire,
            MAX_HANDSHAKE_BYTES,
        )
        .unwrap();
        let acknowledgement: PairingPhoneAcknowledgement =
            serde_json::from_slice(
                &phone_cipher
                    .decrypt_next(&acknowledgement_frame, &transcript)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(acknowledgement.pairing_id, response.pairing_id);
        assert!(!acknowledgement.awaiting_host_confirmation);

        let mut reconnect = TcpStream::connect(address).await.unwrap();
        let reconnect_challenge: ServerChallenge =
            serde_json::from_slice(
                &handshake_read(&mut reconnect).await.unwrap(),
            )
            .unwrap();
        let reconnect_ephemeral = KeyPair::generate();
        let reconnect_hello = create_initiator_hello(
            &phone_identity,
            "phone-live",
            "host-1",
            &reconnect_challenge.challenge,
            &reconnect_ephemeral.public,
        )
        .unwrap();
        handshake_write(
            &mut reconnect,
            &serde_json::to_vec(&reconnect_hello).unwrap(),
        )
        .await
        .unwrap();
        let reconnect_responder: ResponderHello = serde_json::from_slice(
            &handshake_read(&mut reconnect).await.unwrap(),
        )
        .unwrap();
        verify_responder_hello(
            &reconnect_responder,
            &reconnect_hello,
            "host-1",
            &offer.host_identity_public_key_hex,
        )
        .unwrap();

        shutdown_tx.send(true).unwrap();
        server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn sync_connection_sends_snapshot_replay_and_records_ack() {
        let journal_path = std::env::temp_dir().join(format!(
            "muxport-sync-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let mut journal =
                EventJournal::open_file(&journal_path, 22).unwrap();
            journal
                .append_event(&Event {
                    event_id: "event-1".into(),
                    timestamp_ms: 1,
                    inner: None,
                })
                .unwrap();
            journal
                .save_snapshot(&HostSnapshot {
                    host_id: "host-1".into(),
                    hostname: "Test host".into(),
                    connector_state: ConnectorState::Ready as i32,
                    runtimes: Vec::new(),
                    credential_profiles: Vec::new(),
                    active_sessions: Vec::new(),
                    snapshot_sequence: 1,
                })
                .unwrap();
            journal
                .append_event(&Event {
                    event_id: "event-2".into(),
                    timestamp_ms: 2,
                    inner: None,
                })
                .unwrap();
        }

        let phone_identity = SigningKey::generate(&mut OsRng);
        let (service, host_identity) = service(&phone_identity);
        let service = service.with_event_journal(&journal_path);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(service.serve(listener, shutdown_rx));

        let mut stream = TcpStream::connect(address).await.unwrap();
        let challenge: ServerChallenge =
            serde_json::from_slice(&handshake_read(&mut stream).await.unwrap())
                .unwrap();
        let phone_ephemeral = KeyPair::generate();
        let initiator = create_initiator_hello(
            &phone_identity,
            "phone-1",
            "host-1",
            &challenge.challenge,
            &phone_ephemeral.public,
        )
        .unwrap();
        let request = SyncHandshakeRequest {
            kind: "sync".into(),
            protocol_version: HANDSHAKE_PROTOCOL_VERSION,
            after_sequence: 0,
            known_boot_epoch: 0,
            initiator: initiator.clone(),
        };
        handshake_write(
            &mut stream,
            &serde_json::to_vec(&request).unwrap(),
        )
        .await
        .unwrap();
        let responder: ResponderHello =
            serde_json::from_slice(&handshake_read(&mut stream).await.unwrap())
                .unwrap();
        let responder_ephemeral = verify_responder_hello(
            &responder,
            &initiator,
            "host-1",
            &encode_hex(host_identity.verifying_key().as_bytes()),
        )
        .unwrap();
        let transcript =
            authenticated_transcript(&initiator, &responder).unwrap();
        let shared_secret = derive_shared_secret(
            phone_ephemeral.secret,
            &responder_ephemeral,
            &transcript,
        )
        .unwrap();
        let keys = derive_session_keys(&shared_secret, &transcript).unwrap();
        let cipher = SessionCipher::from_directional_keys(
            &keys,
            SessionRole::Initiator,
        );
        let mut phone_session = SecureEnvelopeSession::new(
            cipher,
            "phone-1",
            "host-1",
            11,
            "host-1",
            "phone-1",
            &transcript,
            DEFAULT_MAX_PLAINTEXT_BYTES,
        )
        .unwrap();

        let snapshot = read_secure_envelope(
            &mut stream,
            &mut phone_session,
        )
        .await;
        assert!(matches!(
            snapshot.payload,
            Some(muxport_envelope::Payload::Snapshot(ref value))
                if value.snapshot_sequence == 1
        ));
        let replayed = read_secure_envelope(
            &mut stream,
            &mut phone_session,
        )
        .await;
        assert_eq!(
            replayed
                .header
                .as_ref()
                .unwrap()
                .idempotency_key,
            "2"
        );
        assert!(matches!(
            replayed.payload,
            Some(muxport_envelope::Payload::Event(ref value))
                if value.event_id == "event-2"
        ));
        let boundary = read_secure_envelope(
            &mut stream,
            &mut phone_session,
        )
        .await;
        assert!(matches!(
            boundary.payload,
            Some(muxport_envelope::Payload::Ack(ref value))
                if value.sequence_acknowledged == 2
                    && value.boot_epoch == 22
        ));

        let acknowledgement = phone_session
            .encrypt_payload(
                muxport_envelope::Payload::Ack(Ack {
                    sequence_acknowledged: 2,
                    boot_epoch: 22,
                }),
                "",
            )
            .unwrap();
        write_record(
            &mut stream,
            &acknowledgement.encode_wire().unwrap(),
        )
        .await
        .unwrap();
        let empty_boundary = read_secure_envelope(
            &mut stream,
            &mut phone_session,
        )
        .await;
        assert!(matches!(
            empty_boundary.payload,
            Some(muxport_envelope::Payload::Ack(ref value))
                if value.sequence_acknowledged == 2
        ));

        drop(stream);
        shutdown_tx.send(true).unwrap();
        server.await.unwrap().unwrap();
        let journal =
            EventJournal::open_file(&journal_path, 23).unwrap();
        assert_eq!(
            journal.get_cursor_ack("phone-1").unwrap(),
            Some(2)
        );
        drop(journal);
        let _ = std::fs::remove_file(&journal_path);
        let _ = std::fs::remove_file(
            journal_path.with_extension("db-wal"),
        );
        let _ = std::fs::remove_file(
            journal_path.with_extension("db-shm"),
        );
    }

    #[tokio::test]
    async fn connection_capacity_is_bounded() {
        let phone = SigningKey::generate(&mut OsRng);
        let (service, _) = service(&phone);
        let service = service.with_max_connections(1);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(service.serve(listener, shutdown_rx));

        let mut first = TcpStream::connect(address).await.unwrap();
        handshake_read(&mut first).await.unwrap();
        let mut second = TcpStream::connect(address).await.unwrap();
        let rejected = handshake_read(&mut second).await;
        assert!(matches!(
            rejected,
            Err(DirectTransportError::Io(ref error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                )
        ));

        shutdown_tx.send(true).unwrap();
        server.await.unwrap().unwrap();
    }

    #[test]
    fn server_challenge_signature_binds_boot_epoch_and_expiry() {
        let host = SigningKey::generate(&mut OsRng);
        let challenge = ServerChallenge::create(
            "host-1",
            22,
            encode_hex(&[7_u8; 32]),
            &host,
        )
        .unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        challenge
            .verify("host-1", &host.verifying_key(), now)
            .unwrap();

        let mut changed_epoch = challenge.clone();
        changed_epoch.boot_epoch = 23;
        assert!(matches!(
            changed_epoch.verify("host-1", &host.verifying_key(), now),
            Err(DirectTransportError::InvalidServerChallenge)
        ));
        assert!(matches!(
            challenge.verify(
                "host-1",
                &host.verifying_key(),
                challenge.expires_at_ms
                    + CHALLENGE_CLOCK_SKEW.as_millis() as i64
                    + 1,
            ),
            Err(DirectTransportError::InvalidServerChallenge)
        ));
    }
}
