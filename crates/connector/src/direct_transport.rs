use crate::{
    CommandDispatchError, CommandRouter, SecureEnvelopeSession,
    SecureSessionError, DEFAULT_MAX_PLAINTEXT_BYTES,
};
use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use muxport_crypto::{
    authenticated_transcript, create_responder_hello, derive_session_keys,
    derive_shared_secret, verify_authorized_initiator, CryptoError,
    DeviceRegistry, EncryptedFrame, InitiatorHello, KeyPair, SessionCipher,
    SessionRole, HANDSHAKE_PROTOCOL_VERSION,
};
use muxport_protocol::muxport_envelope;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::{debug, warn};

const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;
const MAX_ENCRYPTED_RECORD_BYTES: usize =
    DEFAULT_MAX_PLAINTEXT_BYTES + 16 + 16;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
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

#[derive(Clone)]
pub struct DirectTransportService {
    host_id: String,
    boot_epoch: u64,
    host_identity: Arc<SigningKey>,
    registry: Arc<DeviceRegistry>,
    command_router: Arc<CommandRouter>,
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
            command_router,
            max_connections: DEFAULT_MAX_CONNECTIONS,
        })
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
        let initiator: InitiatorHello =
            serde_json::from_slice(&initiator_bytes)?;
        let verified = verify_authorized_initiator(
            &initiator,
            &self.host_id,
            &challenge,
            &self.registry,
        )?;

        let responder_key = KeyPair::generate();
        let responder = create_responder_hello(
            &self.host_identity,
            &self.host_id,
            &initiator,
            &responder_key.public,
        )?;
        let transcript = authenticated_transcript(&initiator, &responder)?;
        let shared_secret = derive_shared_secret(
            responder_key.secret,
            verified.ephemeral_public(),
            &transcript,
        )?;
        let directional_keys =
            derive_session_keys(&shared_secret, &transcript)?;
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
                .dispatch(
                    &authenticated.idempotency_key,
                    &authenticated.command,
                )
                .await?;
            let response = session.encrypt_payload(
                muxport_envelope::Payload::CommandResult(result),
                "",
            )?;
            write_record(&mut stream, &response.encode_wire()?).await?;
        }
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
    use crate::CommandRouter;
    use connector_core::CommandLedger;
    use ed25519_dalek::SigningKey;
    use muxport_crypto::{
        create_initiator_hello, verify_responder_hello,
        PairedDeviceRecord, ResponderHello,
    };
    use muxport_protocol::{
        command, Command, ProbeHostCmd, RemoteOpState,
    };
    use rand::rngs::OsRng;
    use std::collections::HashMap;
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
