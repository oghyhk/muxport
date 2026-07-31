# ADR-0006: Agent Adapter Trait Model and Built-in vs Plugin Architecture

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Muxport unifies OpenCode and Codex agent runtimes behind a normalized interface.

## Decision

1. **`AgentAdapter` Rust Trait:**
   - Unified interface in `crates/adapter-api`:
     - `probe() -> CapabilitySet`
     - `discover_projects() -> Vec<Project>`
     - `list_sessions() / read_session()`
     - `subscribe_events() -> Stream<AgentEvent>`
     - `start_session() / send_input() / steer() / interrupt()`
     - `respond_approval(approval_id, decision)`
     - `validate_credential() / activate_credential()`
2. **Built-in Signed Adapters:**
   - OpenCode adapter (`crates/adapter-opencode`): HTTP/OpenAPI + SSE event stream against OpenCode REST API.
   - Codex adapter (`crates/adapter-codex`): `codex app-server` over stdio JSON-RPC.
3. **Future Plugin Model:**
   - Plugins run out-of-process via IPC/gRPC. Plugins never have direct filesystem access to the credential vault.

## Consequences

- MVP adapters are compiled into connector for maximum performance and security.
- Clear contract for adding future AI coding agents.
