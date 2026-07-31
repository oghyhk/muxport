# OpenCode Server Adapter Contract

- **Status:** Managed-profile and transactional credential foundation implemented
- **Reviewed:** 2026-07-31
- **Primary sources:**
  - <https://opencode.ai/docs/server/>
  - <https://github.com/anomalyco/opencode/blob/dev/packages/sdk/js/src/gen/types.gen.ts>

## Supported operations

The built-in adapter uses OpenCode's documented HTTP server contract:

| Muxport operation | OpenCode route | Behavior |
|---|---|---|
| Probe | `GET /global/health` | Requires `healthy: true` and a non-empty version |
| Discover projects | `GET /project` | Rejects entries without a path |
| List sessions | `GET /session` and `GET /session/status` | Queries every discovered project directory and de-duplicates session IDs |
| Subscribe | `GET /global/event` | Parses global SSE and normalizes session, text-delta, completion, and permission events |
| Start session | `POST /session`, then `POST /session/:id/prompt_async` | Creates in the selected project and dispatches one text part |
| Send input | `POST /session/:id/prompt_async` | Resolves the session directory before mutation |
| Interrupt | `POST /session/:id/abort` | Requires a `true` response |
| Reply to approval | `POST /session/:id/permissions/:permissionID` | Maps allow to `once` and deny to `reject` |
| Discover auth methods | `GET /provider/auth` | Requires the selected provider to advertise the requested auth kind |
| Activate managed API key | `PUT /auth/:providerID` | Available only for a connector-created isolated profile |
| Provider readback | `GET /provider` | Requires the activated provider ID to appear in `connected` |

HTTP Basic authentication uses the documented `opencode` username and the
configured server password. Credentials are attached only as an Authorization
header and are never included in adapter events or logs.

## Managed profile boundary

- Every managed profile has a separate home, XDG data, configuration, cache,
  and state directory below the configured Muxport profiles root.
- Profile IDs cannot contain path separators or traversal components.
- Profile paths reject symbolic-link substitution and use mode `0700` on Unix.
- The OpenCode child environment is cleared before a minimal OS bootstrap
  allowlist and the isolated paths are applied. Unrelated provider variables
  are not inherited.
- Managed servers always launch with `--hostname 127.0.0.1`, a fixed selected
  port, and `OPENCODE_SERVER_PASSWORD`.
- The connector monitor owns the managed child. If it observes an exit, the
  next bounded reconciliation attempt launches the same executable, project,
  port, password, and isolated profile roots before probing and snapshotting.
- Local enrollment runs `opencode auth login --provider ...` inside the
  isolated environment. OpenCode, not Muxport, writes vendor auth state.
- Externally adopted servers remain fail-closed for every credential mutation.

Credential replacement is coordinated with the Muxport vault. The candidate
remains staged while the old encrypted secret stays active. The connector
checks the runtime-discovered auth schema, activates through the supported
route, reads provider state back, and only then commits the vault slot. Any
failure after mutation begins reactivates the prior secret. If rollback also
fails, the result is explicitly `RollbackFailed` and the vault remains staged
for recovery.

A deterministic child-exit fixture verifies same-profile restart and the
supervisor latches after five rapid restarts in sixty seconds. It requires an
operator restart instead of continuing an infinite crash loop. Surviving child
adoption after the connector itself is killed, graceful restart policy, and
provider/account validation before reopening mutations remain incomplete.

## Restart and reconciliation rules

- The authoritative state remains OpenCode plus Muxport's durable event journal;
  adapter maps are caches only.
- Session commands contain a session ID. If its directory is absent from the
  cache after restart, the adapter enumerates known project directories and
  reconstructs the mapping from `GET /session`.
- Approval commands carry both `session_id` and `approval_id`. This is required
  because OpenCode exposes a session-scoped reply route but no documented
  endpoint for listing pending permissions after restart.
- Transport errors, server errors, or invalid success bodies from mutating
  requests return `OutcomeUnknown`. Callers must reconcile before retrying.
- A successfully created session followed by an unconfirmed initial prompt is
  reported as partial/unknown, including the created session ID.
- The SSE parser accepts LF, CRLF, and CR line endings, handles chunk
  boundaries, ignores comments, bounds each source event to 1 MiB, and feeds a
  bounded 128-item channel. Stream end is not treated as success; the connector
  must snapshot and resubscribe.

## Deliberately unsupported

- Credential mutation on externally adopted OpenCode servers
- Claiming an account fingerprint when OpenCode exposes only provider-connected
  state
- Automatic quota/rate-limit rotation and multi-host bulk switching
- Steering with semantics distinct from a new user message
- Usage/quota reads

The repository has deterministic managed-launch, auth-schema, activation,
readback, and rollback tests. The live OpenCode 1.18.10 fixture confirms that a
runtime-advertised API-key provider survives two starts under one isolated
profile. It also found that OpenCode Go's `opencode` provider is not advertised
by `/provider/auth` on that version. Muxport therefore keeps Go activation
fail-closed instead of hard-coding the ID or bypassing schema discovery.
OpenCode Go validation with a non-production authorized key remains required
before this path can be used for Go rotation.

## Normalization boundary

The adapter currently emits Muxport events for:

- `session.created`, `session.updated`, `session.deleted`, `session.status`,
  and `session.idle`;
- incremental text from `message.part.updated` and final markers from completed
  assistant `message.updated` events;
- `permission.updated` and `permission.replied`.

Unknown OpenCode events are ignored because the current Muxport protocol has no
safe raw-source-event envelope. They must not be misrepresented as audit
events. Adding a sanitized, versioned raw event type requires a separate
protocol decision.
