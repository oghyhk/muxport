# Muxport

Muxport is an open-source, mobile-first control plane for operating OpenCode
and Codex runtimes on user-controlled desktops and servers.

> **Project status:** early security-focused scaffold. The protocol, Rust
> workspace, and Flutter shell compile and have baseline tests, but remote
> control, end-to-end pairing, durable credential rotation, and real-time Codex
> and OpenCode synchronization are not complete. Do not deploy this build as a
> public relay or use it to manage production credentials.

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

## Security

Never report a security issue containing live credentials in a public issue.
Until a private disclosure channel is published, redact secrets and provide
only the minimum reproduction details to a maintainer.

## License

MIT. See [LICENSE](LICENSE).
