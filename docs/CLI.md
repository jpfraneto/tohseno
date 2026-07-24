# TOHSENO launcher and machine protocol

`tohseno` has three deliberately different presentation surfaces over one
factory:

- the human surface is an interactive launcher for a coding-agent conversation;
- Studio is a localhost browser contact sheet and creation door;
- the machine surface is deterministic, shot-scoped JSON for that agent.

Explicit CLI commands remain available for automation. CLI parsing, browser
request handling, and the application-level factory are separate layers:
`tohseno create` and `tohseno studio` both call the same `createShot` service.
Neither requires the other process to be running.

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

### Explicit creation input

The CLI can provide private, deterministic creation input without putting that
input in a shell argument:

```sh
tohseno create --file intention.md
tohseno create --file intention.md \
  --reference sketch.png \
  --reference flow.webp
```

`--file` accepts one regular, non-symlinked UTF-8 `.md` file up to 1 MiB.
`--reference` supplies optional context for that intention and is repeatable for
up to eight regular image files, each up to 12 MiB; the factory checks their
bytes rather than trusting extensions. A reference cannot replace the intention
file when no slug is given. Supported content is PNG, JPEG, WebP, GIF, HEIC, or
AVIF. If no slug is given, the shared allocator assigns the next `shot-NNN`.

An intention supplied this way selects the automated agent adapter: the agent
reads the private normalized input inside the shot, completes the app without
an extra product interview, and the shared factory verifies the result. The
classic `tohseno create <slug>` path without intention input still launches the
interactive agent conversation. `--no-launch` prepares and verifies the
baseline without starting an agent.

The normalized private input is written under:

```text
<shot>/.tohseno/provenance/
  intention.md
  references/reference-001.<detected-extension>
  provenance.json
  events.jsonl
```

`provenance.json` records the creation timestamp, CLI or Studio door, exact
factory release and bundle digest, normalized intention components, original
reference filenames, byte counts and SHA-256 hashes, creation options, and the
input digest. The entire provenance directory is gitignored. Tracked
`.tohseno/shot.json` carries only a non-content creation summary and digest, so
verification can bind the local private record to the immutable baseline
without committing the intention or images. Moving the complete shot working
directory preserves its private provenance; a Git clone intentionally does
not.

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

## Studio surface

Run:

```sh
tohseno studio
tohseno studio --port 4747
tohseno studio --no-open
tohseno studio --shots-dir /absolute/private/shots
```

Studio resolves the same configured shots directory as the CLI, starts one
local `Bun.serve` process on `127.0.0.1`, uses port `4747` by default, prints
its URL, and opens the system browser unless `--no-open` is present. It never
binds to the LAN. `SIGINT` and `SIGTERM` stop filesystem watchers, SSE clients,
an active creation job, and the owned live-preview helper before the process
exits.

The contact sheet discovers recognized direct children of `/shots`, orders
them newest first, and uses
`.tohseno/artifacts/screenshot.png` when a Simulator capture exists. A missing
capture is a normal fallback, not a broken shot. Detail pages expose the
recorded intention and references plus verify, run, live preview, Xcode, and
folder actions. The browser never constructs or accepts a general shell
command.

Contact-sheet `CREATION / …` labels and the detail page's
`LAST CREATION ACTIVITY` fact are derived from the portable creation journal.
`CREATING`, `INTERRUPTED`, and `READY` describe the last recorded creation
attempt, not a project lifecycle or the current run/verification state.

The creation form accepts:

- a long-form typed intention;
- one optional UTF-8 `.md` file;
- up to eight optional image references;
- an optional name.

At least typed text or Markdown is required. If both exist, normalization is
deterministic: `# Typed intention` and its text come first, followed by
`# Attached Markdown` and the file content. An omitted name uses the next
numbered `shot-NNN`.

Uploads first enter a mode-`0700` random directory below the managed local
Studio home. Internal random filenames replace untrusted path components.
Count, size, extension, UTF-8, MIME, and image magic-byte checks run before the
shared factory starts, and staging is removed on success, rejection, or safe
shutdown. Studio creates a mode-`0600` temporary launcher whose fragment
bootstraps an HTTP-only, path-scoped local session and is removed from browser
history immediately. The reusable credential is absent from the served shell,
process arguments, and printed base URL. Private reads and mutations both
require the session; unexpected Host and Origin values, non-loopback requests,
traversal, symlinks, and cross-site requests are rejected.

Studio intentionally permits one heavy Studio operation at a time across
create, run, preview, and verify. A conflicting request receives `409`; a
rejected creation also removes its staging. This does not create a separate
allocator: a simultaneous CLI creation and Studio creation still use the
shared owner-checked workspace lock and durable exclusive sequence claims, so
they cannot receive the same shot number even if a stale allocator resumes.

Structured factory events are appended to private JSONL journals under the
shots workspace. Studio streams its own job over server-sent events and watches
both the journals and atomically published shot directories, so a shot created
by a separate CLI process appears without that CLI calling Studio. Closing
Studio never makes the CLI or a completed shot unusable.

Studio is local-first, not a local gateway to a Tohseno service. The Studio
server does not upload intentions, references, generated source, app content,
credentials, or Simulator images. The locally selected coding agent consumes
creation input under that agent's own provider and privacy settings.

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

### Simulator doors

The human CLI and Studio use one application-level Simulator service rather
than duplicating build or launch commands:

```sh
tohseno run <shot-slug-or-path> [--shots-dir <path>]
tohseno preview <shot-slug-or-path> [--shots-dir <path>]
```

`run` resolves a recognized shot with a pinned regular
`.tohseno/machine.ts`, invokes its `dev start` and `ios launch` operations with
argument arrays, boots or selects an available iPhone Simulator, builds,
installs, and launches the app, then attempts an atomic capture at
`.tohseno/artifacts/screenshot.png`. The capture is private and gitignored; a
capture-only failure is reported but does not tear down a successfully launched
app or prevent live preview.

Before any global CLI or Studio door runs pinned verification or machine code,
it authenticates the shot's embedded content-addressed release record and
checks the regular files, pinned manifest/runtime tree, modes, and hashes. It
then executes a read-only private snapshot of those verified tools, not the
mutable shot-local copy. An available immutable factory cache is cross-checked
and reused; without it, the snapshot is reconstructed from the authenticated
embedded release. A modified or symlinked shot-local tool is rejected before
it can run.

`preview` performs the same run, starts one owned loopback helper for the
launched Simulator UDID, opens the interactive browser stream, and remains in
the foreground until `Ctrl-C`. Studio embeds that same capability in the shot
detail page. Only one live session is managed by a Studio process at a time;
closing Studio or stopping `preview` terminates its exact child and removes its
private temporary directory.

This is the Mac’s real native Apple Simulator streamed into a browser, not an
iOS emulator implemented in JavaScript and not an instrumented build of the
shot. Pointer, keyboard, swipe, gesture, and hardware-button input is forwarded
through the pinned
[`serve-sim` 0.1.45](https://github.com/EvanBacon/serve-sim) middleware. The
helper is capability-gated on `127.0.0.1`; it exposes only the allowlisted
preview/stream routes, not upstream command execution or a general terminal.

`run` requires macOS, Xcode command-line tools, healthy `xcrun simctl`, and an
available iPhone Simulator. Interactive `preview` additionally requires Apple
Silicon, a native arm64 Node.js 20 or newer, and the exact pinned `serve-sim` version
`0.1.45`. Simulator operation does not require paid Apple Developer Program
membership.

`tohseno doctor` reports platform, CPU architecture, the selected Node binary's
version and architecture, Xcode, simctl device inventory, exact serve-sim
compatibility, and the combined interactive-preview readiness. It therefore
catches an x64 Node running through Rosetta before starting the helper. Missing
preview support is reported as a warning. An explicit
`preview` command returns an actionable error, while Studio keeps its contact
sheet, creation form, ordinary run/verify actions, and CLI interoperability
available. Stopping the preview terminates its owned stream helper; the native
Simulator and the shot’s development service remain under their existing
Simulator and `machine dev stop` controls.

### Token operations (optional)

A shot may launch a token under the owner's own [Bankr](https://docs.bankr.bot)
account. This is an optional external action, not part of the core flow and
never the reason to build: TOHSENO ships no server, holds no keys, and takes
no fees.

```sh
tohseno machine token status --json
tohseno machine token launch --name <name> --symbol <sym> --chain base|robinhood --json
tohseno machine token fees --json
```

`token status` and `token fees` are read-only. `token launch` is an external,
irreversible financial action on the same side of the approval boundary as
deployment: in `--json` mode it refuses without `--yes` (exit `2`) and the
refusal identifies the exact name, symbol, chain, and fee recipient and states
that the action is permanent. Provider economics and limits can change;
TOHSENO deliberately does not quote or guarantee them. Review Bankr's current
terms before approving. One token per shot; a second launch is refused.

Install Bankr explicitly with `npm install -g @bankr/cli`, then authenticate
with `bankr login`. TOHSENO never falls back to `npx` for a financial action.
Credentials live in `~/.bankr/config.json` or `BANKR_API_KEY` and never enter
the shot repository, manifest, logs, or factory releases. Before broadcast,
TOHSENO requires Bankr's non-broadcasting `--simulate` path to succeed. On
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
It also scans the bounded public worktree for exact reference bytes and exact
or embedded private intention content. Creation invokes this gate after every
coding-agent exit, including failure. A failed gate moves the result to an
explicitly unsafe hidden sibling path instead of presenting the canonical shot
as ready.

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

These explicit commands remain implemented for scripts and contributors:

```sh
tohseno create [slug] [--file <intention.md>] [--reference <image> ...] \
  [--platform ios] [--agent codex|claude] [--no-launch] [--no-interactive] \
  [--shots-dir <path>]
tohseno list [--shots-dir <path>]
tohseno open <slug> [--shots-dir <path>]
tohseno doctor [--shots-dir <path>]
tohseno verify [slug-or-path] [--shots-dir <path>]
tohseno adopt <path> [--yes] [--no-interactive]
tohseno studio [--port 4747] [--no-open] [--shots-dir <path>]
tohseno run <slug-or-path> [--shots-dir <path>]
tohseno preview <slug-or-path> [--shots-dir <path>]
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
