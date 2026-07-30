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
state transition layer. `mobile_cache_store.dart` persists the non-secret host
list, projections, cursors, recent event IDs, and pending operation IDs under
the platform application-support directory.

Each save creates a new checksummed generation and flushes it before the
previous generation can be pruned. Startup selects the newest valid generation
and falls back across a torn or corrupt newest write. At least two verified
generations are retained. If every generation is invalid, recovery blocks
writes and preserves the files instead of replacing them with an empty cache.
Known secret-shaped fields are rejected as defense in depth; the longer-term
protocol must replace generic snapshot maps with generated redacted types.

Protected device-identity storage is implemented as described in
`mobile-device-identity.md`. Authenticated transport and source-backed UI
binding remain separate unfinished layers.

The application bootstrap now opens the generation cache and protected device
identity before showing navigation. Cache corruption and identity lock are
represented independently, cached hosts render as stale, pending operation IDs
are visible as requiring reconciliation, and host controls stay disabled.
Diagnostics reports the transport and leak audit as not connected/not run
instead of presenting placeholder success. Session, approval, and credential
screens still contain illustrative data, are marked as unwired, and cannot
dispatch actions.
