# Durable Command Dispatch

- **Status:** Implemented local dispatch boundary; transport wiring pending
- **Reviewed:** 2026-07-30

## Purpose

All future direct and relay command transports must enter the connector through
one `CommandRouter`. The router owns adapter selection and a separate
SQLite/WAL command ledger. Transport code is not allowed to invoke OpenCode,
Codex, the vault, or the process supervisor directly.

## Dispatch order

1. Hash the canonical protobuf command bytes with SHA-256.
2. Validate and reserve the caller's idempotency key in the durable ledger.
3. Reject the same key if it is later paired with different command bytes.
4. Persist `Dispatched` before invoking an adapter.
5. Execute the command once.
6. Persist the complete terminal `CommandResult`.
7. Return the persisted result for every later duplicate, including duplicates
   received after the original deadline.

Expired commands are rejected before reservation and never reach an adapter.
The ledger uses WAL, full synchronization, a busy timeout, schema migration,
and a startup integrity check.

## Concurrent duplicates

An in-process duplicate waits on the original dispatch instead of immediately
marking it uncertain or invoking the adapter again. The wait registration is
created while the ledger reservation is still locked, closing the
reserve-before-register race.

If the original future is cancelled, panics, or cannot persist its result, a
guard releases all waiters. The next duplicate sees the durable nonterminal
record and returns `ReconciliationRequired`; it never retries the source
mutation blindly.

## Restart behavior

A completed result survives connector restart and is returned byte-for-byte
without another adapter call. A `Created`, `Persisted`, `Dispatched`,
`SourceAcknowledged`, or unknown-outcome record without a terminal result is
converted to a persisted `ReconciliationRequired` result on the next retry.
Terminal states without a serialized result, mismatched state/result pairs,
and malformed serialized results are treated as ledger corruption.

## Routing

Every session mutation now carries an explicit `runtime_id` in addition to its
source session or approval identifier. This avoids cross-adapter guessing and
session-ID collision. Implemented commands route as follows:

- start session, send input, steer, interrupt, and approve: selected adapter;
- probe host: local success;
- change assignment and rotate credential: fail closed until vault transactions
  and managed profile isolation are connected.

Adapter transport-loss and unknown-outcome errors become
`ReconciliationRequired`. Known validation, unsupported-operation, and source
rejection errors become terminal failures.

## Boundary still pending

No unauthenticated network listener is connected to this router. The next
transport slice must authenticate the paired device, decrypt and validate the
envelope, enforce recipient/boot/sequence rules, pass the header idempotency key
and command to the router, then encrypt the returned `CommandResult`.
