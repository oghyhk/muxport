# Runbook: Database Corruption & Reconstructible Recovery

## Invariant Rule
NEVER delete or repair the only copy of a damaged SQLite database file (`metadata.db`, `events.db`, `audit.db`). Always preserve raw files and WAL/SHM components first.

## Procedure
1. Stop host connector daemon:
   `systemctl stop muxport-connector`
2. Create an exact diagnostic backup copy of the data directory:
   `cp -r ~/.muxport/state ~/.muxport/state-corrupt-backup-$(date +%Y%m%d%H%M%S)`
3. Run SQLite integrity check on copies:
   `sqlite3 ~/.muxport/state-corrupt-backup-*/events.db "PRAGMA integrity_check;"`
4. If `events.db` is irrecoverable, `events.db` is explicitly RECONSTRUCTIBLE:
   - Remove corrupted `events.db`.
   - Restart connector in `recovering` state.
   - Connector probes OpenCode and Codex source runtimes to fetch authoritative snapshots and rebuild the journal baseline sequence.
5. If `metadata.db` or `vault.sealed` is corrupt:
   - Restore verified snapshot archive using `muxport-connector restore --archive <PATH> --isolated-drill`.
