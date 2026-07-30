# Mobile sync and recovery boundary

The Flutter recovery model treats its cache as a stale projection, never as
runtime truth. Deserializing a cached host always produces `cachedStale`,
regardless of the phase that existed before process death.

## Recovery sequence

1. Render cached, non-secret state with mutations disabled.
2. Authenticate the connector against the cached host ID and pinned public key.
3. Negotiate the supported protocol version.
4. Request replay from the last atomically persisted host epoch and sequence.
5. Replace the complete projection with an authoritative snapshot if the host
   epoch changed or the requested journal position is no longer retained.
6. Apply contiguous events in order. Persist the resulting projection and
   cursor atomically before acknowledging the cursor to the connector.
7. Enable mutations only after replay completes or a valid snapshot replaces
   the cache.

An event with a repeated event ID is ignored. An event carrying an older source
object version consumes its valid journal position but cannot replace a newer
object. An epoch change, sequence gap, identity mismatch, or incompatible
protocol version disables mutations.

## Pending operations

The cache retains only operation metadata and idempotency keys, never provider
secrets or plaintext command inputs. Every non-terminal operation is queried
after reconnect. Local dispatch, connector persistence, and source
acknowledgement remain visibly pending; only the connector's terminal
`succeeded` state is presented as success. Unknown outcomes remain
`reconciliationRequired` and are not automatically replayed.

## Current implementation boundary

`apps/mobile/lib/state/mobile_sync_state.dart` implements and tests this pure
state transition layer and a versioned JSON cache representation. Atomic
device storage, secure device-identity storage, authenticated transport, and UI
binding are separate unfinished layers. Until those are connected, the
existing screens remain explicitly marked as demo data.
