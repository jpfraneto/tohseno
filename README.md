# TOHSENO

## Take another one.

Every idea deserves a body. Most shots miss. That is why you take a lot of
them.

```sh
curl -fsSL https://tohseno.com/install.sh | bash
tohseno
```

Tell Codex or Claude Code what you want to make. Your agent opens inside an
independent iOS **shot** under your shots directory (`~/tohseno/shots` by
default) and keeps operating until the prototype is alive. You get the source,
history, data, and the next shot.

The prototype is the payoff. TOHSENO makes no promise of riches.

## The flow

1. Install TOHSENO.
2. Run `tohseno`.
3. Take your first shot, take another, or continue one.
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

Each shot is one frame on your contact sheet and its own Git repository. It
carries its source, history, tests, manifest, runtime playbook, migrations,
landing page, and factory provenance.
It has no symlink back to TOHSENO and remains operable after the global CLI is
upgraded or removed.

Private product intentions, credentials, app content, development databases,
logs, generated endpoints, and signing configuration do not enter the factory
release or Git history. This repository operates no backend that receives
content from generated apps.

## Optional: a token for your shot

A shot can launch a token — a distribution and revenue mechanism worth looking
into, never a requirement. The launch runs under your own
[Bankr](https://docs.bankr.bot) account through `tohseno machine token launch`:
TOHSENO ships no server, holds no keys, and takes no fees. Trading fees accrue
to your wallet (95% of a 0.7% swap fee). The agent prepares the parameters;
you approve an explicitly irreversible action; the machine executes it
deterministically. Details in [CLI and machine operations](docs/CLI.md).

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
