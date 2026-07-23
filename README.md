# TOHSENO

One sentence to a running iOS app.

```sh
curl -fsSL https://tohseno.com/install.sh | bash
tohseno
```

TOHSENO launches Codex or Claude Code inside an independent app repository,
then gives that agent deterministic tools to make the app run.

You describe the product. The agent handles the technical loop.

## The flow

1. Install TOHSENO.
2. Run `tohseno`.
3. Choose **Create something new** or **Continue a shot**.
4. Tell the coding agent what you want to make.

That is the human interface. Commands for APIs, databases, tunnels, simulators,
logs, and verification are agent tools.

For the complete iOS path, the machine needs macOS, Xcode, Git, and either
[Codex](https://github.com/openai/codex) or Claude Code. TOHSENO manages its own
pinned Bun runtime and can install a pinned `cloudflared` binary.

## What works today

Every new iOS **shot** starts with:

- a compiling SwiftUI app;
- BIP39 identity instead of an account screen;
- crash-safe local writing;
- a localhost Bun API with health checks;
- deterministic SQLite migrations;
- supervised start, status, logs, and stop;
- optional development-only Cloudflare Quick Tunnels;
- Debug endpoint injection and simulator launch;
- pinned manifest, privacy, provenance, and Git verification.

The agent discovers those operations from the shot itself:

```sh
bun .tohseno/machine.ts operations --json
```

The global CLI also exposes them under `tohseno machine ...` for automation.

## What stays yours

Each shot is its own Git repository. It carries its source, history, tests,
manifest, runtime playbook, migrations, landing page, and factory provenance.
It has no symlink back to TOHSENO and remains operable after the global CLI is
upgraded or removed.

Private product intentions, credentials, app content, development databases,
logs, generated endpoints, and signing configuration do not enter the factory
release or Git history. This repository operates no backend that receives
content from generated apps.

## Current boundary

iOS local development is **Implemented**.

Production inspection is **Implemented** and reports missing endpoints,
persistence, backups, secrets, and deployment capabilities without changing
external infrastructure.

Automatic production deployment, monitoring, recovery, DNS changes, TestFlight
submission, TokenMint, and SessionLink are **Proposed**. Accounts, credentials,
costs, publishing, and destructive operations always require human approval.
A Quick Tunnel is never a production endpoint.

## Learn more

- [Human setup and first run](docs/LOCAL_DEVELOPMENT.md)
- [CLI and machine operations](docs/CLI.md)
- [System architecture](docs/SYSTEM_ARCHITECTURE.md)
- [Production boundary](docs/DEPLOYMENT.md)
- [Ownership and ejection](docs/EJECTION.md)

## Contributing

```sh
bun install
bun run tohseno -- --help
bun run validate templates/continuity-app/continuity.manifest.json
bun run check
```

`bun run tohseno:link` is a contributor convenience, not the product install
path.

Apache License 2.0. The license grants no trademark rights to TOHSENO or Anky;
see [TRADEMARKS.md](TRADEMARKS.md).
