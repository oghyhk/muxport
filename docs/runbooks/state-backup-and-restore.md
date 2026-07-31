# State backup and restore

Muxport SQLite write-ahead logs are live state. Never delete a `-wal` or
`-shm` file, and never treat a copy of only the main `.db` file as a valid
backup.

## Create a backup

Choose a new absolute directory that does not exist:

```text
muxport-connector state-backup /absolute/path/to/muxport-backup
```

The command uses SQLite's online backup API for the event journal, command
ledger, and pairing registry. This includes committed transactions still
resident in WAL and runs `PRAGMA quick_check` against both source and backup.
Provider secrets are excluded by default.

To explicitly enable encrypted credential recovery, include the already sealed
vault generation:

```text
muxport-connector state-backup /absolute/path/to/muxport-backup --include-vault
```

This copies one complete atomic generation of `vault.sealed` separately. The
vault remains encrypted by its host-specific wrapping policy; the relay never
receives a recovery key.

The output directory is owner-private. `manifest.json` records a SHA-256
digest and size for every output file without recording source paths or
secrets. A missing source component is omitted rather than fabricated.

The host identity key remains in the configured OS credential store and is
not exported. Restoring a pairing registry without its original host identity
will correctly require phones to pair again.

The Muxport event journal (`state.db` by default) is reconstructible from
authoritative agent snapshots, but it may be rebuilt only after the original
database, WAL/SHM companions, logs, hashes, and versions have been preserved
as incident evidence.

## Restore

1. Stop the connector and confirm no Muxport process owns the state files.
2. Preserve the damaged/original files, their WAL/SHM companions, hashes,
   timestamps, permissions, logs, and versions before changing anything.
3. Verify every file against `manifest.json`.
4. Run `PRAGMA quick_check` on each backed-up SQLite database.
5. Restore the databases and sealed vault as separate files with
   owner-private permissions. Do not restore backup-side WAL/SHM files; the
   online backup outputs are checkpointed standalone databases.
6. Start the connector in recovery mode. Do not accept mutations until source
   probes, authoritative session snapshots, event subscriptions, and account
   readback have reconciled.
7. If the protected host identity is missing or differs from the pairing
   registry pin, re-pair devices. Never replace the stored pin silently.

Do not claim recovery from a successful process start alone. Verify connector
state, runtime state, active credential fingerprints, session IDs, pending
approvals, and unknown operations from their authoritative sources.
