# Local development

Repository work uses Bun 1.2.18 or newer. The product installer manages its own
Bun; a contributor checkout may use an existing one.

```sh
bun install
bun run check
```

The full gate runs strict TypeScript, Bun tests, manifest/contract fixtures,
public-site and installer boundaries, oneshot pin ancestry, secret hygiene, and
Git whitespace checks.

## Develop the launcher and machine protocol

```sh
bun run tohseno --
bun run tohseno -- --help
bun test packages/cli/tests
bun run --cwd packages/cli typecheck
```

The first command exercises the actual no-argument human experience. A source
link remains a contributor convenience:

```sh
bun run tohseno:link
tohseno
```

It is not the product installation path. Remove that checkout link with
`bun unlink --cwd packages/cli`.

All automated factory and runtime tests isolate `HOME`, `TOHSENO_HOME`, shots,
cache, `PATH`, Git configuration, process state, and paths containing spaces.
They must never touch the developer’s real `~/tohseno`, `~/.tohseno`, shell
profile, Git config, Keychain, simulator data, or public network.

The focused acceptance suite uses fake agents and fake child processes to
cover launcher selection, immutable releases, API/SQLite startup, readiness,
Quick Tunnel parsing, endpoint injection, logs/status/stop, stale ownership,
concurrent and multi-shot runs, production inspection, and legacy-shot
compatibility.

## Rehearse the managed installer

Build the deterministic source artifact without publishing it:

```sh
bun run tohseno:release
cat dist/tohseno-cli-0.2.0.json
```

The installer test builds that artifact in a temporary directory, creates a
fake checksum-pinned Bun archive, removes Bun from `PATH`, installs to an
isolated home, creates a shot, runs its API and fake tunnel, verifies and stops
it, re-runs the installer, and confirms a bad checksum fails closed:

```sh
bun test packages/cli/tests/installer.test.ts
```

Supported script-only checks are:

```sh
sh -n apps/site/public/install.sh
sh apps/site/public/install.sh --help
sh apps/site/public/install.sh --dry-run
```

The default dry run is valid only after the current artifact checksum has been
finalized. No test modifies a real shell profile or downloads a real public
tunnel.

## Exercise an isolated shot manually

Use explicit temporary locations and skip agent launch:

```sh
ROOT="$(mktemp -d)"
HOME="$ROOT/home" \
TOHSENO_HOME="$ROOT/factory" \
TOHSENO_SHOTS_DIR="$ROOT/shots" \
bun run tohseno -- create docs-smoke \
  --platform ios --no-launch --no-interactive

cd "$ROOT/shots/docs-smoke"
bun .tohseno/machine.ts dev start --json
bun .tohseno/machine.ts dev status --json
bun .tohseno/machine.ts verify --json
bun .tohseno/machine.ts dev stop --json
```

Do not add `--tunnel` to a manual smoke test unless public reachability is
actually needed. A real Quick Tunnel is not a CI prerequisite.

## Develop the public site

```sh
bun run dev        # http://localhost:3000
```

The site is a stateless `Bun.serve` process with raw HTML/CSS and minimal
same-origin JavaScript. `.env.example` contains its only four settings:
`NODE_ENV`, `PORT`, `BASE_URL`, and `TRUST_PROXY`.

`/install.sh` is the canonical prepared installer. `/oneshot.sh` remains a
non-mutating legacy migration notice with the previous creator pin until the
required public release and follow-up commit exist:

```sh
bash -n apps/site/public/oneshot.sh
bash apps/site/public/oneshot.sh --help
bash apps/site/public/oneshot.sh; code=$?; test "$code" -eq 2
```

No local site command deploys Railway or publishes the CLI artifact.

## Develop the iOS base

The project is XcodeGen-owned. After changing `project.yml` or adding,
removing, or moving Swift files:

```sh
cd templates/continuity-app
xcodegen generate
```

Validate a changed manifest through the real CLI gate:

```sh
cd ../..
bun run validate templates/continuity-app/continuity.manifest.json
```

On a Mac with an available iPhone simulator:

```sh
cd templates/continuity-app
UDID=$(xcrun simctl list devices available --json | \
  bun -e 'const v=await Bun.stdin.json(); for (const ds of Object.values(v.devices)) for (const d of ds) if (d.isAvailable !== false && d.name?.startsWith("iPhone")) { console.log(d.udid); process.exit(0) }')
xcodebuild -project Writing.xcodeproj -scheme Writing \
  -destination "platform=iOS Simulator,id=$UDID" test
```

The base must still build with no keys. Release intentionally fails until a
stable production API origin is configured; Debug simulator tests use the
separate development configuration. A generated shot’s agent can instead run
`machine dev start` followed by `machine ios launch`.
