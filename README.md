# Muxport

Muxport is an open-source, mobile-first control plane for operating OpenCode
and Codex runtimes on user-controlled desktops and servers.

> **Project status:** security-focused development build. Authenticated direct
> commands, signed/SAS-confirmed pairing, restart-safe mobile enrollment, and
> encrypted snapshot/event replay are integrated and tested. Connector-managed
> OpenCode profiles now isolate vendor state and provide staged activation,
> readback, and rollback primitives. Mobile credential provisioning, full
> session/approval UI wiring, relay deployment, and release hardening remain
> incomplete. Do not expose this build publicly or use it to manage production
> credentials.

Muxport is an independent project. It is not built, sponsored, or endorsed by
the OpenCode team or OpenAI.

## Intended architecture

- A Flutter client provides one mobile view of hosts, sessions, approvals, and
  credential assignments.
- A Rust connector runs on each managed host and talks to supported OpenCode
  HTTP/SSE and Codex App Server interfaces.
- An optional relay routes opaque encrypted frames. Provider credentials remain
  on explicitly selected user-controlled hosts.
- Host journals and authoritative runtime snapshots allow clients to recover
  after network loss or process restarts.

The product contract is in [SPEC.md](SPEC.md). The complete implementation
checklist and acceptance gates are in [PLAN.md](PLAN.md).

## Repository layout

| Path | Purpose |
| --- | --- |
| `apps/mobile` | Flutter iOS and Android client |
| `crates/connector` | Host connector process |
| `crates/adapter-opencode` | OpenCode adapter |
| `crates/adapter-codex` | Codex App Server adapter |
| `crates/credential-vault` | Host credential-encryption primitives |
| `crates/event-journal` | Durable event and snapshot journal |
| `crates/muxport-crypto` | Pairing and frame-encryption primitives |
| `crates/relay` | Optional opaque WebSocket relay |
| `protocol/schema` | Versioned protobuf schema |

## Verify the current scaffold

Rust requires toolchain 1.85 or newer:

```sh
cargo test --locked --workspace --all-targets
```

Flutter requires Flutter 3.44 or a compatible stable release:

```sh
cd apps/mobile
flutter pub get
flutter analyze
flutter test
```

See [the worker audit](docs/reviews/worker-audit-2026-07-30.md) for the exact
verified state and known gaps.

The connector's authenticated direct listener is disabled by default. For
local development it can be enabled on loopback:

```sh
MUXPORT_DIRECT_BIND=127.0.0.1:45821 cargo run -p connector
```

Non-loopback binding additionally requires
`MUXPORT_ALLOW_REMOTE_DIRECT=1`. Use only on an access-controlled private
network.

### Development pairing flow

Pairing offers are opt-in and expire after 30–600 seconds. Set the advertised
endpoint to an address the phone can reach:

```sh
MUXPORT_DIRECT_BIND=0.0.0.0:45821 \
MUXPORT_ALLOW_REMOTE_DIRECT=1 \
MUXPORT_PAIRING_ENDPOINT=192.0.2.10:45821 \
MUXPORT_PAIRING_OFFER_TTL_SECONDS=300 \
cargo run -p connector --bin muxport-connector
```

Copy the logged `pairing_offer_json` into the mobile pairing dialog. Compare
the six-digit SAS on both trusted displays. Complete the host half from the
same OS user account and working directory as the daemon:

```sh
cargo run -p connector --bin muxport-connector -- \
  pairing-confirm HOST_ID PAIRING_ID SIX_DIGIT_SAS
```

If `MUXPORT_PAIRING_DB` is customized for the daemon, pass the same environment
value to the confirmation command. The command loads only the existing
OS-protected host identity and fails closed for a wrong host ID. Once both
sides confirm, the mobile app periodically authenticates, restores a snapshot,
replays contiguous journal events, persists its cursor, and acknowledges it.

For a lost phone, list and revoke the affected paired device from the same OS
user account, then restart the connector so its listener reloads the signed
registry:

```sh
cargo run -p connector --bin muxport-connector -- pairing-list HOST_ID
cargo run -p connector --bin muxport-connector -- pairing-revoke HOST_ID DEVICE_ID
```

See the [lost-phone revocation runbook](docs/runbooks/lost-phone-revocation.md)
before performing this recovery action.

For a suspected host-identity compromise, stop the connector, create and
verify a recovery backup, then use `pairing-rotate-host-key`. This deliberately
invalidates every paired phone while preserving provider credentials; follow
the [host-key rotation runbook](docs/runbooks/host-key-rotation.md) exactly.

### Development managed OpenCode profile

Managed profiles keep OpenCode home, data, configuration, cache, and state in
separate directories. The connector never copies or edits OpenCode's
`auth.json`; local enrollment delegates to OpenCode's supported interactive
authentication command:

```sh
export MUXPORT_PROFILES_DIR="$PWD/muxport-profiles"
export MUXPORT_OPENCODE_PATH="/absolute/path/to/opencode"
export MUXPORT_OPENCODE_PROJECT="/absolute/path/to/project"

cargo run -p connector --bin muxport-connector -- \
  opencode-profile-auth go-account-a opencode
```

Start the connector with the same profile settings:

```sh
export MUXPORT_OPENCODE_PROFILE_ID="go-account-a"
export MUXPORT_OPENCODE_PORT="4096"
cargo run -p connector --bin muxport-connector
```

The managed server binds only to `127.0.0.1` and uses a fresh in-memory server
password unless `MUXPORT_OPENCODE_PASSWORD` is explicitly supplied. Its child
environment is cleared before an OS bootstrap allowlist and the isolated
profile paths are added, preventing unrelated provider variables from leaking
into the managed runtime.

### Development managed Codex profile

The connector can own a Codex account profile with separate configuration,
authentication, session, log, skill-metadata, and SQLite roots:

```sh
export MUXPORT_PROFILES_DIR="$PWD/muxport-profiles"
export MUXPORT_CODEX_PROFILE_ID="personal"
export MUXPORT_CODEX_PATH="/absolute/path/to/codex"
export MUXPORT_CODEX_PROJECT="/absolute/path/to/project"
cargo run -p connector --bin muxport-connector -- \
  codex-profile-login personal
cargo run -p connector --bin muxport-connector
```

This launches App Server over stdio with isolated `CODEX_HOME` and
`CODEX_SQLITE_HOME` directories. Codex owns login persistence and token
refresh. Muxport uses supported `account/*` methods and never reads, edits, or
copies Codex credential/database files. The local enrollment command uses
Codex's device-code flow, displays only the verification URL/code, waits for
the supported completion notification, and confirms `account/read`. Mobile
account enrollment and production installers remain incomplete.

### Multiple managed OpenCode and Codex instances

Set `MUXPORT_RUNTIME_MANIFEST` to an absolute versioned JSON manifest to
supervise up to 64 isolated runtimes in one connector:

```json
{
  "version": 1,
  "profiles_root": "/absolute/path/to/muxport-profiles",
  "runtimes": [
    {
      "runtime_id": "opencode-work",
      "agent_type": "opencode",
      "profile_id": "work",
      "executable": "/absolute/path/to/opencode",
      "project_directory": "/absolute/path/to/work-project",
      "port": 4096
    },
    {
      "runtime_id": "codex-personal",
      "agent_type": "codex",
      "profile_id": "personal",
      "executable": "/absolute/path/to/codex",
      "project_directory": "/absolute/path/to/personal-project"
    }
  ]
}
```

Do not put credentials in this file. Unknown fields are rejected, OpenCode
ports must be unique, and duplicate runtime/profile assignments fail before
startup. See
[the managed runtime manifest design](docs/architecture/runtime-manifest.md)
for enrollment, validation, synchronization, and restart behavior. The legacy
single-runtime environment variables remain available when the manifest
variable is unset.

For unattended VPS operation and the rootless container boundary, see the
[Linux server deployment guide](docs/deployment/linux-server.md).
For WAL-safe online backups and evidence-preserving restore steps, see the
[state backup and restore runbook](docs/runbooks/state-backup-and-restore.md).

## Security

Never report a security issue containing live credentials in a public issue.
Until a private disclosure channel is published, redact secrets and provide
only the minimum reproduction details to a maintainer.

## License

MIT. See [LICENSE](LICENSE).
