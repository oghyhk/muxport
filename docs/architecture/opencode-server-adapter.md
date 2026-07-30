# OpenCode Server Adapter Contract

- **Status:** Implemented foundation; credential isolation remains blocked
- **Reviewed:** 2026-07-30
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

HTTP Basic authentication uses the documented `opencode` username and the
configured server password. Credentials are attached only as an Authorization
header and are never included in adapter events or logs.

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

- Profile-bound session creation
- Credential validation, activation, or rotation
- Steering with semantics distinct from a new user message
- Usage/quota reads

OpenCode exposes an auth mutation route, but using it directly would not prove
per-account process/state isolation, session identity immutability, validation,
or rollback. These operations must remain fail-closed until a managed-runtime
compatibility fixture proves those properties for the installed OpenCode
version.

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
