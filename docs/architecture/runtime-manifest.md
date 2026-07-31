# Managed runtime manifest

The connector can supervise several isolated OpenCode and Codex instances in
one daemon. Set `MUXPORT_RUNTIME_MANIFEST` to the absolute path of a version 1
JSON manifest:

```json
{
  "version": 1,
  "profiles_root": "/home/alice/.local/share/muxport/profiles",
  "runtimes": [
    {
      "runtime_id": "opencode-work",
      "agent_type": "opencode",
      "profile_id": "work",
      "display_name": "Work OpenCode",
      "executable": "/home/alice/.local/bin/opencode",
      "project_directory": "/home/alice/code/work",
      "port": 4096
    },
    {
      "runtime_id": "codex-personal",
      "agent_type": "codex",
      "profile_id": "personal",
      "display_name": "Personal Codex",
      "executable": "/home/alice/.local/bin/codex",
      "project_directory": "/home/alice/code/personal"
    }
  ]
}
```

Every runtime and profile ID must contain only ASCII letters, digits, `-`, or
`_` and may be at most 128 bytes. Runtime IDs must be unique. A profile may be
assigned to only one runtime of its agent type, and every managed OpenCode
runtime needs a unique nonzero loopback port. Codex entries must omit `port`.
The manifest supports at most 64 runtimes.

The manifest path, `profiles_root`, executables, and project directories must
be absolute. The manifest must be a regular file, not a symbolic link, and is
limited to 1 MiB. Unknown JSON fields fail closed. This deliberately rejects
fields such as `password`, `api_key`, or provider-specific tokens.

## Credential boundary

The manifest is topology, not a credential store:

- OpenCode credentials remain in each connector-owned isolated profile and
  are written only by OpenCode's supported authentication flow.
- The connector generates a different in-memory OpenCode server password for
  every managed process on every daemon start. It is never persisted in the
  manifest.
- Codex credentials remain owned by Codex inside the selected isolated
  `CODEX_HOME`; Muxport uses supported `account/*` App Server methods.
- Provider credentials must never be placed in the manifest or process logs.

Enroll each profile locally before starting the manifest-backed daemon. The
existing `opencode-profile-auth` and `codex-profile-login` commands accept one
profile at a time; point `MUXPORT_PROFILES_DIR` at the same `profiles_root`
used by the manifest.

## Startup, synchronization, and restart behavior

The whole manifest is parsed and collision-checked before any managed process
starts. Every configured executable and project path is also checked before
the first child starts. If a later managed runtime still cannot start,
already-started OpenCode children are stopped before connector startup fails.

Preflight the exact file without starting a runtime:

```sh
cargo run -p connector --bin muxport-connector -- \
  runtime-manifest-validate /absolute/path/to/runtimes.json
```

The command prints only the manifest version and runtime count; it does not
print paths, account metadata, or credential material.

Each entry gets an independent adapter, monitor, authoritative snapshot, event
subscription, health loop, and persistent runtime ID. Mobile projections
therefore receive the actual projects, sessions, approvals, account metadata,
and runtime health for every configured instance through the normal snapshot
and journal protocol.

When a managed OpenCode child exits, its monitor restarts the same executable,
project, profile roots, loopback port, and in-memory password, then takes a new
authoritative snapshot. Five process failures in ten minutes latch the runtime
as degraded until the connector is explicitly restarted. Managed Codex App Server
processes use the adapter supervisor and reopen the same isolated profile after
failure. Connector restart reloads the same stable runtime IDs and profile
roots, while the event journal repairs mobile cursors from a snapshot plus
contiguous replay. On Unix, both `SIGINT` and the `SIGTERM` used by service
managers trigger graceful adapter shutdown and a final snapshot. Managed child
handles are also kill-on-drop, so an error during later connector startup
cannot leave an orphan OpenCode server behind.

If the connector host is offline, mobile keeps its last redacted projection
and marks synchronization stale; it does not invent runtime state or execute
queued mutations against an unknown process. Normal synchronization resumes
after the connector and runtime become reachable.

## Compatibility

If `MUXPORT_RUNTIME_MANIFEST` is unset, the connector retains the legacy
single-OpenCode plus single-Codex environment-variable configuration. When the
manifest variable is set, the manifest is authoritative for runtime topology.
