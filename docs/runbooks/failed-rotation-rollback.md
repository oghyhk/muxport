# Runbook: Failed Credential Rotation and Staged Rollback

## Context
An automatic or manual credential rotation attempt failed (e.g. invalid API key, provider auth endpoint error).

## Safety Semantics
- Active agent sessions must NEVER change identity mid-turn.
- Old assignments are retained until new key activation is explicitly validated.

## Protocol
1. Host connector stages new credential record as `CREDENTIAL_STATUS_STAGED`.
2. Connector runs non-destructive validation probe against provider (`validate_credential`).
3. If probe fails or returns 401/403:
   - Staged assignment is immediately cancelled.
   - Active profile assignment remains pointing to previous working secret handle.
   - Reason logged in `audit.db` as `ROTATION_FAILED_ROLLED_BACK`.
   - Notification trigger sent to phone UI.
4. User can inspect failure details and re-enroll valid credential.
