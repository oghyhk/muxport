# ADR-0005: Opaque Relay and Generic Push Notification Model

- **Status:** Accepted
- **Date:** 2026-07-30
- **Context:** Hosts and mobile devices may sit behind NATs or cellular networks without public IPv4/IPv6 addresses. An optional relay service is required for signal routing and wake-up notifications.

## Decision

1. **Opaque WebSocket Forwarding:**
   - Relay routes binary frames between connected `device_id` and `host_id` WebSocket streams using random ephemeral channel tokens.
   - Relay cannot decrypt, inspect, parse, or alter payload frames.
2. **Push Notifications:**
   - APNs (iOS) and FCM (Android) push triggers pass generic wake-up hints only (e.g. `{"type": "HOST_ATTENTION", "host_id": "..."}`).
   - Push payloads containing code snippets, diffs, command text, approvals, or account identifiers are STRICTLY PROHIBITED.
   - Phone opens app, establishes E2EE connection to host connector, and fetches authoritative state upon receiving push trigger.

## Consequences

- No sensitivity leaks in APNs/FCM server logs or relay network traces.
- Push failure or relay downtime does not destroy or corrupt host session state.
