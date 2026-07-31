# Runbook: Failed Credential Rotation and Staged Rollback

## Context
An automatic or manual credential rotation attempt failed (e.g. invalid API key, provider auth endpoint error).

## Safety Semantics
- Active agent sessions must NEVER change identity mid-turn.
- Old assignments are retained until new key activation is explicitly validated.

## Protocol
1. Host connector stages new credential record as `CREDENTIAL_STATUS_STAGED`.
2. Connector verifies the installed runtime advertises the requested
   authentication method without changing the active secret.
3. Connector activates only on a connector-managed isolated runtime and reads
   provider state back before committing the vault slot.
4. If validation, activation, or readback fails:
   - The prior encrypted secret is reactivated when runtime mutation may have
     occurred.
   - The staged secret remains staged for explicit retry or discard.
   - Active profile assignment remains pointing to previous working secret handle.
   - If runtime rollback fails, report `RollbackFailed`, stop automatic
     mutation, and require reconciliation.
5. User can inspect failure details and re-enroll or discard the staged
   credential.

Audit-database persistence and phone notification are required by `PLAN.md`
but are not implemented yet; operators must not infer those side effects from
the current return value.
