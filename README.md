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
2. Run `tohseno` or open the local contact sheet with `tohseno studio`.
3. Take your first shot, take another, or continue one.
4. Tell the coding agent what you want to make.

The terminal launcher and Studio are two doors into the same factory. They
allocate from the same configured `/shots` directory, apply the same pinned
release, create the same independent Git repository, and use the same
verification and provenance rules. Studio is not a daemon dependency: the CLI
continues to work when Studio is closed.

Explicit creation input is also available from the terminal:

```sh
tohseno create --file intention.md --reference sketch.png
```

That command allocates the next numbered shot when no slug is supplied.
`--reference` may be repeated. The ordinary interactive launcher still keeps
the product conversation between the owner and the selected coding agent.

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

The local Studio contact sheet is **Implemented**:

```sh
tohseno studio
```

It binds to `127.0.0.1`, reads the configured `/shots` directory, observes
shots made by either door, and can create, verify, run, and inspect them.
Studio itself does not upload shots or private creation input; the selected
coding agent uses that input under its own provider and privacy settings. Port
`4747` is the default;
use `--port <port>` to override it, `--no-open` to leave the browser closed, or
`--shots-dir <path>` to select the same explicit workspace as other commands.
Studio permits one heavy Studio operation (create, run, preview, or verify) at
a time and one managed live preview; a separate CLI process still uses the
shared concurrency-safe allocator.

On a supported Mac, `tohseno run <shot>` builds, installs, launches, and
attempts to capture the shot in Apple Simulator. A capture failure is reported
without stopping an otherwise running app. `tohseno preview <shot>` adds an
interactive browser stream of that same native Simulator. This is not an
in-browser iOS emulator. The live stream uses the pinned
[`serve-sim` 0.1.45](https://github.com/EvanBacon/serve-sim) package and
requires macOS on Apple Silicon, a native arm64 Node.js 20 or newer, Xcode command-line tools,
and an available iPhone Simulator. Simulator use does not require a paid Apple
Developer Program membership. Run `tohseno doctor` for exact readiness
diagnostics. If live preview is unsupported, the contact sheet, creation, CLI,
and ordinary verification remain available.

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

When `create --file` or Studio receives an intention or references, the
normalized private input stays inside the shot at
`.tohseno/provenance/`. That directory is gitignored and contains the intention,
copied references with their original filenames and hashes, factory/door
metadata, options, and structured creation events. The tracked shot metadata
contains only a non-content summary and input digest. Temporary Studio uploads
are removed after the job succeeds, fails, or stops safely.

Private product intentions, credentials, app content, development databases,
logs, generated endpoints, signing configuration, and Simulator captures do
not enter the factory release or Git history. This repository operates no
backend that receives content from generated apps.

## Optional: a token for your shot

A shot can launch a token — a distribution and revenue mechanism worth looking
into, never a requirement. The launch runs under your own
[Bankr](https://docs.bankr.bot) account through `tohseno machine token launch`:
TOHSENO ships no server, holds no keys, and takes no fees. Trading fees accrue
to your wallet (95% of a 0.7% swap fee). The agent prepares the parameters;
you approve an explicitly irreversible action; the machine executes it
deterministically. Details in [CLI and machine operations](docs/CLI.md).

## Current boundary

iOS local development, the shared CLI/Studio factory, the local contact sheet,
Simulator run/capture, and supported-Mac live preview are **Implemented**.

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
