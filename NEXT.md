You are the primary implementation agent for Muxport. Your concrete objective is to implement, test, secure, document, and verify the complete Muxport system described by `SPEC.md` and every applicable checklist item in `PLAN.md`.



\*\*DO NOT STOP\*\* until you finish every assigned item and complete all required validation, unless a genuine blocker requires user input or an external-state change. Do not treat a prototype, partial vertical slice, passing unit tests, or exhausted context window as completion. Continue across turns and preserve durable progress through scoped commits and pushes.



\## Repository and durable state



\- Repository: `C:\\Users\\oghyh\\Coding\\Muxport`

\- Remote: `https://github.com/oghyhk/muxport.git`

\- Current branch: `main`

\- Upstream: `origin/main`

\- Current commit: `b1061e3 docs: add initial Muxport specification and plan`

\- Working tree at handoff: clean

\- Public repository: yes

\- Existing files:

&#x20; - `AGENTS.md`

&#x20; - `SPEC.md`

&#x20; - `PLAN.md`

\- Existing implementation: none

\- Existing tests: none

\- Completed work: product specification, detailed engineering plan, short repository instructions, repository creation, initial commit, and push.

\- In-progress work: none.

\- Uncommitted or untracked files at handoff: none.

\- Verification completed so far: documentation presence and Git state only. No product code or engineering checklist item has been implemented or tested.



\## Mandatory instructions before acting



1\. Read `C:\\Users\\oghyh\\Coding\\Muxport\\AGENTS.md`, `SPEC.md`, and `PLAN.md` completely.

2\. Because this work involves OpenCode configuration, authentication, state, credentials, providers, plugins, and infrastructure, \*\*MUST\*\* read `C:\\Users\\oghyh\\Coding\\Incidents\\LESSONS.md` completely before any edit, implementation command, or delegation.

3\. Read any other applicable parent or nested `AGENTS.md` files if present.

4\. Follow the repository rule requiring meaningful changes to be committed and pushed.

5\. Codebase-wide or workspace-wide search is prohibited. Target specific files and directories. Ask the user before any genuinely necessary broad search.

6\. Use supported OpenCode and Codex interfaces only. \*\*NEVER\*\* edit their token stores, databases, WAL/SHM files, or session state directly.

7\. \*\*NEVER\*\* expose or commit credentials, tokens, private keys, real account data, live databases, unredacted logs, production data, or plaintext backup material.



\## Recovery-first start procedure



Before editing:



1\. Confirm the repository path and read the mandatory files.

2\. Run targeted Git checks:

&#x20;  - `git status --short --branch`

&#x20;  - `git log -5 --oneline --decorate`

&#x20;  - `git remote -v`

&#x20;  - focused `git diff` and untracked-file inspection

3\. Verify the clean state and commit `b1061e3`; treat this prompt as a lead and durable Git/filesystem state as authoritative.

4\. Inspect whether any Muxport-specific process, test, or development server is already running before launching duplicates.

5\. Create a scoped development branch using the required `codex/` prefix, such as `codex/implement-muxport`, unless an existing continuation branch is already present.

6\. Preserve all user and other-agent work. \*\*DO NOT\*\* reset, clean, stash, overwrite, or revert unrelated changes.

7\. Do not redo the completed specification, planning, repository initialization, or initial commit. Update the documents only when implementation evidence or accepted architectural decisions require it.



\## Architecture that must be preserved unless an ADR justifies a change



\- Flutter mobile client for iOS and Android.

\- Rust connector, relay, protocol core, runtime supervision, event journal, credential vault, adapters, and security-sensitive components.

\- OpenCode integration through supported HTTP/OpenAPI and SSE interfaces.

\- Codex integration through locally supervised `codex app-server` over its supported stdio JSON-RPC transport.

\- Do not depend on Codex’s experimental WebSocket listener for production.

\- Optional opaque relay with end-to-end encrypted application traffic.

\- Direct LAN/private-network path.

\- Host runtime remains the source of truth.

\- Mobile cache and connector event journal are reconstructible.

\- Built-in signed OpenCode and Codex adapters for MVP; future third-party plugins must be out of process and isolated from the vault.

\- Credentials remain on explicitly selected hosts by default.

\- Active turns never change credential identity silently.

\- Unknown operation outcomes remain explicitly unknown until reconciled.

\- Bulk account switching is a tracked set of per-host idempotent operations, not a fictitious distributed atomic transaction.



\## Assigned work



\*\*DO NOT STOP\*\* until every item below is implemented and the required validation is complete, unless a genuine blocker requires user input.



Work through `PLAN.md` systematically. Treat its checkboxes, exit conditions, release gates, and definition of done as the authoritative implementation checklist. Update a checkbox only after implementation and verification evidence exists.



\### Priority 1 — Phase 0 decisions and feasibility



\- Produce ADRs for Flutter/Rust boundaries, protocol encoding, E2EE pairing, key storage, relay behavior, adapter model, and profile isolation.

\- Complete the validation spikes in `SPEC.md` and `PLAN.md`.

\- Prove OpenCode snapshots, SSE streaming, reconnect, and reconciliation.

\- Prove supported OpenCode Go credential validation/activation and determine restart requirements.

\- Determine a supported isolation method for concurrent OpenCode profiles.

\- Prove Codex App Server initialization, threads, turns, deltas, diffs, approvals, interruption, login, rate limits, and recovery.

\- Determine a supported isolation method for concurrent Codex account profiles.

\- Test whether externally started OpenCode/Codex sessions can be adopted safely.

\- Verify current provider behavior and terms before enabling quota-triggered automatic rotation.

\- Record unknowns honestly; do not invent capabilities.



\### Priority 2 — Repository and protocol foundation



\- Scaffold the monorepo structure in `PLAN.md`.

\- Add pinned Rust and Flutter toolchains, lockfiles, formatting, linting, CI, schema generation, and build scripts.

\- Implement canonical versioned wire schemas with Rust and Dart generation.

\- Add golden protocol fixtures and compatibility tests.

\- Implement fake connector and deterministic fake OpenCode/Codex adapters.

\- Implement explicit connector, runtime, and operation state machines.

\- Implement idempotent command handling, deadlines, boot epochs, event cursors, snapshot boundaries, and gap recovery.



\### Priority 3 — Secure connector and storage



\- Implement the Rust connector and CLI.

\- Implement desired-versus-observed reconciliation.

\- Separate metadata, event journal, audit history, vault, vendor profiles, and diagnostics.

\- Implement safe SQLite migrations, integrity checks, consistent backups, and evidence-preserving recovery.

\- Implement platform vault integrations and a reviewed headless-server fallback.

\- Ensure plaintext credentials never enter logs, analytics, crash reports, event journals, push payloads, ordinary mobile caches, or repository history.

\- Implement QR pairing, device identity, revocation, replay protection, and E2EE using established reviewed libraries.

\- Do not invent custom cryptography.



\### Priority 4 — OpenCode and Codex adapters



\- Build the OpenCode adapter against documented APIs and runtime-discovered capabilities.

\- Build the Codex App Server adapter over stdio JSON-RPC.

\- Normalize shared concepts without discarding source-specific safety semantics.

\- Implement snapshots, streams, prompts, approvals, diffs, interruption, account state, usage, and source-version compatibility.

\- Implement supervised runtime lifecycle, graceful shutdown, crash-loop detection, readback, and reconciliation.

\- Never automatically resubmit an unknown prompt after a crash.



\### Priority 5 — Flutter application



\- Implement iOS and Android targets.

\- Implement pairing, host fleet, runtime/project details, unified session timeline, approval inbox, account profiles, assignment matrix, rotation pools, bulk-switch preview, operation progress, recovery states, revocation, and diagnostics.

\- Implement mobile process-death, background, suspension, network-change, and push-reconnect behavior.

\- Use push only as a generic wake-up hint.

\- Add biometric step-up for credentials, rotation, bulk operations, export, and revocation.

\- Add accessibility, screen-reader, text scaling, safe diff/command cards, app-switcher privacy, and duplicate-tap protection.



\### Priority 6 — Credential rotation



\- Implement staged credential validation, activation, readback, commit, and rollback.

\- Implement manual, bulk, scheduled, round-robin, and guarded confirmed-failure rotation for authorized OpenCode Go profiles.

\- Implement cooldowns, switch-rate limits, exclusions, compatibility checks, and complete audit history.

\- Never rotate merely because of a connector restart, runtime crash, network failure, malformed response, or guessed error string.

\- Default changes to new sessions.

\- Require explicit confirmation before draining or restarting active runtimes.

\- Model bulk operations with per-host results and honest partial failure.



\### Priority 7 — Relay, recovery, and operations



\- Implement the opaque relay and direct/private-network mode.

\- Implement generic APNs/FCM notification flow without sensitive payloads.

\- Implement restart recovery for the phone, relay, connector, host, OpenCode, Codex, vault, databases, and interrupted updates.

\- Implement encrypted backup/restore with verified manifests and isolated restore drills.

\- Add lost-phone, host replacement, compromised credential, failed rotation, crash-loop, vendor incompatibility, database corruption, vault recovery, relay outage, rollback, and incident runbooks.

\- Implement signed atomic connector updates with rollback.

\- Keep an independent recovery path available during connector/relay maintenance.



\### Priority 8 — Verification and release hardening



\- Add unit, contract, integration, E2E, recovery, chaos, fuzz, compatibility, and security tests described by `PLAN.md`.

\- Test actual phone process death and OS background eviction, not only development hot restart.

\- Kill each layer at every important mutation boundary.

\- Verify no duplicated prompt, false approval, silent identity change, lost assignment, or unreported unknown outcome.

\- Add dependency auditing, secret scanning, SBOMs, provenance, signed artifacts, checksums, and staged release channels.

\- Complete physical-device testing for supported iOS and Android versions.

\- Complete Windows, macOS, Linux, headless Linux, OpenCode, and Codex compatibility testing.

\- Resolve all critical/high security findings.

\- Do not claim an external penetration test, cryptographic review, legal review, App Store publication, or production deployment occurred unless it actually did. If one requires external authority, credentials, payment, or human review, complete all preparatory work and report the exact blocker.



\## Security and data constraints



\- The public repository must remain safe to clone.

\- Use synthetic accounts, fake keys, disposable directories, and isolated test profiles.

\- Preserve source runtime state before recovery experiments.

\- Diagnose corrupted databases only on copies.

\- Treat SQLite WAL files as data.

\- Keep the credential vault separate from reconstructible event state.

\- Never put secrets in environment dumps, subprocess diagnostics, fixtures, screenshots, support bundles, CI logs, GitHub Actions output, or test snapshots.

\- The relay must be unable to decrypt prompts, source code, diffs, commands, approvals, account labels, or credentials.

\- A compromised host is an explicit trust-boundary limitation; document it rather than claiming impossible protection.

\- Account rotation must support authorized identity separation and must not be marketed or implemented as provider-restriction evasion.



\## Compatibility and change discipline



\- Preserve backward compatibility across the documented mobile/connector protocol support window.

\- Fail closed for unsupported mutating operations.

\- Keep source-version capability matrices.

\- Update `SPEC.md`, `PLAN.md`, ADRs, schemas, runbooks, and tests when behavior changes.

\- Do not collapse source-specific behavior into a misleading lowest-common-denominator model.

\- Do not add arbitrary in-process plugins with vault access.

\- Do not deploy or publish to production, App Store, or Play Store without explicit user authorization and required credentials.

\- Use current official primary documentation for OpenCode, Codex, Flutter, platform security, APNs, and FCM when implementation depends on unstable details.



\## Testing and completion evidence



Run proportionate checks after each scoped change and comprehensive checks before declaring a phase complete. At minimum, establish and maintain:



\- Rust format, lint, unit, integration, contract, fuzz, and platform builds.

\- Flutter format, analyze, unit, widget, integration, accessibility, and physical-device tests.

\- Rust/Dart protocol golden tests.

\- OpenCode/Codex compatibility fixtures and live disposable-environment tests.

\- Recovery/chaos tests at command and persistence boundaries.

\- Secret scanning and artifact inspection.

\- Backup/restore drills using verified isolated copies.

\- Signed update rollback tests.

\- End-to-end multi-phone, multi-host, multi-runtime, multi-credential tests.



Record exact commands, versions, results, skipped checks, and reasons. \*\*NEVER\*\* mark a `PLAN.md` checkbox complete without evidence.



\## Git, branch, and push rules



\- Work on a `codex/` branch, not directly on `main`.

\- Keep commits small, coherent, and reviewable.

\- Commit and push every meaningful completed change.

\- Do not force-push, rewrite public history, reset destructively, or merge to `main` without explicit authorization.

\- Preserve unrelated user and other-agent changes.

\- Before each push, inspect the staged diff and run relevant checks.

\- Do not commit generated secrets, live state, build signing material, or private test artifacts.

\- If a release/deployment needs new authority, stop only at that genuine boundary and request the exact missing input.



\## Genuine blockers



A task is not blocked merely because it is difficult, large, slow, or requires more iterations. Continue implementing everything that is safely possible. A genuine blocker includes missing provider accounts needed for a live multi-account test, unavailable signing identities, required legal decisions, unavailable external security reviewers, or explicit production-deployment authorization.



When blocked:



1\. Preserve durable state.

2\. Commit and push all verified in-scope progress.

3\. Document the exact checklist items affected.

4\. Explain what was attempted and what evidence exists.

5\. Ask for only the specific missing input.

6\. Continue all independent work instead of stopping the entire project.



\## Final completion and report



\*\*DO NOT STOP\*\* until all assigned implementation items and required validation are finished, unless a genuine blocker requires user input. Context limits, intermediate milestones, successful prototypes, and partially passing test suites are not completion; persist progress and continue.



The final report must include:



\- Architecture and major implementation decisions.

\- Files/modules added or changed.

\- `PLAN.md` checklist completion status with evidence.

\- Commits and pushed branches.

\- Exact test, build, security, compatibility, recovery, and restore results.

\- Supported OS/OpenCode/Codex/mobile versions.

\- Credential-safety and restart-recovery verification.

\- Known limitations and residual risks.

\- Any genuinely blocked items, the precise external dependency, and the smallest required user action.

\- Confirmation that no secrets or production data were exposed.

\- Confirmation that all claims were checked against durable repository and test evidence.



Do not claim Muxport is finished while any assigned checklist item remains unimplemented, unverified, falsely checked off, or silently skipped.

