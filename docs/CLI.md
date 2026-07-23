# TOHSENO launcher and machine protocol

`tohseno` has two deliberately different surfaces:

- the human surface is an interactive launcher for a coding-agent conversation;
- the machine surface is deterministic, shot-scoped JSON for that agent.

The Phase 1 commands remain available under an advanced compatibility section.
They are not the product’s conceptual entrance.

## Human surface

Run:

```sh
tohseno
```

An interactive terminal sees exactly two top-level choices:

```text
What would you like to do?

  1. Take your first shot
  2. Continue a shot
```

When recognized shots already exist, the launcher shows the current local
count and changes the first choice to `Take another shot`. This is an honest
count of repositories in the configured shots directory, not a centralized
ecosystem counter.

### Take a new shot

TOHSENO asks for a human name, derives a safe slug, shows only platforms that
are actually implemented, detects installed agents, creates the independent
repository through the immutable factory-release machinery, verifies and
commits its baseline, and launches the selected agent there.

iOS is the only implemented platform. Codex and Claude Code are the supported
agent adapters. If exactly one is installed, it is selected automatically. If
both are installed, TOHSENO asks. A `defaultAgent` in config becomes the
default selection; a missing configured or shot-preferred agent is reported
instead of being silently invoked.

The agent process receives a narrow environment allowlist for terminal and
agent configuration. Arbitrary parent variables, `OPENAI_API_KEY`,
`ANTHROPIC_API_KEY`, `DEV_SECRET`, and unrelated provider credentials are not
forwarded. Its only prompt argument is constant:

```text
Read the local AGENTS.md and begin.
```

The shot is already committed before launch. If the agent exits or is missing,
the repository remains intact.

### Continue a shot

TOHSENO discovers only recognized direct children of the configured shots
directory. The picker shows each manifest name, iOS platform, Git worktree
state, and development state. It uses the configured default agent first, then
the shot’s recorded creation preference, while still handling a missing or
changed agent explicitly.

The shortcut below continues one exact slug and cannot be confused with a
built-in command:

```sh
tohseno the-trenches
```

### Non-interactive behavior

No-argument mode refuses a non-interactive terminal with exit `2`; it never
guesses answers. Automation uses explicit compatibility or machine commands.
Neither Codex nor Claude installed is a missing-dependency failure. Missing
Xcode or a simulator does not undo a successfully created shot or stop its API.

## Machine surface

Discover the protocol from inside a shot:

```sh
tohseno machine operations --json
```

The global CLI is a dispatcher into the pinned local implementation. The
ejectable equivalent is:

```sh
bun .tohseno/machine.ts operations --json
```

Critical runtime and verification behavior therefore does not change merely
because the global CLI is upgraded.

### Shot resolution

Machine operations use the nearest recognized shot from the current directory,
or one explicit target:

```sh
tohseno machine dev status --shot /absolute/path/to/shot --json
tohseno machine dev status --shot the-trenches --json
```

A slug resolves only under the configured shots directory. The CLI never scans
for and guesses a shot to mutate. `--shots-dir <path>` may override that
directory for a global invocation.

### JSON and exits

With `--json`, stdout is exactly one JSON protocol document. Content-free
diagnostics from subprocesses go to stderr. Success envelopes have this shape:

```json
{
  "schemaVersion": 1,
  "ok": true,
  "operation": "dev.status",
  "shot": "/absolute/shot",
  "result": {}
}
```

Failure envelopes contain `error.code`, `error.message`, and optional structured
`error.details`. Stable exits are:

| Exit | Code | Meaning |
|---:|---|---|
| `0` | success | Operation completed or inspection returned a valid report |
| `2` | `INVALID_CONFIGURATION` | Bad arguments, shot, manifest, or production configuration |
| `3` | `MISSING_DEPENDENCY` | Required executable, Xcode, simulator, or cloudflared is absent |
| `4` | `UNHEALTHY_SERVICES` | Startup/readiness failed or a managed service is unhealthy |
| `5` | `INTERNAL_FAILURE` | The operation itself failed unexpectedly |

### Development operations

```sh
tohseno machine dev start --json
tohseno machine dev start --tunnel --json
tohseno machine dev status --json
tohseno machine dev logs --service all --lines 100 --json
tohseno machine dev stop --json
```

`dev start` accepts:

- `--port <0-65535>`; default `0` asks the OS for an available port;
- `--readiness-timeout-ms <250-120000>`; default `15000`;
- `--tunnel`; starts an explicit Cloudflare Quick Tunnel;
- `--cloudflared <absolute-path>`; a test/operator override that also requests
  a tunnel.

Start is idempotent. A healthy repeated start returns the existing instance.
Changing a running instance from local-only to tunnel transport requires a
clean stop first. A per-shot directory lock serializes concurrent starts.

The background supervisor starts `Backend/server.ts`, runs SQLite migrations,
waits for `/health`, optionally waits for the Quick Tunnel URL, writes the Debug
xcconfig and runtime state atomically, then monitors its children. Process
records include an unguessable instance marker and expected command fragments.
Stale records are recovered. Stop uses a per-instance control request and
signals only command-identity-checked PIDs; it never kills by port.

Persistent development data lives under `.tohseno/data/`. Ephemeral state and
logs live under `.tohseno/run/`. Stop removes state and the generated endpoint
but preserves the database and logs. `dev status` returns exit `4` with the full
status in error details if a managed stack is unhealthy; a cleanly stopped
stack is a valid exit-`0` report.

Quick Tunnels are opt-in development/testing transport. They use a random
public hostname, have no uptime SLA, limited concurrent requests, and no
server-sent events. They are reachability, not authentication, and are never a
production fallback.

### iOS operations

```sh
tohseno machine ios inspect --json
tohseno machine ios launch --json
tohseno machine ios launch --device <simulator-udid> --json
```

Inspection reports Xcode and available iPhone simulators without mutation.
Launch requires a healthy development stack, boots a concrete simulator UDID,
builds Debug into the shot’s gitignored runtime directory, installs and opens
the app, and compares the built Info.plist endpoint with the active endpoint.

Physical-device launch remains an explicit Xcode signing/trust action. The
agent starts development with `--tunnel`, then guides the owner to run that
Debug configuration. It never edits Swift or stores the random URL in Release.

### Token operations (optional)

A shot may launch a token under the owner's own [Bankr](https://docs.bankr.bot)
account. This is an optional distribution and revenue mechanism, not part of
the core flow: TOHSENO ships no server, holds no keys, and takes no fees.
Trading fees accrue to the owner's wallet (95% of the 0.7% swap fee; 5% to the
protocol).

```sh
tohseno machine token status --json
tohseno machine token launch --name <name> --symbol <sym> --chain base|robinhood --json
tohseno machine token fees --json
```

`token status` and `token fees` are read-only. `token launch` is an external,
irreversible financial action on the same side of the approval boundary as
deployment: in `--json` mode it refuses without `--yes` (exit `2`) and the
refusal carries the full economics summary — 100B fixed supply, 85% pool / 15%
creator vesting over one year with a 30-day cliff — plus the exact rerun
command, so the confirmation always lands with the human. One token per shot;
a second launch is refused.

The only new human ritual is a one-time `npx @bankr/cli login email` per
machine. Credentials live in `~/.bankr/config.json` or `BANKR_API_KEY` and
never enter the shot repository, manifest, logs, or factory releases; a
missing CLI or missing credentials is exit `3` with that exact one-liner. On
success the token record (address, chain, tx hash) becomes a fact in
`continuity.manifest.json`. Fee claiming afterwards is the owner's business
with `bankr fees`, outside TOHSENO's scope.

### Verification and production inspection

```sh
tohseno machine verify --json
tohseno machine production inspect --json
```

Verification runs the shot’s pinned validator. A new shot checks manifest
truth, required structure, independent Git ownership, local provenance,
production endpoint safety, external symlinks, tracked private files, and
Git-ignore coverage for data, logs, runtime state, and generated endpoints.

Production inspection is read-only. It reports:

- whether `Config/Production.xcconfig` defines one stable HTTPS bare origin;
- localhost, loopback, HTTP, credentials/paths, and Quick Tunnel rejection;
- the declared single-instance SQLite path contract;
- backup configuration;
- unresolved secret references;
- readiness blockers;
- implemented, prepared, and proposed capabilities.

It does not deploy. The baseline declares production deploy, monitoring, and
recovery as proposed.

## Configuration

The optional file is `~/.tohseno/config.json`:

```json
{
  "schemaVersion": 1,
  "shotsDirectory": "~/tohseno/shots",
  "defaultAgent": "codex"
}
```

Precedence for the shots directory is `--shots-dir`,
`TOHSENO_SHOTS_DIR`, config, then `~/tohseno/shots`. `TOHSENO_HOME`
relocates the managed factory home for isolated tests or operator workflows.
Unknown config fields are rejected.

## Advanced compatibility commands

These Phase 1 commands remain implemented for scripts and contributors:

```sh
tohseno create <slug> [--platform ios] [--agent codex|claude] [--no-launch]
tohseno list [--shots-dir <path>]
tohseno open <slug> [--shots-dir <path>]
tohseno doctor [--shots-dir <path>]
tohseno verify [slug-or-path] [--shots-dir <path>]
tohseno adopt <path> [--yes] [--no-interactive]
```

Existing Phase 1 shots continue to use their pinned verifier. `machine verify`
has an explicit legacy fallback; other new machine operations report that the
old shot has no pinned runtime. Existing shots are never silently rewritten.
`adopt` remains narrow and explicit: it adds `.tohseno` rails to a compatible,
independent iOS repository and does not overwrite app code or agent manuals.

## Installer and release artifact

`apps/site/public/install.sh` installs under `~/.tohseno` without a pre-existing
Bun. It detects OS/architecture, downloads only versioned HTTPS artifacts,
verifies pinned SHA-256 digests, installs the source distribution plus a
managed Bun runtime, optionally acquires pinned `cloudflared` when it is
missing, creates one `~/.tohseno/bin/tohseno` wrapper, and makes an idempotent
marked PATH update unless disabled.

The source distribution is deliberate: the global CLI and every independent
shot use Bun for pinned TypeScript validation, SQLite, the backend, and local
machine operations. A standalone launcher would still have to install that
runtime and would create two execution paths. The managed-runtime design keeps
one verified engine and no global Bun prerequisite.

Installer test overrides accept local files or explicit localhost HTTP; normal
installs require HTTPS. `--non-interactive`, `--no-modify-path`,
`--without-cloudflared`, `--dry-run`, `--help`, and `--version` are supported.
The installer collects no credentials and installs no Git, Xcode, or coding
agent; it detects and reports those owner-managed dependencies.
