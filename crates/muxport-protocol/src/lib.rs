pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/muxport.protocol.v1.rs"));
}

pub use v1::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_serialization() {
        use prost::Message;

        let envelope = MuxportEnvelope {
            header: Some(EnvelopeHeader {
                protocol_version: 1,
                sender_id: "host_1".into(),
                recipient_id: "phone_1".into(),
                boot_epoch: 100,
                sequence: 1,
                timestamp_ms: 1700000000000,
                idempotency_key: "cmd_123".into(),
            }),
            payload: Some(muxport_envelope::Payload::Snapshot(HostSnapshot {
                host_id: "host_1".into(),
                hostname: "my-vps".into(),
                connector_state: ConnectorState::Ready as i32,
                runtimes: vec![],
                credential_profiles: vec![],
                active_sessions: vec![],
                snapshot_sequence: 1,
            })),
        };

        let mut buf = Vec::new();
        envelope.encode(&mut buf).expect("encoding failed");
        let decoded = MuxportEnvelope::decode(&buf[..]).expect("decoding failed");

        assert_eq!(decoded.header.unwrap().sender_id, "host_1");
    }
}
