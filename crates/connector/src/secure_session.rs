use muxport_crypto::{CryptoError, EncryptedFrame, SessionCipher};
use muxport_protocol::{
    muxport_envelope, Command, EnvelopeHeader, MuxportEnvelope,
};
use prost::Message;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CURRENT_PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_MAX_PLAINTEXT_BYTES: usize = 1024 * 1024;

#[derive(Error, Debug)]
pub enum SecureSessionError {
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error("Secure session identity or boot epoch is invalid")]
    InvalidSessionIdentity,
    #[error("Secure envelope header is missing or invalid")]
    InvalidEnvelopeHeader,
    #[error("Secure envelope sender or recipient does not match the session")]
    WrongPeer,
    #[error("Secure envelope sequence does not match its encrypted frame")]
    SequenceMismatch,
    #[error("Remote boot epoch changed inside one encrypted session")]
    RemoteBootEpochChanged,
    #[error("Secure envelope exceeded the configured plaintext limit")]
    PlaintextTooLarge,
    #[error("Secure envelope protobuf decode failed: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("Inbound secure envelope is not a command")]
    UnexpectedPayload,
    #[error("Inbound command idempotency key must not be empty")]
    EmptyIdempotencyKey,
}

pub struct AuthenticatedCommand {
    pub sender_id: String,
    pub idempotency_key: String,
    pub command: Command,
}

/// Protocol envelope codec for an already authenticated handshake.
///
/// This type does not open sockets or pair devices. It binds encrypted frames
/// to the authenticated host/device route, validates both transport and
/// protobuf sequences, fixes the remote boot epoch for the session, and only
/// releases a command after all checks pass.
pub struct SecureEnvelopeSession {
    cipher: SessionCipher,
    local_id: String,
    remote_id: String,
    local_boot_epoch: u64,
    remote_boot_epoch: Option<u64>,
    aad: Vec<u8>,
    max_plaintext_bytes: usize,
}

impl SecureEnvelopeSession {
    pub fn new(
        cipher: SessionCipher,
        local_id: &str,
        remote_id: &str,
        local_boot_epoch: u64,
        host_id: &str,
        device_id: &str,
        transcript: &[u8],
        max_plaintext_bytes: usize,
    ) -> Result<Self, SecureSessionError> {
        if local_id.trim().is_empty()
            || remote_id.trim().is_empty()
            || host_id.trim().is_empty()
            || device_id.trim().is_empty()
            || local_boot_epoch == 0
            || max_plaintext_bytes == 0
            || !((local_id == host_id && remote_id == device_id)
                || (local_id == device_id && remote_id == host_id))
        {
            return Err(SecureSessionError::InvalidSessionIdentity);
        }
        Ok(Self {
            cipher,
            local_id: local_id.to_owned(),
            remote_id: remote_id.to_owned(),
            local_boot_epoch,
            remote_boot_epoch: None,
            aad: session_aad(host_id, device_id, transcript),
            max_plaintext_bytes,
        })
    }

    pub fn encrypt_payload(
        &mut self,
        payload: muxport_envelope::Payload,
        idempotency_key: &str,
    ) -> Result<EncryptedFrame, SecureSessionError> {
        if matches!(payload, muxport_envelope::Payload::Command(_))
            && idempotency_key.trim().is_empty()
        {
            return Err(SecureSessionError::EmptyIdempotencyKey);
        }
        let sequence = self.cipher.next_send_sequence()?;
        let envelope = MuxportEnvelope {
            header: Some(EnvelopeHeader {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                sender_id: self.local_id.clone(),
                recipient_id: self.remote_id.clone(),
                boot_epoch: self.local_boot_epoch,
                sequence,
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                idempotency_key: idempotency_key.to_owned(),
            }),
            payload: Some(payload),
        };
        let plaintext = envelope.encode_to_vec();
        if plaintext.len() > self.max_plaintext_bytes {
            return Err(SecureSessionError::PlaintextTooLarge);
        }
        let frame = self.cipher.encrypt_next(&plaintext, &self.aad)?;
        if frame.sequence != sequence {
            return Err(SecureSessionError::SequenceMismatch);
        }
        Ok(frame)
    }

    pub fn decrypt_envelope(
        &mut self,
        frame: &EncryptedFrame,
    ) -> Result<MuxportEnvelope, SecureSessionError> {
        let max_ciphertext = self
            .max_plaintext_bytes
            .checked_add(16)
            .ok_or(SecureSessionError::PlaintextTooLarge)?;
        if frame.ciphertext.len() > max_ciphertext {
            return Err(SecureSessionError::PlaintextTooLarge);
        }
        let plaintext = self.cipher.decrypt_next(frame, &self.aad)?;
        if plaintext.len() > self.max_plaintext_bytes {
            return Err(SecureSessionError::PlaintextTooLarge);
        }
        let envelope = MuxportEnvelope::decode(plaintext.as_slice())?;
        self.validate_envelope(frame.sequence, &envelope)?;
        Ok(envelope)
    }

    pub fn decrypt_command(
        &mut self,
        frame: &EncryptedFrame,
    ) -> Result<AuthenticatedCommand, SecureSessionError> {
        let envelope = self.decrypt_envelope(frame)?;
        let header = envelope
            .header
            .ok_or(SecureSessionError::InvalidEnvelopeHeader)?;
        if header.idempotency_key.trim().is_empty() {
            return Err(SecureSessionError::EmptyIdempotencyKey);
        }
        let command = match envelope.payload {
            Some(muxport_envelope::Payload::Command(command)) => command,
            _ => return Err(SecureSessionError::UnexpectedPayload),
        };
        Ok(AuthenticatedCommand {
            sender_id: header.sender_id,
            idempotency_key: header.idempotency_key,
            command,
        })
    }

    fn validate_envelope(
        &mut self,
        frame_sequence: u64,
        envelope: &MuxportEnvelope,
    ) -> Result<(), SecureSessionError> {
        let header = envelope
            .header
            .as_ref()
            .ok_or(SecureSessionError::InvalidEnvelopeHeader)?;
        if header.protocol_version != CURRENT_PROTOCOL_VERSION
            || header.boot_epoch == 0
            || header.sequence == 0
        {
            return Err(SecureSessionError::InvalidEnvelopeHeader);
        }
        if header.sender_id != self.remote_id
            || header.recipient_id != self.local_id
        {
            return Err(SecureSessionError::WrongPeer);
        }
        if header.sequence != frame_sequence {
            return Err(SecureSessionError::SequenceMismatch);
        }
        match self.remote_boot_epoch {
            Some(epoch) if epoch != header.boot_epoch => {
                return Err(SecureSessionError::RemoteBootEpochChanged)
            }
            None => self.remote_boot_epoch = Some(header.boot_epoch),
            _ => {}
        }
        Ok(())
    }
}

fn session_aad(host_id: &str, device_id: &str, transcript: &[u8]) -> Vec<u8> {
    let transcript_hash = Sha256::digest(transcript);
    let mut aad = b"muxport-secure-envelope-v1".to_vec();
    append_aad_field(&mut aad, host_id.as_bytes());
    append_aad_field(&mut aad, device_id.as_bytes());
    append_aad_field(&mut aad, &transcript_hash);
    aad
}

fn append_aad_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxport_crypto::{derive_session_keys, SessionRole};
    use muxport_protocol::{
        command, CommandResult, ProbeHostCmd, RemoteOpState,
    };

    fn sessions() -> (SecureEnvelopeSession, SecureEnvelopeSession) {
        let shared = [7_u8; 32];
        let transcript = b"authenticated transcript";
        let phone_keys = derive_session_keys(&shared, transcript).unwrap();
        let host_keys = derive_session_keys(&shared, transcript).unwrap();
        let phone_cipher = SessionCipher::from_directional_keys(
            &phone_keys,
            SessionRole::Initiator,
        );
        let host_cipher = SessionCipher::from_directional_keys(
            &host_keys,
            SessionRole::Responder,
        );
        (
            SecureEnvelopeSession::new(
                phone_cipher,
                "phone-1",
                "host-1",
                11,
                "host-1",
                "phone-1",
                transcript,
                DEFAULT_MAX_PLAINTEXT_BYTES,
            )
            .unwrap(),
            SecureEnvelopeSession::new(
                host_cipher,
                "host-1",
                "phone-1",
                22,
                "host-1",
                "phone-1",
                transcript,
                DEFAULT_MAX_PLAINTEXT_BYTES,
            )
            .unwrap(),
        )
    }

    #[test]
    fn encrypted_command_validates_route_sequence_and_idempotency() {
        let (mut phone, mut host) = sessions();
        let command = Command {
            command_id: "command-1".into(),
            deadline_ms: chrono::Utc::now().timestamp_millis() + 60_000,
            inner: Some(command::Inner::ProbeHost(ProbeHostCmd {})),
        };
        let frame = phone
            .encrypt_payload(
                muxport_envelope::Payload::Command(command),
                "idempotency-1",
            )
            .unwrap();
        let authenticated = host.decrypt_command(&frame).unwrap();
        assert_eq!(authenticated.sender_id, "phone-1");
        assert_eq!(authenticated.idempotency_key, "idempotency-1");
        assert_eq!(authenticated.command.command_id, "command-1");
        assert!(matches!(
            host.decrypt_command(&frame),
            Err(SecureSessionError::Crypto(CryptoError::ReplayDetected))
        ));
    }

    #[test]
    fn encrypted_results_round_trip_and_boot_epoch_cannot_change() {
        let (mut phone, mut host) = sessions();
        let first = phone
            .encrypt_payload(
                muxport_envelope::Payload::Command(Command {
                    command_id: "command-1".into(),
                    deadline_ms: 0,
                    inner: Some(command::Inner::ProbeHost(ProbeHostCmd {})),
                }),
                "idempotency-1",
            )
            .unwrap();
        host.decrypt_command(&first).unwrap();

        let result = host
            .encrypt_payload(
                muxport_envelope::Payload::CommandResult(CommandResult {
                    command_id: "command-1".into(),
                    state: RemoteOpState::Succeeded as i32,
                    success: true,
                    error_message: String::new(),
                    completed_at_ms: 1,
                    result_json: "{}".into(),
                }),
                "",
            )
            .unwrap();
        let envelope = phone.decrypt_envelope(&result).unwrap();
        assert!(matches!(
            envelope.payload,
            Some(muxport_envelope::Payload::CommandResult(_))
        ));

        phone.local_boot_epoch = 12;
        let changed_boot = phone
            .encrypt_payload(
                muxport_envelope::Payload::Command(Command {
                    command_id: "command-2".into(),
                    deadline_ms: 0,
                    inner: Some(command::Inner::ProbeHost(ProbeHostCmd {})),
                }),
                "idempotency-2",
            )
            .unwrap();
        assert!(matches!(
            host.decrypt_command(&changed_boot),
            Err(SecureSessionError::RemoteBootEpochChanged)
        ));
    }
}
