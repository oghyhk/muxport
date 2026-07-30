# Muxport Agent Instructions

- Read `SPEC.md` and `PLAN.md` before changing architecture or product scope.
- Build the mobile client with Flutter and security-sensitive host components with Rust unless an accepted architecture decision says otherwise.
- Use only supported OpenCode and Codex interfaces; never edit their credential stores, databases, WAL/SHM files, or session state directly.
- Never commit credentials, tokens, private keys, live databases, unredacted logs, or real user data. Use obvious test placeholders.
- Preserve state before recovery work. Diagnose copies, verify backups, and report unknown outcomes honestly.
- Changes to credentials, rotation, synchronization, approvals, or restart recovery require automated failure-path tests and updated documentation.
- Keep the relay unable to decrypt application traffic and keep plugins isolated from the credential vault.
- Preserve unrelated work, keep commits scoped, and commit and push every meaningful completed change.
