# System architecture

TOHSENO is a local agent launcher, immutable factory, and set of independent
app repositories. It operates no centralized content plane for generated apps.
Each new shot contains its own minimal API foundation.

```text
human intention
      │ natural language
      ▼
Codex or Claude Code inside each shot
      │ structured local operations
      ▼
pinned .tohseno/machine.ts ── API / SQLite / tunnel / Xcode / verification
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

`/oneshot.sh` remains the non-mutating migration notice for the last published
rails creator. Its pin trails the serving commit by one and cannot change until
a CLI-containing commit and artifact are public. It must never become a second
factory in shell.

## 2. Human launcher (`packages/cli`)

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

## 3. Immutable factory releases

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

## 4. Shot publication and ownership

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

## 5. Shot-local machine runtime

The global `tohseno machine` command resolves one current or explicit shot and
delegates to `.tohseno/machine.ts`. JSON mode emits one protocol document on
stdout; diagnostics go to stderr. Stable failure classes are configuration,
missing dependency, unhealthy service, and internal failure.

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

## 6. Development transport and iOS endpoint

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

## 7. Agent instruction system

`AGENTS.md` is canonical. `CLAUDE.md` is a small adapter pointing to it.
`skills/continuity-app/SKILL.md` carries the product/build protocol and
`.tohseno/OPERATIONS.md` carries stable operational detail. The short index
teaches an agent where app, backend, data, configuration, tests, health, logs,
and production inspection live without duplicating a giant manual.

The agent owns the technical lifecycle but not external authority. Accounts,
costs, credentials, publishing, DNS, destructive changes, production deploy,
and store submission require human approval.

## 8. Production contract

`production inspect` reads the manifest, tracked Release origin, and
`operations/production.json`. It reports stable HTTPS, single-instance SQLite,
backups, secret references, blockers, and capability status without mutation.

Inspection is implemented. Deployment, monitoring, recovery, VPS provisioning,
DNS, and store submission are proposed. The prepared fastlane lane is never
run automatically. This boundary leaves room for a future deployment cell
without pretending one exists today.
