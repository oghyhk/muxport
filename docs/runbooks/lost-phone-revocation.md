# Runbook: Lost Phone & Device Key Revocation

## Context
A paired mobile phone has been lost, stolen, or compromised.

## Verification & Facts Preservation
1. Access an existing authorized paired client or log into the host connector CLI:
   `muxport-connector devices list`
2. Note the `device_id` and public key fingerprint of the lost device.

## Remediation Protocol
1. Revoke device certificate from host vault:
   `muxport-connector devices revoke --device-id <DEVICE_ID>`
2. Host connector removes the public key from the E2EE whitelist.
3. The lost phone's E2EE session keys are immediately invalidated; any subsequent encrypted frame sent by the lost device will be rejected at the AEAD layer.
4. Upstream provider credentials remain untouched on the host vault.
