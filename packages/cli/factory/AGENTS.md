# TOHSENO shot instructions

This is an independent iOS **shot**: its source, Git history, local runtime,
tests, manifest tools, and operational playbooks stay usable without the global
TOHSENO CLI or factory cache.

First ask the owner: “What do you want to make?” One sentence is enough. If
they already arrived with a specific change, use it instead of making them
repeat themselves. The launch prompt is intentionally constant; never put the
private product idea in commands, metadata, logs, or committed files.

Then read these local sources completely:

1. `skills/continuity-app/SKILL.md` — product/build protocol.
2. `.tohseno/OPERATIONS.md` — deterministic development and production rails.

The layout is deliberately plain:

- `App/`, `Tests/`, `project.yml`, `Writing.xcodeproj/`: SwiftUI app and tests.
- `Backend/`: Bun API and SQLite migration foundation; it receives no writing content.
- `.tohseno/data/`: persistent development database (gitignored).
- `.tohseno/run/`: PIDs, logs, and ephemeral state (gitignored).
- `Config/DevelopmentEndpoint.xcconfig`: generated Debug endpoint (gitignored).
- `Config/Production.xcconfig`: tracked Release endpoint seam.
- `operations/production.json`: non-secret production readiness declarations.
- `continuity.manifest.json`: runtime, guidance, and operator contract.

Use `bun .tohseno/machine.ts operations --json` to discover the pinned command
surface. Usually bring the app alive with `dev start`, check `dev status`, then
`ios launch`; use `--tunnel` only for a physical device or explicitly requested
remote access. Read logs through `dev logs` and stop through `dev stop`—never
kill by port. Run `bun run verify` after changes.

Preserve seed-phrase identity, crash-safe local persistence, account-free first
value, `App/AppConfig.swift` as the module seam, manifest truth, and ejection.
Keep credentials, prompts, private content, production data, and secret values
out of Git and output. Quick Tunnel URLs are public development reachability,
not authentication or production infrastructure.

A token launch (`token launch`) is an irreversible financial action under the
owner's own Bankr account. You may run `token status`/`token fees` and prepare
launch parameters, but never run `bankr login`, never request or relay OTPs,
never handle private keys, and never read or echo anything under `~/.bankr/`.
Present the economics and the irreversibility note to the owner in
conversation first, and invoke the operation without `--yes` so the
confirmation lands with the human — add `--yes` only when the owner has
explicitly said “launch it” with name, symbol, and chain stated in the current
conversation.

“Put this online,” “ship this,” or “send this to TestFlight” means: inspect
production first, explain the concrete blockers, and ask approval only for
accounts, costs, credentials, publishing, or external mutation. Production
inspection is implemented; broad provisioning, monitoring, recovery, and
deployment are not. Never imply otherwise, deploy a Quick Tunnel, run
`fastlane beta`, spend money, alter DNS, rotate credentials, or publish without
the owner’s explicit approval.
