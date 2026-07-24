# System architecture

TOHSENO is a local agent launcher, browser Studio, immutable factory, and set
of independent app repositories. It operates no centralized content plane for
generated apps. Each new shot contains its own minimal API foundation.

```text
terminal launcher ─┐
explicit CLI input ├── shared createShot factory ── atomic /shots/<slug>
localhost Studio ──┘             │
                                 ├── private progress + input provenance
                                 └── pinned release + independent Git history
                                                    │
                                                    ▼
Codex / Claude ── pinned .tohseno/machine.ts ── API / SQLite / Xcode / verify
```

## 1. Public site and installation

`apps/site` is a stateless `Bun.serve` process for the landing page, human
docs, privacy notice, health, and two shell endpoints. It has no database,
forms, accounts, analytics, third-party browser resources, or path for app
content.

`/install.sh` is the canonical managed installer. It detects macOS/Linux and
arm64/x86_64, installs under `~/.tohseno`, verifies version-pinned SHA-256
artifacts, supplies its own Bun, acquires pinned cloudflared only when missing,
and installs one wrapper on the user PATH. Git, Xcode, Codex, and Claude Code
remain owner-installed dependencies and are reported rather than silently
acquired.

The CLI ships as a deterministic source artifact because Bun is also the
pinned execution engine for shot verification, SQLite, the API, and machine
operations. A compiled launcher would still need that runtime and would add a
second execution path.

`/oneshot.sh` is a thin compatibility entry point. Its pin is the direct parent
of the serving commit: the published CLI release commit. It downloads that
commit's canonical installer, verifies the installer SHA-256, and forwards
arguments. It contains no template copier, validator, shot creator, or agent
launcher and must never become a second factory in shell.

## 2. Entry doors and shared factory (`packages/cli`)

No-argument `tohseno` is the primary interface. It asks create or continue,
shows only the implemented iOS platform, detects Codex/Claude, prepares or
reuses an immutable release, creates the shot, and launches the selected agent
inside it.

The CLI does not interpret the product idea. The process argument is always:

```text
Read the local AGENTS.md and begin.
```

The agent then asks the owner. The child receives an environment allowlist,
not arbitrary provider secrets or a snapshot of the parent environment.

Continuation discovers recognized direct children only, shows manifest/Git/
runtime status, and respects explicit, configured, then shot-recorded agent
preferences. A missing or changed agent is surfaced.

`tohseno create --file`, `tohseno studio`, and the interactive launcher do not
own separate project generators. They adapt terminal prompts or browser
requests into the application-level `createShot` service. That service:

1. normalizes typed text, Markdown, and byte-validated image references;
2. allocates one sequence through an owner-checked workspace lock and a
   durable exclusive sequence claim;
3. prepares or verifies the immutable factory release;
4. stages provenance and the shot beside its final destination;
5. validates, initializes an independent Git history, and atomically publishes;
6. invokes the selected coding-agent adapter when requested;
7. verifies the result and optionally calls the shared Simulator runner;
8. emits structured events to presentation-independent sinks and private JSONL.

The CLI renders those events as terminal text. Studio streams the same event
objects over SSE. Workspace journals let a Studio process observe a separate
CLI process without either process calling the other. An interrupted
presentation sink cannot turn successful factory work into failure.

The lock record binds a process, random token, and filesystem inode. A resumed
stale owner cannot remove a replacement lock, and the exclusive claim files
remain the uniqueness backstop even if ownership changes mid-allocation. Final
publication first reserves the destination with an exclusive directory create,
then renames the complete staged shot over that private empty reservation.
Existing—even empty—paths are never replaced. Immutable shot metadata and
private provenance are checked after every agent exit, including failure; the
factory repairs them from the already-normalized in-memory input or safely
isolates a path it cannot repair.

## 3. Local Studio process

`tohseno studio` is one local `Bun.serve` process bound to `127.0.0.1`.
The configured `/shots` directory remains the source of truth; Studio has no
database and no Studio-only project format. It watches progress journals and
atomically published recognized shot directories, renders the contact sheet,
and routes creation and shot actions into the shared application services.
The read-only last-creation activity shown for a shot is derived from its
portable event journal. It is presentation metadata, not a lifecycle field,
and run or verification actions do not mutate immutable creation provenance.

The server accepts only expected loopback Host and Origin values. A random
per-process token exists only in a mode-`0600` temporary browser launcher. Its
URL fragment is consumed and removed from history, then exchanged under exact
same-origin checks for an HTTP-only, SameSite-strict cookie scoped to an
unguessable API path. The served shell, command line, and printed base URL do
not contain the credential; all private reads and mutations require the
session.
Uploaded names are metadata only: files receive internal random names in a
mode-`0700` staging root, and path, symlink, count, size, UTF-8, MIME, and
magic-byte checks run before factory work. Staging is cleaned on rejection,
success, interruption, and shutdown. No endpoint accepts arbitrary commands.

Studio permits one heavy Studio operation (create, run, preview, or verify) and
one owned live-preview helper at a time. That is an operational queue boundary,
not a shot lifecycle. CLI and Studio may still create concurrently because the
shared allocator prevents number collisions and each job uses an isolated
staging directory.

Studio shutdown aborts and awaits its active job, stops watchers and SSE
subscribers, and disposes the exact live-preview child. Contact-sheet and
creation behavior do not depend on Simulator support.

## 4. Immutable factory releases

Factory source is projected into:

```text
~/.tohseno/cache/releases/<release-id>/
  platforms/ios/base/       SwiftUI app, API, config, tests, landing page
  agent/continuity-app/     build protocol
  manifest/                 schema, types, validator, CLI gate
  shot/                     instructions, runtime, verifier, playbook
  factory/cli/              versioned launcher source
  legal/LICENSE
  release.json              provenance and SHA-256 inventory
```

Assembly uses a private staging directory and atomic rename. Finished bundles
are read-only and fully inventoried. The release ID records clean/dirty Git
provenance plus the content digest, or a pure content ID outside Git. Concurrent
equivalent builders converge. Cache corruption fails closed rather than being
silently healed.

Known private files, credentials, `.git`, dependencies, local setup values,
and symlinks are excluded. An atomic active pointer permits verified offline
reuse without guessing among multiple cached releases.

## 5. Shot publication and ownership

A shot is staged beside its destination. TOHSENO copies the iOS base,
customizes the manifest and app identifiers, installs pinned rails, validates,
initializes an independent Git repository, makes a neutral baseline commit,
and verifies again before one atomic destination rename.

```text
<shots>/<slug>/
  .git/
  .tohseno/
    shot.json
    factory-release.json
    provenance/             private, gitignored normalized input + events
    artifacts/              private, gitignored Simulator screenshot
    manifest/
    runtime/
    machine.ts
    verify.ts
    OPERATIONS.md
  AGENTS.md
  CLAUDE.md
  skills/continuity-app/SKILL.md
  App/ Tests/ Config/
  Backend/
  operations/production.json
  continuity.manifest.json
  project.yml Writing.xcodeproj/
  site/
```

No symlink points back to the factory or managed installation. Existing Phase
1 shots are not rewritten; they keep their pinned verifier and receive an
explicit legacy verification fallback from the global dispatcher.

The tracked `shot.json` records a content-free creation summary: door,
timestamp, input digest, reference count, options, sequence, and pinned release
identity. The full intention and copied references live only under the
gitignored `.tohseno/provenance/` directory with hashes and original filenames.
The verifier binds those private files to the tracked digest when they are
present and warns rather than inventing them after a Git-only clone. It also
walks a bounded public worktree, rejects unsafe links and exact reference
copies, and searches regular files for exact or embedded private intention
content. That gate runs after every coding-agent exit, even a nonzero one; an
unsafe result is atomically isolated under an explicitly hidden sibling name.

## 6. Shot-local machine runtime

The global `tohseno machine` command resolves one current or explicit shot,
authenticates its embedded release inventory, and dispatches through a private
read-only snapshot of the pinned machine. It does not execute the mutable
shot-local copy. JSON mode emits one protocol document on stdout; diagnostics
go to stderr. Stable failure classes are configuration, missing dependency,
unhealthy service, and internal failure. Direct
`bun .tohseno/machine.ts ...` remains the independently ejectable local door.

`dev start` acquires a per-shot directory lock and launches a detached
supervisor. The supervisor:

1. starts `Backend/server.ts` with a random instance marker;
2. binds to `127.0.0.1` and asks the OS for an available port;
3. opens the shot-owned SQLite file and runs ordered transactional migrations;
4. waits for readiness and `/health`;
5. optionally starts `cloudflared` and parses the generated Quick Tunnel URL;
6. atomically writes Debug endpoint and process state;
7. monitors the API and tunnel until an owned stop request or child failure.

State and content-free JSON logs live under `.tohseno/run/`; persistent
development SQLite lives under `.tohseno/data/`. A valid state records PID,
role, instance, and expected command fragments. Stop uses a per-instance file
and verifies command identity before signal fallback. It never kills by port.
Stale state, partial startup, reboot, concurrent starts, paths with spaces, and
multiple simultaneous shots are explicit test cases.

The API exposes `GET /health` and `GET /ready` only. Health includes non-secret
schema, platform, release, and uptime information. The baseline database stores
operational metadata, not device writing or identity.

## 7. Development transport and Simulator

The API is localhost-only by default. A Quick Tunnel is an explicit
development transport for a physical Debug device or remote test; its URL is
public reachability, not authentication. It has no production SLA, uses a
random hostname, has request/SSE constraints, and is kept only in ephemeral
state and a gitignored xcconfig.

Xcode has separate configuration roots:

- Debug includes `Config/DevelopmentEndpoint.xcconfig` only;
- Release includes `Config/Production.xcconfig` only.

The generated URL therefore never requires Swift or project-file replacement.
The Debug app validates localhost HTTP or HTTPS. The Release build script,
Swift runtime gate, and pinned verifier all require a stable HTTPS bare origin
and reject localhost, loopback, credentials, paths, and Quick Tunnels.
`NSAllowsLocalNetworking` permits local Debug access without weakening App
Transport Security globally.

`ios launch` inspects concrete available iPhone simulators, boots one, builds
Debug into gitignored DerivedData, installs and launches the bundle, and checks
that its Info.plist endpoint matches the active runtime. Xcode/signing/simulator
failures leave the created shot and API intact.

`tohseno run`, `tohseno preview`, and Studio call one Simulator service. The
service resolves a recognized shot and its pinned regular machine runtime,
authenticates it against the embedded content-addressed release record and,
when present, the immutable factory cache, then executes its private read-only
snapshot. It starts development, launches the native app, and attempts a PNG
through `xcrun simctl` into `.tohseno/artifacts/screenshot.png` using
temporary-file validation and atomic rename. A capture-only failure leaves the
launched app available. Commands are always argument arrays.

Live preview starts a separate owned Node child on `127.0.0.1` for the exact
launched Simulator UDID. It uses the pinned
[`serve-sim` 0.1.45](https://github.com/EvanBacon/serve-sim) middleware behind
a capability-bearing allowlist; the upstream general execution routes are not
exposed. Browser input controls the Mac’s real Apple Simulator. This is not an
in-browser iOS emulator, and the generated app is not instrumented.

Run/capture requires macOS, Xcode tools, healthy `simctl`, and an available
iPhone Simulator. The interactive stream additionally requires Apple Silicon
and a native arm64 Node.js 20 or newer. `doctor` derives its Studio readiness records from the
same diagnostic service. Unsupported live preview produces an actionable
blocker while the contact sheet, factory, and ordinary CLI remain operational.
Simulator use does not require paid Apple Developer Program membership.

## 8. Agent instruction system

`AGENTS.md` is canonical. `CLAUDE.md` is a small adapter pointing to it.
`skills/continuity-app/SKILL.md` carries the product/build protocol and
`.tohseno/OPERATIONS.md` carries stable operational detail. The short index
teaches an agent where app, backend, data, configuration, tests, health, logs,
and production inspection live without duplicating a giant manual.

The agent owns the technical lifecycle but not external authority. Accounts,
costs, credentials, publishing, DNS, destructive changes, production deploy,
and store submission require human approval.

## 9. Production contract

`production inspect` reads the manifest, tracked Release origin, and
`operations/production.json`. It reports stable HTTPS, single-instance SQLite,
backups, secret references, blockers, and capability status without mutation.

Inspection is implemented. Deployment, monitoring, recovery, VPS provisioning,
DNS, and store submission are proposed. The prepared fastlane lane is never
run automatically. This boundary leaves room for a future deployment cell
without pretending one exists today.
