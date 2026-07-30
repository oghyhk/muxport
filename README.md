# Muxport

Muxport is an open-source, mobile-first control plane for operating OpenCode
and Codex runtimes on user-controlled desktops and servers.

> **Project status:** early security-focused scaffold. The protocol, Rust
> workspace, and Flutter shell compile and have baseline tests. An opt-in
> authenticated direct command endpoint and Flutter wire client exist, but
> their app integration, end-to-end pairing UI, durable credential rotation,
> and real-time Codex and OpenCode synchronization are not complete. Do not
> expose this build publicly or use it to manage production credentials.

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

The connector's command-only direct listener is disabled by default. For local
development it can be enabled on loopback:

```sh
MUXPORT_DIRECT_BIND=127.0.0.1:45821 cargo run -p connector
```

Non-loopback binding additionally requires
`MUXPORT_ALLOW_REMOTE_DIRECT=1`. Use only on an access-controlled private
network; the Flutter wire client is not launched by the app yet, and
snapshot/event sync is not connected.

## Security

Never report a security issue containing live credentials in a public issue.
Until a private disclosure channel is published, redact secrets and provide
only the minimum reproduction details to a maintainer.

## License

MIT. See [LICENSE](LICENSE).
