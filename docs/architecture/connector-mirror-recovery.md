# Connector Source Mirror and Restart Recovery

- **Status:** Implemented local OpenCode foundation
- **Reviewed:** 2026-07-30

## Purpose

The connector daemon maintains a non-secret projection of OpenCode runtime and
session state. The projection is derived from authoritative HTTP snapshots and
normalized SSE events; it is not a replacement for OpenCode's own state.

The direct/relay mobile transport is not implemented yet. Until it is
authenticated and encrypted, the daemon deliberately reports the connector as
`degraded` even when local OpenCode mirroring is healthy.

## Startup sequence

1. Open the event database in WAL mode and run SQLite integrity checking.
2. Load the latest persisted host snapshot. A configured host ID that conflicts
   with the persisted ID is fatal instead of silently changing identity.
3. Replay journal events newer than the snapshot into the in-memory projection.
4. Persist a new recovery snapshot at the current journal sequence.
5. Probe OpenCode and fetch authoritative projects, sessions, and statuses.
6. Persist that baseline before opening the event stream.
7. Open global SSE, then fetch and persist a second baseline while the HTTP
   response buffers events. This closes the list-before-subscribe race for
   session and status state.
8. Begin consuming the buffered live stream.

The host ID persisted here is only a stable logical identifier. It is not a
cryptographic host identity and must not be used as proof of pairing; durable
signing keys and device certificates remain separate required work.

## Live durability order

Each normalized event follows this order:

1. Add connector runtime correlation.
2. Append the event to the durable journal and receive its monotonic sequence.
3. Apply it to the in-memory projection.
4. Persist a snapshot for state-changing events, and at least every 100 output
   deltas.

If journal append fails, the projection is not changed. If snapshot persistence
fails after append, restart recovery replays the durable event after the older
snapshot.

The daemon takes an authoritative health/session snapshot every 30 seconds to
repair incomplete or ambiguous source events. High-frequency text deltas remain
in the journal and are not copied into the host snapshot.

## Disconnect behavior

- A source error or clean SSE end marks OpenCode and the connector degraded.
- No source mutation is automatically retried or replayed.
- Reconnect uses exponential backoff from 1 second to a 30-second cap.
- Recovery requires a fresh health probe, two snapshot baselines around a new
  subscription, and a new persisted snapshot.
- Repeated failed reconnect attempts do not flood the journal with duplicate
  degraded-state events.
- Shutdown remains responsive during streaming and backoff, and writes a final
  snapshot before exit.

## Configuration

| Environment variable | Meaning | Default |
|---|---|---|
| `MUXPORT_STATE_DB` | Connector event/snapshot SQLite path | `muxport-state.db` |
| `MUXPORT_HOST_ID` | Optional stable logical host ID; must match persisted state | generated once |
| `MUXPORT_HOSTNAME` | Display hostname | OS hostname or `unnamed-host` |
| `MUXPORT_OPENCODE_URL` | OpenCode server base URL | `http://127.0.0.1:4096` |
| `MUXPORT_OPENCODE_PASSWORD` | OpenCode HTTP Basic password | unset |
| `MUXPORT_OPENCODE_RUNTIME_ID` | Stable runtime correlation ID | `opencode-local` |
| `MUXPORT_CODEX_PATH` | Codex executable path for conservative probing | `codex` |

Passwords are read from the environment and are not persisted in snapshots,
events, or logs.

## Known boundaries

- The mirror currently snapshots projects, sessions, and statuses. Full message
  history, todos, diffs, usage, and pending-permission enumeration still need
  supported source reads and version fixtures.
- OpenCode does not document a pending-permission list endpoint, so approval
  commands carry `session_id` for restart-safe routing.
- Relay/direct delivery, client acknowledgements, snapshot transfer, and mobile
  application state replacement are not connected to this mirror yet.
- Managed OpenCode process restart and credential/profile isolation are not
  implemented; an external runtime is observed but never restarted by this
  slice.
