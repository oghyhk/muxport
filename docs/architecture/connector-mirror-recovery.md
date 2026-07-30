# Connector Source Mirror and Restart Recovery

- **Status:** Implemented local OpenCode and Codex foundation
- **Reviewed:** 2026-07-30

## Purpose

The connector daemon maintains one non-secret projection for multiple local
agent runtimes. OpenCode and Codex are monitored independently and feed a
single journal owner, so one unavailable or restarting runtime does not block,
erase, or reorder state from the other. The projection is derived from
authoritative vendor snapshots plus normalized live events; it is not a
replacement for either vendor's own state.

The direct/relay mobile transport is not implemented yet. Until it is
authenticated and encrypted, the daemon deliberately reports the connector as
`degraded` even when every local source mirror is healthy.

## Startup sequence

1. Acquire an OS-owned exclusive connector lock before opening any store.
2. Open the event database in WAL mode and run SQLite integrity checking.
3. Load the latest persisted host snapshot. A configured host ID that conflicts
   with the persisted ID is fatal instead of silently changing identity.
4. Replay journal events newer than the snapshot into the in-memory projection.
5. Persist a new recovery snapshot at the current journal sequence.
6. Start independent OpenCode and Codex monitor tasks.
7. Each monitor probes its runtime and fetches authoritative projects, sessions,
   and statuses.
8. Persist that runtime baseline before opening its event stream.
9. Open the runtime event stream, then fetch and persist a second baseline while
   the stream buffers source events.
10. Begin consuming buffered live events through the shared journal owner.

The double baseline closes the list-before-subscribe race for state represented
by host snapshots. Runtime updates share a bounded queue, but only the main
daemon task writes the journal and projection; SQLite sequence order therefore
remains deterministic across sources.

The host ID persisted here is only a stable logical identifier. It is not a
cryptographic host identity and must not be used as proof of pairing; durable
signing keys and device certificates remain separate required work.

## Live durability order

Each normalized event follows this order:

1. Add connector runtime correlation.
2. Append the event to the durable journal and receive its monotonic sequence.
3. Apply it to the in-memory projection.
4. Persist a snapshot for state-changing events, and at least every 100 output
   deltas per runtime.

If journal append fails, the projection is not changed. If snapshot persistence
fails after append, restart recovery replays the durable event after the older
snapshot.

Compaction never resets the connector sequence. On reopen, the journal restores
SQLite's AUTOINCREMENT high-water mark even when every event row was deleted.
A client cursor older than the retained window receives an explicit gap instead
of an empty replay and must replace its cache from an authoritative snapshot.
A cursor ahead of the connector is rejected as invalid.

Each monitor takes an authoritative health/session snapshot every 30 seconds to
repair incomplete or ambiguous source events. Reconciliation replaces only the
target runtime's sessions; state from other runtimes remains intact.
High-frequency text deltas remain in the journal and are not copied into the
host snapshot.

## Disconnect and restart behavior

- A source error or clean event-stream end marks only that runtime degraded.
- No source mutation is automatically retried or replayed.
- Each runtime reconnects independently with exponential backoff from 1 second
  to a 30-second cap.
- Recovery requires a fresh probe, two snapshot baselines around a new
  subscription, and persisted source state.
- Repeated failed attempts do not flood the journal with duplicate degraded
  events; a new event is recorded after a real recovery and later failure.
- Codex App Server loss discards process-bound active-turn IDs and approval
  callbacks. Durable threads are rediscovered with `thread/list`.
- OpenCode is externally managed and is never killed by its HTTP adapter.
- Shutdown signals every monitor, drops the update receiver to release blocked
  producers, closes or terminates the owned Codex child, and writes a final
  snapshot.
- A concurrent connector using the same lock path fails closed. The marker file
  is retained because only the live OS lock proves ownership; normal exit,
  crash, and reboot release the lock without stale-file deletion.

## Configuration

| Environment variable | Meaning | Default |
|---|---|---|
| `MUXPORT_STATE_DB` | Connector event/snapshot SQLite path | `muxport-state.db` |
| `MUXPORT_LOCK_FILE` | Connector OS-lock marker path | `<MUXPORT_STATE_DB>.lock` |
| `MUXPORT_COMMAND_DB` | Durable command idempotency/result SQLite path | `muxport-commands.db` |
| `MUXPORT_PAIRING_DB` | Pairing challenges and signed device registry SQLite path | `muxport-pairing.db` |
| `MUXPORT_HOST_ID` | Optional stable logical host ID; must match persisted state | generated once |
| `MUXPORT_HOSTNAME` | Display hostname | OS hostname or `unnamed-host` |
| `MUXPORT_OPENCODE_URL` | OpenCode server base URL | `http://127.0.0.1:4096` |
| `MUXPORT_OPENCODE_PASSWORD` | OpenCode HTTP Basic password | unset |
| `MUXPORT_OPENCODE_RUNTIME_ID` | Stable OpenCode runtime correlation ID | `opencode-local` |
| `MUXPORT_CODEX_PATH` | Codex executable used to own an App Server child | `codex` |
| `MUXPORT_CODEX_RUNTIME_ID` | Stable Codex runtime correlation ID | `codex-local` |

Passwords are read from the environment and are not persisted in snapshots,
events, or logs. Codex uses the account state already visible to its isolated
process environment; the connector does not mutate that state.

## Known boundaries

- OpenCode snapshots cover projects, sessions, and statuses. Full message
  history, todos, diffs, usage, and pending-permission enumeration still need
  supported source reads and version fixtures.
- OpenCode does not document a pending-permission list endpoint, so approval
  commands carry `session_id` for restart-safe routing.
- Codex active-turn and pending-approval state is process-bound and cannot be
  reconstructed after App Server loss from the currently used stable reads.
- Relay/direct delivery, client acknowledgements, snapshot transfer, command
  dispatch, and mobile application state replacement are not connected to this
  mirror yet.
- Managed runtime restart and credential/profile isolation are not implemented.
  OpenCode is observed externally; Codex owns one child using the current
  process account state.
