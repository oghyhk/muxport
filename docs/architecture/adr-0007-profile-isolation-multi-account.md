# ADR-0007: Account Identity Routing and Profile Isolation

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Users require concurrent account identities for OpenCode Go and Codex (e.g. personal vs. work API keys / accounts) without cross-account profile bleed.

## Decision

1. **Vendor Profile Isolation:**
   - Managed runtime instances operate under separate state directories:
     - OpenCode: `<profiles_root>/opencode/<profile_id>/`
     - Codex: `<profiles_root>/codex/<profile_id>/`
   - OpenCode receives isolated home and XDG roots and does not inherit
     arbitrary provider environment variables.
   - Externally adopted runtimes are read/control-only for credentials.
2. **Session Identity Immutability:**
   - Every session records an immutable `session_binding` containing the `credential_profile_id` under which it was created.
   - Changing default credential assignment or switching active accounts applies ONLY to newly created sessions by default. Active turns are never silently moved to another identity.
3. **Idempotent Multi-Host Switching:**
   - Bulk credential switching tracks per-host results as separate child operations with honest partial-failure reports.

## Consequences

- Managed OpenCode sessions carry their bound profile ID into snapshots and
  normalized session events.
- The design prevents intentional cross-profile state reuse. Complete
  cross-platform proof still requires Windows ACL enforcement, real OpenCode
  compatibility fixtures, Codex profile isolation, and crash-adoption tests.
