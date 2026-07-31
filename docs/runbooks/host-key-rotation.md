# Runbook: Host Identity Key Rotation

## When to use this

Use this only when the connector's long-term host identity may be compromised
or must be replaced. It invalidates every paired phone and every outstanding
pairing offer. It does **not** rotate, delete, export, or otherwise open any
provider credential in the Muxport vault.

## Preconditions

1. Stop the connector's supervisor. The maintenance command refuses to run
   while it holds the normal connector instance lock.
2. Preserve the current SQLite/WAL evidence. Do not delete, move, checkpoint,
   or edit the live database files.
3. From the same OS user and with the same `MUXPORT_*_DB` paths used by the
   connector, create a new absolute-path backup and verify it:

   ```sh
   muxport-connector state-backup /absolute/path/muxport-recovery-YYYYMMDD
   muxport-connector state-backup-verify /absolute/path/muxport-recovery-YYYYMMDD
   ```

   The recovery backup must contain `pairing.db`; the rotation command checks
   this rather than accepting a partial or unverified backup.

## Rotate

Enter the host ID twice as a deliberate confirmation:

```sh
muxport-connector pairing-rotate-host-key HOST_ID \
  /absolute/path/muxport-recovery-YYYYMMDD \
  --confirm-host-id HOST_ID
```

The connector first writes a replacement identity and a temporary recovery
record to the OS credential store, then records a pending transition in the
pairing database. Only after both are durable does it promote the new identity
and atomically clear the signed device registry and pairing offers. If the host
restarts during the narrow post-promotion window, connector startup completes
that recorded transition; it never accepts a replacement key just because the
previous one is absent.

## Verify and recover service

1. Start the connector supervisor.
2. Confirm the connector can start with the new protected identity.
3. Pair each trusted phone again using a fresh, one-time offer and verify its
   SAS. Previously paired phones must be rejected before a secure session is
   established.
4. Confirm managed OpenCode/Codex runtimes are healthy. Their provider
   credentials and isolated runtime-state directories should be unchanged.

If the command reports a protected-store or database error, keep the verified
backup and existing files intact. Do not attempt to edit the identity pin or
signed registry by hand; collect the error and use the preserved recovery
evidence for investigation.
