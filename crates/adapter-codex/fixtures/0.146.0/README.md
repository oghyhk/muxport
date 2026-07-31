# Codex App Server 0.146.0 account contract

These JSON Schema files were generated from the official `@openai/codex@0.146.0`
package:

```sh
codex app-server generate-json-schema --out <temporary-directory>
```

Only the stable account methods and notifications implemented by Muxport are
retained. Regenerate them from the exact supported Codex release when bumping
the fixture version, review the diff, run the adapter contract tests, and run
the ignored live compatibility fixture. Do not hand-edit generated JSON.
