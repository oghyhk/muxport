use crate::{
    ConfirmingParty, PairingStore, PairingStoreError,
};
use ed25519_dalek::SigningKey;
use muxport_crypto::{
    authenticated_transcript, create_responder_hello,
    derive_session_keys, derive_shared_secret, generate_sas_code,
    verify_pairing_initiator, CryptoError, EncryptedFrame, InitiatorHello,
    KeyPair, PairedDeviceRecord, ResponderHello, SessionCipher, SessionRole,
    SignedPairingOffer,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

const MAX_PENDING_OFFERS: usize = 8;

#[derive(Error, Debug)]
pub enum PairingCoordinatorError {
    #[error(transparent)]
    Store(#[from] PairingStoreError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error("pairing coordinator configuration is invalid")]
    InvalidConfiguration,
    #[error("pairing offer is unknown, expired, or no longer active")]
    OfferUnavailable,
    #[error("too many pairing offers are already active")]
    OfferCapacityReached,
}

struct PendingOffer {
    key_pair: KeyPair,
    expires_at_ms: i64,
}

pub struct PairingClaimResult {
    pairing_id: String,
    responder: ResponderHello,
    host_sas: String,
    transcript: Vec<u8>,
    transport_cipher: SessionCipher,
}

impl PairingClaimResult {
    pub fn pairing_id(&self) -> &str {
        &self.pairing_id
    }

    pub fn responder(&self) -> &ResponderHello {
        &self.responder
    }

    /// This value is for the trusted local host display only. A transport
    /// response must never send it to the phone; the phone derives its own
    /// value from the authenticated transcript.
    pub fn host_sas(&self) -> &str {
        &self.host_sas
    }

    pub fn decrypt_phone_record(
        &mut self,
        frame: &EncryptedFrame,
    ) -> Result<Vec<u8>, CryptoError> {
        self.transport_cipher.decrypt_next(frame, &self.transcript)
    }

    pub fn encrypt_host_record(
        &mut self,
        plaintext: &[u8],
    ) -> Result<EncryptedFrame, CryptoError> {
        self.transport_cipher.encrypt_next(plaintext, &self.transcript)
    }
}

pub struct PairingCoordinator {
    store: PairingStore,
    host_identity: Arc<SigningKey>,
    host_id: String,
    hostname: String,
    direct_endpoint: String,
    pending_offers: HashMap<String, PendingOffer>,
}

impl PairingCoordinator {
    pub fn new(
        store: PairingStore,
        host_identity: Arc<SigningKey>,
        host_id: impl Into<String>,
        hostname: impl Into<String>,
        direct_endpoint: impl Into<String>,
    ) -> Result<Self, PairingCoordinatorError> {
        let host_id = host_id.into();
        let hostname = hostname.into();
        let direct_endpoint = direct_endpoint.into();
        if host_id.trim().is_empty()
            || hostname.trim().is_empty()
            || direct_endpoint.trim().is_empty()
        {
            return Err(PairingCoordinatorError::InvalidConfiguration);
        }
        Ok(Self {
            store,
            host_identity,
            host_id,
            hostname,
            direct_endpoint,
            pending_offers: HashMap::new(),
        })
    }

    pub fn create_offer(
        &mut self,
        ttl: Duration,
    ) -> Result<SignedPairingOffer, PairingCoordinatorError> {
        self.discard_expired_offers(chrono::Utc::now().timestamp_millis());
        if self.pending_offers.len() >= MAX_PENDING_OFFERS {
            return Err(PairingCoordinatorError::OfferCapacityReached);
        }
        let key_pair = KeyPair::generate();
        let ephemeral_public_key_hex = encode_hex(key_pair.public.as_bytes());
        let offer = self.store.create_signed_offer(
            &self.host_id,
            &self.hostname,
            &self.direct_endpoint,
            &ephemeral_public_key_hex,
            ttl,
            &self.host_identity,
        )?;
        self.pending_offers.insert(
            offer.rendezvous_token.clone(),
            PendingOffer {
                key_pair,
                expires_at_ms: offer.expires_at_ms,
            },
        );
        Ok(offer)
    }

    pub fn claim(
        &mut self,
        rendezvous_token: &str,
        expected_challenge: &str,
        initiator: &InitiatorHello,
        device_name: &str,
    ) -> Result<PairingClaimResult, PairingCoordinatorError> {
        let now = chrono::Utc::now().timestamp_millis();
        self.discard_expired_offers(now);
        let pending = self
            .pending_offers
            .get(rendezvous_token)
            .ok_or(PairingCoordinatorError::OfferUnavailable)?;
        if pending.expires_at_ms < now {
            return Err(PairingCoordinatorError::OfferUnavailable);
        }
        let verified = verify_pairing_initiator(
            initiator,
            &self.host_id,
            expected_challenge,
        )?;
        // Do not consume the in-memory secret until the identity signature and
        // complete offer context have both verified.
        let pending = self
            .pending_offers
            .remove(rendezvous_token)
            .ok_or(PairingCoordinatorError::OfferUnavailable)?;
        let result = self.finish_claim(
            pending,
            rendezvous_token,
            initiator,
            verified,
            device_name,
        );
        if result.is_err() {
            // The X25519 secret has been consumed or dropped, so the durable
            // token must not continue to advertise a claimable offer.
            let _ = self.store.cancel_by_token(rendezvous_token);
        }
        result
    }

    pub fn cancel_offer(
        &mut self,
        rendezvous_token: &str,
    ) -> Result<bool, PairingCoordinatorError> {
        let removed = self.pending_offers.remove(rendezvous_token).is_some();
        let cancelled = self.store.cancel_by_token(rendezvous_token)?;
        Ok(removed || cancelled)
    }

    fn finish_claim(
        &mut self,
        pending: PendingOffer,
        rendezvous_token: &str,
        initiator: &InitiatorHello,
        verified: muxport_crypto::VerifiedInitiator,
        device_name: &str,
    ) -> Result<PairingClaimResult, PairingCoordinatorError> {
        let KeyPair { secret, public } = pending.key_pair;
        let responder = create_responder_hello(
            &self.host_identity,
            &self.host_id,
            initiator,
            &public,
        )?;
        let transcript = authenticated_transcript(initiator, &responder)?;
        let shared_secret = derive_shared_secret(
            secret,
            verified.ephemeral_public(),
            &transcript,
        )?;
        let host_sas = generate_sas_code(&shared_secret, &transcript)?;
        let directional_keys =
            derive_session_keys(&shared_secret, &transcript)?;
        let transport_cipher = SessionCipher::from_directional_keys(
            &directional_keys,
            SessionRole::Responder,
        );
        let pairing_id = self.store.claim_verified_initiator(
            rendezvous_token,
            &verified,
            device_name,
            &host_sas,
        )?;
        Ok(PairingClaimResult {
            pairing_id,
            responder,
            host_sas,
            transcript,
            transport_cipher,
        })
    }

    pub fn confirm(
        &mut self,
        pairing_id: &str,
        sas: &str,
        party: ConfirmingParty,
    ) -> Result<bool, PairingCoordinatorError> {
        let both_confirmed =
            self.store.confirm_sas(pairing_id, sas, party)?;
        if both_confirmed {
            self.store.finalize(
                pairing_id,
                &self.host_id,
                &self.host_identity,
            )?;
        }
        Ok(both_confirmed)
    }

    pub fn finalize(
        &mut self,
        pairing_id: &str,
    ) -> Result<PairedDeviceRecord, PairingCoordinatorError> {
        Ok(self.store.finalize(
            pairing_id,
            &self.host_id,
            &self.host_identity,
        )?)
    }

    pub fn load_registry(
        &self,
    ) -> Result<muxport_crypto::DeviceRegistry, PairingCoordinatorError> {
        Ok(self
            .store
            .load_registry(&self.host_id, &self.host_identity)?)
    }

    pub fn revoke_device(
        &mut self,
        device_id: &str,
    ) -> Result<bool, PairingCoordinatorError> {
        Ok(self.store.revoke_device(
            &self.host_id,
            &self.host_identity,
            device_id,
        )?)
    }

    fn discard_expired_offers(&mut self, now_ms: i64) {
        self.pending_offers
            .retain(|_, pending| pending.expires_at_ms >= now_ms);
    }
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
    use muxport_crypto::{
        create_initiator_hello, derive_session_keys,
        verify_responder_hello, SessionCipher, SessionRole,
    };
    use rand::rngs::OsRng;

    fn coordinator() -> (PairingCoordinator, Arc<SigningKey>) {
        let host_identity =
            Arc::new(SigningKey::generate(&mut OsRng));
        let store = PairingStore::open_in_memory().unwrap();
        (
            PairingCoordinator::new(
                store,
                Arc::clone(&host_identity),
                "host-1",
                "Developer workstation",
                "192.0.2.10:45821",
            )
            .unwrap(),
            host_identity,
        )
    }

    #[test]
    fn derived_sas_and_dual_confirmation_finalize_registry() {
        let (mut coordinator, host_identity) = coordinator();
        let offer = coordinator
            .create_offer(Duration::from_secs(60))
            .unwrap();
        assert_eq!(
            offer
                .verify(chrono::Utc::now().timestamp_millis())
                .unwrap(),
            host_identity.verifying_key()
        );

        let device_identity = SigningKey::generate(&mut OsRng);
        let device_ephemeral = KeyPair::generate();
        let device_public = device_ephemeral.public;
        let initiator = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            &offer.rendezvous_token,
            &device_public,
        )
        .unwrap();
        let mut claim = coordinator
            .claim(
                &offer.rendezvous_token,
                &offer.rendezvous_token,
                &initiator,
                "Phone",
            )
            .unwrap();
        let host_ephemeral = verify_responder_hello(
            claim.responder(),
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
            authenticated_transcript(&initiator, claim.responder()).unwrap();
        let device_shared = derive_shared_secret(
            device_ephemeral.secret,
            &host_ephemeral,
            &transcript,
        )
        .unwrap();
        let device_sas =
            generate_sas_code(&device_shared, &transcript).unwrap();
        assert_eq!(device_sas, claim.host_sas());
        let device_keys =
            derive_session_keys(&device_shared, &transcript).unwrap();
        let mut device_cipher = SessionCipher::from_directional_keys(
            &device_keys,
            SessionRole::Initiator,
        );
        let encrypted_confirmation = device_cipher
            .encrypt_next(b"confirmed", &transcript)
            .unwrap();
        assert_eq!(
            claim
                .decrypt_phone_record(&encrypted_confirmation)
                .unwrap(),
            b"confirmed"
        );
        let encrypted_ack =
            claim.encrypt_host_record(b"accepted").unwrap();
        assert_eq!(
            device_cipher
                .decrypt_next(&encrypted_ack, &transcript)
                .unwrap(),
            b"accepted"
        );

        assert!(!coordinator
            .confirm(
                claim.pairing_id(),
                &device_sas,
                ConfirmingParty::Phone,
            )
            .unwrap());
        assert!(matches!(
            coordinator.finalize(claim.pairing_id()),
            Err(PairingCoordinatorError::Store(
                PairingStoreError::ConfirmationIncomplete
            ))
        ));
        assert!(coordinator
            .confirm(
                claim.pairing_id(),
                claim.host_sas(),
                ConfirmingParty::Host,
            )
            .unwrap());
        let record = coordinator.finalize(claim.pairing_id()).unwrap();
        assert_eq!(record.device_id, "phone-1");
        assert!(coordinator
            .load_registry()
            .unwrap()
            .is_authorized("phone-1", &record.public_key_hex));

        let host_keys =
            derive_session_keys(&device_shared, &transcript).unwrap();
        let mut device_cipher = SessionCipher::from_directional_keys(
            &device_keys,
            SessionRole::Initiator,
        );
        let mut host_cipher = SessionCipher::from_directional_keys(
            &host_keys,
            SessionRole::Responder,
        );
        let frame = device_cipher
            .encrypt_next(b"paired", b"pairing-test")
            .unwrap();
        assert_eq!(
            host_cipher
                .decrypt_next(&frame, b"pairing-test")
                .unwrap(),
            b"paired"
        );
    }

    #[test]
    fn invalid_claim_does_not_consume_offer_but_successful_claim_does() {
        let (mut coordinator, _) = coordinator();
        let offer = coordinator
            .create_offer(Duration::from_secs(60))
            .unwrap();
        let device_identity = SigningKey::generate(&mut OsRng);
        let device_ephemeral = KeyPair::generate();
        let mut initiator = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            &offer.rendezvous_token,
            &device_ephemeral.public,
        )
        .unwrap();
        let valid = initiator.clone();
        initiator.device_id = "attacker".into();
        assert!(matches!(
            coordinator.claim(
                &offer.rendezvous_token,
                &offer.rendezvous_token,
                &initiator,
                "Attacker"
            ),
            Err(PairingCoordinatorError::Crypto(
                CryptoError::InvalidHandshakeSignature
            ))
        ));
        coordinator
            .claim(
                &offer.rendezvous_token,
                &offer.rendezvous_token,
                &valid,
                "Phone",
            )
            .unwrap();
        assert!(matches!(
            coordinator.claim(
                &offer.rendezvous_token,
                &offer.rendezvous_token,
                &valid,
                "Phone"
            ),
            Err(PairingCoordinatorError::OfferUnavailable)
        ));
    }

    #[test]
    fn active_offer_cap_and_cancellation_bound_pairing_resources() {
        let (mut coordinator, _) = coordinator();
        let mut offers = Vec::new();
        for _ in 0..MAX_PENDING_OFFERS {
            offers.push(
                coordinator
                    .create_offer(Duration::from_secs(60))
                    .unwrap(),
            );
        }
        assert!(matches!(
            coordinator.create_offer(Duration::from_secs(60)),
            Err(PairingCoordinatorError::OfferCapacityReached)
        ));

        let cancelled = &offers[0];
        assert!(coordinator
            .cancel_offer(&cancelled.rendezvous_token)
            .unwrap());
        assert!(!coordinator
            .cancel_offer(&cancelled.rendezvous_token)
            .unwrap());
        coordinator
            .create_offer(Duration::from_secs(60))
            .unwrap();

        let device_identity = SigningKey::generate(&mut OsRng);
        let device_ephemeral = KeyPair::generate();
        let initiator = create_initiator_hello(
            &device_identity,
            "phone-1",
            "host-1",
            &cancelled.rendezvous_token,
            &device_ephemeral.public,
        )
        .unwrap();
        assert!(matches!(
            coordinator.claim(
                &cancelled.rendezvous_token,
                &cancelled.rendezvous_token,
                &initiator,
                "Phone"
            ),
            Err(PairingCoordinatorError::OfferUnavailable)
        ));
    }
}
