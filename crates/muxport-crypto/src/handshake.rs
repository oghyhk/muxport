use crate::{CryptoError, DeviceRegistry};
use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::PublicKey as XPublicKey;

pub const HANDSHAKE_PROTOCOL_VERSION: u32 = 1;
const INITIATOR_DOMAIN: &[u8] = b"muxport-initiator-hello-v1";
const RESPONDER_DOMAIN: &[u8] = b"muxport-responder-hello-v1";
const TRANSCRIPT_DOMAIN: &[u8] = b"muxport-authenticated-transcript-v1";
const PAIRING_OFFER_DOMAIN: &[u8] = b"muxport-pairing-offer-v1";
const PAIRING_CONNECTION_CHALLENGE_DOMAIN: &[u8] =
    b"muxport-pairing-connection-challenge-v1";
const MAX_PAIRING_OFFER_LIFETIME_MS: i64 = 10 * 60 * 1000;
const PAIRING_OFFER_CLOCK_SKEW_MS: i64 = 60 * 1000;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignedPairingOffer {
    pub protocol_version: u32,
    pub host_id: String,
    pub hostname: String,
    pub host_identity_public_key_hex: String,
    pub direct_endpoint: String,
    pub rendezvous_token: String,
    pub ephemeral_public_key_hex: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub signature_hex: String,
}

impl SignedPairingOffer {
    pub fn create(
        host_identity: &SigningKey,
        host_id: &str,
        hostname: &str,
        direct_endpoint: &str,
        rendezvous_token: &str,
        ephemeral_public_key_hex: &str,
        issued_at_ms: i64,
        expires_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        let mut offer = Self {
            protocol_version: HANDSHAKE_PROTOCOL_VERSION,
            host_id: host_id.to_owned(),
            hostname: hostname.to_owned(),
            host_identity_public_key_hex: encode_hex(
                host_identity.verifying_key().as_bytes(),
            ),
            direct_endpoint: direct_endpoint.to_owned(),
            rendezvous_token: rendezvous_token.to_owned(),
            ephemeral_public_key_hex: ephemeral_public_key_hex.to_owned(),
            issued_at_ms,
            expires_at_ms,
            signature_hex: String::new(),
        };
        offer.validate_fields()?;
        offer.signature_hex =
            encode_hex(&host_identity.sign(&offer.signed_bytes()?).to_bytes());
        Ok(offer)
    }

    /// Verifies the offer's self-signature and validity window. The returned
    /// key is a candidate pin: callers must still complete the displayed SAS
    /// comparison before persisting trust.
    pub fn verify(
        &self,
        now_ms: i64,
    ) -> Result<VerifyingKey, CryptoError> {
        self.validate_fields()?;
        if now_ms
            < self
                .issued_at_ms
                .saturating_sub(PAIRING_OFFER_CLOCK_SKEW_MS)
        {
            return Err(CryptoError::InvalidPairingOffer);
        }
        if now_ms
            > self
                .expires_at_ms
                .saturating_add(PAIRING_OFFER_CLOCK_SKEW_MS)
        {
            return Err(CryptoError::ExpiredPairingOffer);
        }
        let identity_bytes =
            parse_hex::<32>(&self.host_identity_public_key_hex)?;
        let identity = VerifyingKey::from_bytes(&identity_bytes)
            .map_err(|_| CryptoError::InvalidPairingOffer)?;
        let signature_bytes = parse_hex::<64>(&self.signature_hex)?;
        identity
            .verify(
                &self.signed_bytes()?,
                &Signature::from_bytes(&signature_bytes),
            )
            .map_err(|_| CryptoError::InvalidHandshakeSignature)?;
        Ok(identity)
    }

    fn validate_fields(&self) -> Result<(), CryptoError> {
        let lifetime = self
            .expires_at_ms
            .checked_sub(self.issued_at_ms)
            .ok_or(CryptoError::InvalidPairingOffer)?;
        if self.protocol_version != HANDSHAKE_PROTOCOL_VERSION
            || self.host_id.trim().is_empty()
            || self.host_id.len() > 256
            || self.hostname.trim().is_empty()
            || self.hostname.len() > 256
            || self.direct_endpoint.trim().is_empty()
            || self.direct_endpoint.len() > 512
            || lifetime <= 0
            || lifetime > MAX_PAIRING_OFFER_LIFETIME_MS
        {
            return Err(CryptoError::InvalidPairingOffer);
        }
        let host_identity =
            parse_hex::<32>(&self.host_identity_public_key_hex)
                .map_err(|_| CryptoError::InvalidPairingOffer)?;
        let rendezvous_token = parse_hex::<32>(&self.rendezvous_token)
            .map_err(|_| CryptoError::InvalidPairingOffer)?;
        let ephemeral_public =
            parse_hex::<32>(&self.ephemeral_public_key_hex)
                .map_err(|_| CryptoError::InvalidPairingOffer)?;
        if self.host_identity_public_key_hex != encode_hex(&host_identity)
            || self.rendezvous_token != encode_hex(&rendezvous_token)
            || self.ephemeral_public_key_hex != encode_hex(&ephemeral_public)
        {
            return Err(CryptoError::InvalidPairingOffer);
        }
        if !self.signature_hex.is_empty() {
            let signature = parse_hex::<64>(&self.signature_hex)
                .map_err(|_| CryptoError::InvalidPairingOffer)?;
            if self.signature_hex != encode_hex(&signature) {
                return Err(CryptoError::InvalidPairingOffer);
            }
        }
        Ok(())
    }

    fn signed_bytes(&self) -> Result<Vec<u8>, CryptoError> {
        let mut claim = Vec::new();
        push_field(&mut claim, PAIRING_OFFER_DOMAIN)?;
        claim.extend_from_slice(&self.protocol_version.to_be_bytes());
        push_field(&mut claim, self.host_id.as_bytes())?;
        push_field(&mut claim, self.hostname.as_bytes())?;
        push_field(
            &mut claim,
            &parse_hex::<32>(&self.host_identity_public_key_hex)?,
        )?;
        push_field(&mut claim, self.direct_endpoint.as_bytes())?;
        push_field(
            &mut claim,
            &parse_hex::<32>(&self.rendezvous_token)?,
        )?;
        push_field(
            &mut claim,
            &parse_hex::<32>(&self.ephemeral_public_key_hex)?,
        )?;
        claim.extend_from_slice(&self.issued_at_ms.to_be_bytes());
        claim.extend_from_slice(&self.expires_at_ms.to_be_bytes());
        Ok(claim)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InitiatorHello {
    pub protocol_version: u32,
    pub device_id: String,
    pub host_id: String,
    pub challenge: String,
    pub device_identity_public_key_hex: String,
    pub ephemeral_public_key_hex: String,
    pub nonce_hex: String,
    pub signature_hex: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResponderHello {
    pub protocol_version: u32,
    pub host_id: String,
    pub device_id: String,
    pub host_identity_public_key_hex: String,
    pub ephemeral_public_key_hex: String,
    pub nonce_hex: String,
    pub initiator_hash_hex: String,
    pub signature_hex: String,
}

pub struct VerifiedInitiator {
    device_id: String,
    host_id: String,
    challenge: String,
    device_identity_public_key_hex: String,
    ephemeral_public: XPublicKey,
}

impl VerifiedInitiator {
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    pub fn device_identity_public_key_hex(&self) -> &str {
        &self.device_identity_public_key_hex
    }

    pub fn ephemeral_public(&self) -> &XPublicKey {
        &self.ephemeral_public
    }
}

pub fn create_initiator_hello(
    identity: &SigningKey,
    device_id: &str,
    host_id: &str,
    challenge: &str,
    ephemeral_public: &XPublicKey,
) -> Result<InitiatorHello, CryptoError> {
    validate_context(device_id, host_id, challenge)?;
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let mut hello = InitiatorHello {
        protocol_version: HANDSHAKE_PROTOCOL_VERSION,
        device_id: device_id.to_owned(),
        host_id: host_id.to_owned(),
        challenge: challenge.to_owned(),
        device_identity_public_key_hex: encode_hex(&identity.verifying_key().to_bytes()),
        ephemeral_public_key_hex: encode_hex(ephemeral_public.as_bytes()),
        nonce_hex: encode_hex(&nonce),
        signature_hex: String::new(),
    };
    let claim = initiator_claim(&hello)?;
    hello.signature_hex = encode_hex(&identity.sign(&claim).to_bytes());
    Ok(hello)
}

/// Binds a one-time pairing token to the fresh, signed challenge emitted by
/// the specific listener connection. This prevents a captured pairing hello
/// from being replayed on a later connection while keeping the rendezvous
/// token itself out of the signed initiator challenge field.
pub fn pairing_connection_challenge(
    rendezvous_token: &str,
    server_challenge: &str,
) -> Result<String, CryptoError> {
    let token = parse_hex::<32>(rendezvous_token)?;
    let challenge = parse_hex::<32>(server_challenge)?;
    let mut bound = Vec::new();
    push_field(&mut bound, PAIRING_CONNECTION_CHALLENGE_DOMAIN)?;
    push_field(&mut bound, &token)?;
    push_field(&mut bound, &challenge)?;
    Ok(encode_hex(&Sha256::digest(bound)))
}

/// Verifies the initiator's self-asserted identity. This is suitable for the
/// pre-registration pairing step only; the caller must still complete SAS
/// confirmation before storing the device.
pub fn verify_pairing_initiator(
    hello: &InitiatorHello,
    expected_host_id: &str,
    expected_challenge: &str,
) -> Result<VerifiedInitiator, CryptoError> {
    let ephemeral_public =
        verify_initiator_signature(hello, expected_host_id, expected_challenge)?;
    Ok(VerifiedInitiator {
        device_id: hello.device_id.clone(),
        host_id: hello.host_id.clone(),
        challenge: hello.challenge.clone(),
        device_identity_public_key_hex: hello
            .device_identity_public_key_hex
            .clone(),
        ephemeral_public,
    })
}

/// Verifies both the signed handshake and the existing non-revoked registry
/// binding used for normal reconnects.
pub fn verify_authorized_initiator(
    hello: &InitiatorHello,
    expected_host_id: &str,
    expected_challenge: &str,
    registry: &DeviceRegistry,
) -> Result<VerifiedInitiator, CryptoError> {
    let ephemeral =
        verify_initiator_signature(hello, expected_host_id, expected_challenge)?;
    if !registry.is_authorized(
        &hello.device_id,
        &hello.device_identity_public_key_hex,
    ) {
        return Err(CryptoError::DeviceIdentityConflict);
    }
    Ok(VerifiedInitiator {
        device_id: hello.device_id.clone(),
        host_id: hello.host_id.clone(),
        challenge: hello.challenge.clone(),
        device_identity_public_key_hex: hello
            .device_identity_public_key_hex
            .clone(),
        ephemeral_public: ephemeral,
    })
}

pub fn create_responder_hello(
    identity: &SigningKey,
    host_id: &str,
    initiator: &InitiatorHello,
    ephemeral_public: &XPublicKey,
) -> Result<ResponderHello, CryptoError> {
    if host_id.trim().is_empty() || initiator.device_id.trim().is_empty() {
        return Err(CryptoError::InvalidHandshake);
    }
    if initiator.host_id != host_id
        || initiator.protocol_version != HANDSHAKE_PROTOCOL_VERSION
    {
        return Err(CryptoError::HandshakeContextMismatch);
    }
    verify_initiator_signature(initiator, host_id, &initiator.challenge)?;
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let mut hello = ResponderHello {
        protocol_version: HANDSHAKE_PROTOCOL_VERSION,
        host_id: host_id.to_owned(),
        device_id: initiator.device_id.clone(),
        host_identity_public_key_hex: encode_hex(&identity.verifying_key().to_bytes()),
        ephemeral_public_key_hex: encode_hex(ephemeral_public.as_bytes()),
        nonce_hex: encode_hex(&nonce),
        initiator_hash_hex: encode_hex(&initiator_hash(initiator)?),
        signature_hex: String::new(),
    };
    let claim = responder_claim(&hello)?;
    hello.signature_hex = encode_hex(&identity.sign(&claim).to_bytes());
    Ok(hello)
}

pub fn verify_responder_hello(
    hello: &ResponderHello,
    initiator: &InitiatorHello,
    expected_host_id: &str,
    expected_host_identity_public_key_hex: &str,
) -> Result<XPublicKey, CryptoError> {
    if hello.protocol_version != HANDSHAKE_PROTOCOL_VERSION
        || hello.host_id != expected_host_id
        || hello.device_id != initiator.device_id
        || hello.host_identity_public_key_hex
            != expected_host_identity_public_key_hex
        || hello.initiator_hash_hex != encode_hex(&initiator_hash(initiator)?)
    {
        return Err(CryptoError::HandshakeContextMismatch);
    }
    parse_hex::<32>(&hello.nonce_hex)?;
    let identity_bytes = parse_hex::<32>(&hello.host_identity_public_key_hex)?;
    let identity = VerifyingKey::from_bytes(&identity_bytes)
        .map_err(|_| CryptoError::InvalidHandshakeEncoding)?;
    let signature_bytes = parse_hex::<64>(&hello.signature_hex)?;
    let signature = Signature::from_bytes(&signature_bytes);
    identity
        .verify(&responder_claim(hello)?, &signature)
        .map_err(|_| CryptoError::InvalidHandshakeSignature)?;
    parse_ephemeral(&hello.ephemeral_public_key_hex)
}

pub fn authenticated_transcript(
    initiator: &InitiatorHello,
    responder: &ResponderHello,
) -> Result<Vec<u8>, CryptoError> {
    if responder.initiator_hash_hex != encode_hex(&initiator_hash(initiator)?) {
        return Err(CryptoError::HandshakeContextMismatch);
    }
    let mut transcript = Vec::new();
    push_field(&mut transcript, TRANSCRIPT_DOMAIN)?;
    push_field(&mut transcript, &initiator_claim(initiator)?)?;
    push_field(
        &mut transcript,
        &parse_hex::<64>(&initiator.signature_hex)?,
    )?;
    push_field(&mut transcript, &responder_claim(responder)?)?;
    push_field(
        &mut transcript,
        &parse_hex::<64>(&responder.signature_hex)?,
    )?;
    Ok(transcript)
}

fn verify_initiator_signature(
    hello: &InitiatorHello,
    expected_host_id: &str,
    expected_challenge: &str,
) -> Result<XPublicKey, CryptoError> {
    validate_context(&hello.device_id, expected_host_id, expected_challenge)?;
    if hello.protocol_version != HANDSHAKE_PROTOCOL_VERSION
        || hello.host_id != expected_host_id
        || hello.challenge != expected_challenge
        || hello.device_id.trim().is_empty()
    {
        return Err(CryptoError::HandshakeContextMismatch);
    }
    parse_hex::<32>(&hello.nonce_hex)?;
    let identity_bytes = parse_hex::<32>(&hello.device_identity_public_key_hex)?;
    let identity = VerifyingKey::from_bytes(&identity_bytes)
        .map_err(|_| CryptoError::InvalidHandshakeEncoding)?;
    let signature_bytes = parse_hex::<64>(&hello.signature_hex)?;
    let signature = Signature::from_bytes(&signature_bytes);
    identity
        .verify(&initiator_claim(hello)?, &signature)
        .map_err(|_| CryptoError::InvalidHandshakeSignature)?;
    parse_ephemeral(&hello.ephemeral_public_key_hex)
}

fn initiator_hash(hello: &InitiatorHello) -> Result<[u8; 32], CryptoError> {
    let mut signed = initiator_claim(hello)?;
    signed.extend_from_slice(&parse_hex::<64>(&hello.signature_hex)?);
    Ok(Sha256::digest(signed).into())
}

fn initiator_claim(hello: &InitiatorHello) -> Result<Vec<u8>, CryptoError> {
    let mut claim = Vec::new();
    push_field(&mut claim, INITIATOR_DOMAIN)?;
    claim.extend_from_slice(&hello.protocol_version.to_be_bytes());
    push_field(&mut claim, hello.device_id.as_bytes())?;
    push_field(&mut claim, hello.host_id.as_bytes())?;
    push_field(&mut claim, hello.challenge.as_bytes())?;
    push_field(
        &mut claim,
        &parse_hex::<32>(&hello.device_identity_public_key_hex)?,
    )?;
    push_field(
        &mut claim,
        &parse_hex::<32>(&hello.ephemeral_public_key_hex)?,
    )?;
    push_field(&mut claim, &parse_hex::<32>(&hello.nonce_hex)?)?;
    Ok(claim)
}

fn responder_claim(hello: &ResponderHello) -> Result<Vec<u8>, CryptoError> {
    let mut claim = Vec::new();
    push_field(&mut claim, RESPONDER_DOMAIN)?;
    claim.extend_from_slice(&hello.protocol_version.to_be_bytes());
    push_field(&mut claim, hello.host_id.as_bytes())?;
    push_field(&mut claim, hello.device_id.as_bytes())?;
    push_field(
        &mut claim,
        &parse_hex::<32>(&hello.host_identity_public_key_hex)?,
    )?;
    push_field(
        &mut claim,
        &parse_hex::<32>(&hello.ephemeral_public_key_hex)?,
    )?;
    push_field(&mut claim, &parse_hex::<32>(&hello.nonce_hex)?)?;
    push_field(&mut claim, &parse_hex::<32>(&hello.initiator_hash_hex)?)?;
    Ok(claim)
}

fn validate_context(
    device_id: &str,
    host_id: &str,
    challenge: &str,
) -> Result<(), CryptoError> {
    if device_id.trim().is_empty()
        || host_id.trim().is_empty()
        || challenge.len() < 32
    {
        Err(CryptoError::InvalidHandshake)
    } else {
        Ok(())
    }
}

fn parse_ephemeral(value: &str) -> Result<XPublicKey, CryptoError> {
    Ok(XPublicKey::from(parse_hex::<32>(value)?))
}

fn parse_hex<const N: usize>(value: &str) -> Result<[u8; N], CryptoError> {
    if value.len() != N * 2 {
        return Err(CryptoError::InvalidHandshakeEncoding);
    }
    let mut bytes = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(chunk)
            .map_err(|_| CryptoError::InvalidHandshakeEncoding)?;
        bytes[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| CryptoError::InvalidHandshakeEncoding)?;
    }
    Ok(bytes)
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

fn push_field(output: &mut Vec<u8>, value: &[u8]) -> Result<(), CryptoError> {
    let length: u32 = value
        .len()
        .try_into()
        .map_err(|_| CryptoError::InvalidHandshake)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        derive_session_keys, derive_shared_secret, generate_sas_code, KeyPair,
        PairedDeviceRecord, SessionCipher, SessionRole,
    };

    #[test]
    fn pairing_connection_challenge_binds_token_and_server_nonce() {
        let token = encode_hex(&[1_u8; 32]);
        let server = encode_hex(&[2_u8; 32]);
        let challenge =
            pairing_connection_challenge(&token, &server).unwrap();
        assert_eq!(challenge.len(), 64);
        assert_eq!(
            challenge,
            pairing_connection_challenge(&token, &server).unwrap()
        );
        assert_ne!(
            challenge,
            pairing_connection_challenge(
                &encode_hex(&[3_u8; 32]),
                &server,
            )
            .unwrap()
        );
        assert_ne!(
            challenge,
            pairing_connection_challenge(
                &token,
                &encode_hex(&[4_u8; 32]),
            )
            .unwrap()
        );
    }

    #[test]
    fn signed_handshake_derives_matching_directional_session() {
        let device_identity = SigningKey::generate(&mut OsRng);
        let host_identity = SigningKey::generate(&mut OsRng);
        let device_ephemeral = KeyPair::generate();
        let host_ephemeral = KeyPair::generate();
        let device_public = device_ephemeral.public;
        let host_public = host_ephemeral.public;
        let challenge = "0123456789abcdef0123456789abcdef";

        let initiator = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            challenge,
            &device_public,
        )
        .unwrap();
        let mut registry = DeviceRegistry::new();
        registry
            .register_device(PairedDeviceRecord {
                device_id: "phone-1".into(),
                device_name: "Phone".into(),
                public_key_hex: initiator
                    .device_identity_public_key_hex
                    .clone(),
                paired_at_ms: 1,
                last_seen_at_ms: 1,
                is_revoked: false,
            })
            .unwrap();
        let verified_device = verify_authorized_initiator(
            &initiator,
            "host-1",
            challenge,
            &registry,
        )
        .unwrap();
        assert_eq!(
            verified_device.ephemeral_public().as_bytes(),
            device_public.as_bytes()
        );

        let responder = create_responder_hello(
            &host_identity,
            "host-1",
            &initiator,
            &host_public,
        )
        .unwrap();
        let host_identity_hex = encode_hex(&host_identity.verifying_key().to_bytes());
        let verified_host_public = verify_responder_hello(
            &responder,
            &initiator,
            "host-1",
            &host_identity_hex,
        )
        .unwrap();
        assert_eq!(verified_host_public.as_bytes(), host_public.as_bytes());

        let transcript = authenticated_transcript(&initiator, &responder).unwrap();
        let device_shared = derive_shared_secret(
            device_ephemeral.secret,
            &verified_host_public,
            &transcript,
        )
        .unwrap();
        let host_shared = derive_shared_secret(
            host_ephemeral.secret,
            verified_device.ephemeral_public(),
            &transcript,
        )
        .unwrap();
        assert_eq!(device_shared, host_shared);
        assert_eq!(
            generate_sas_code(&device_shared, &transcript).unwrap(),
            generate_sas_code(&host_shared, &transcript).unwrap()
        );

        let device_keys = derive_session_keys(&device_shared, &transcript).unwrap();
        let host_keys = derive_session_keys(&host_shared, &transcript).unwrap();
        let mut device_cipher =
            SessionCipher::from_directional_keys(&device_keys, SessionRole::Initiator);
        let mut host_cipher =
            SessionCipher::from_directional_keys(&host_keys, SessionRole::Responder);
        let frame = device_cipher
            .encrypt_next(b"command", b"host-1/phone-1")
            .unwrap();
        assert_eq!(
            host_cipher
                .decrypt_next(&frame, b"host-1/phone-1")
                .unwrap(),
            b"command"
        );
    }

    #[test]
    fn handshake_rejects_tampering_wrong_challenge_and_revocation() {
        let device_identity = SigningKey::generate(&mut OsRng);
        let device_ephemeral = KeyPair::generate();
        let challenge = "0123456789abcdef0123456789abcdef";
        let mut initiator = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            challenge,
            &device_ephemeral.public,
        )
        .unwrap();
        assert!(matches!(
            verify_pairing_initiator(&initiator, "host-1", "different-challenge-value-000000"),
            Err(CryptoError::HandshakeContextMismatch)
        ));
        initiator.device_id = "phone-2".into();
        assert!(matches!(
            verify_pairing_initiator(&initiator, "host-1", challenge),
            Err(CryptoError::InvalidHandshakeSignature)
        ));

        initiator.device_id = "phone-1".into();
        let mut registry = DeviceRegistry::new();
        registry
            .register_device(PairedDeviceRecord {
                device_id: "phone-1".into(),
                device_name: "Phone".into(),
                public_key_hex: initiator
                    .device_identity_public_key_hex
                    .clone(),
                paired_at_ms: 1,
                last_seen_at_ms: 1,
                is_revoked: false,
            })
            .unwrap();
        registry.revoke_device("phone-1");
        assert!(matches!(
            verify_authorized_initiator(
                &initiator,
                "host-1",
                challenge,
                &registry
            ),
            Err(CryptoError::DeviceIdentityConflict)
        ));
    }

    #[test]
    fn signed_pairing_offer_binds_identity_endpoint_and_expiry() {
        let host_identity = SigningKey::generate(&mut OsRng);
        let host_ephemeral = KeyPair::generate();
        let now = 1_700_000_000_000_i64;
        let offer = SignedPairingOffer::create(
            &host_identity,
            "host-1",
            "Developer workstation",
            "192.0.2.10:45821",
            &encode_hex(&[7_u8; 32]),
            &encode_hex(host_ephemeral.public.as_bytes()),
            now,
            now + 60_000,
        )
        .unwrap();
        assert_eq!(
            offer.verify(now).unwrap(),
            host_identity.verifying_key()
        );

        let mut changed_endpoint = offer.clone();
        changed_endpoint.direct_endpoint = "attacker.example:45821".into();
        assert!(matches!(
            changed_endpoint.verify(now),
            Err(CryptoError::InvalidHandshakeSignature)
        ));

        let mut extended_expiry = offer.clone();
        extended_expiry.expires_at_ms += 1;
        assert!(matches!(
            extended_expiry.verify(now),
            Err(CryptoError::InvalidHandshakeSignature)
        ));

        assert!(matches!(
            offer.verify(
                offer.expires_at_ms + PAIRING_OFFER_CLOCK_SKEW_MS + 1
            ),
            Err(CryptoError::ExpiredPairingOffer)
        ));
    }
}
