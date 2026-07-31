# Runbook: Lost Phone & Device Key Revocation

## Context
A paired mobile phone has been lost, stolen, or compromised.

## Verification & Facts Preservation
1. From the same operating-system account as the connector, list the signed
   pairing registry:
   `muxport-connector pairing-list <HOST_ID>`
2. Note the `device_id` and public key fingerprint of the lost device. Do not
   alter or delete the pairing database/WAL while investigating.

## Remediation Protocol
1. Revoke the device from the signed host registry:
   `muxport-connector pairing-revoke <HOST_ID> <DEVICE_ID>`
2. Restart the supervised connector immediately. The revocation is durable,
   but an already-running listener retains its in-memory registry until it is
   restarted; established phone connections end when that listener stops.
3. Confirm the device appears as `revoked` with `pairing-list`, then verify an
   attempt to reconnect from that device is rejected before a secure session
   is created.
4. Upstream provider credentials and existing agent session data remain
   untouched. Re-pair a replacement phone as a new device.
