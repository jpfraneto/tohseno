# Local development

The public `0.5.0` installer manages its own pinned Bun runtime. Repository
work continues to use an existing Bun 1.2.18 or newer:

```sh
bun install
bun run check
```

The full gate runs strict TypeScript, Bun tests, manifest/protocol fixtures,
public-site and published-installer boundaries, deterministic artifact checks,
secret hygiene, and Git whitespace checks.

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

The focused acceptance suite uses fake agents and injected child-process
boundaries to cover strict planning and fallback, kernel/template/skill
composition, dependency order, collision and lock tampering, shared CLI/Studio
creation, immutable releases, private provenance, concurrent allocation,
Studio security, Simulator orchestration, authoritative handoff, installer
behavior, and protocol-state invariants.

## Develop Studio and the shared factory

Studio is served by the CLI package; there is no separate frontend build or
daemon:

```sh
bun test packages/cli/tests/creation.test.ts
bun test packages/cli/tests/studio.test.ts
bun test packages/cli/tests/simulator.test.ts
bun test packages/cli/tests
```

`creation.test.ts` proves that CLI and Studio call the same factory, normalize
typed text plus Markdown deterministically, hash and copy references, preserve
equivalent provenance, and allocate concurrent sequence numbers without
collision, including stale-owner resumption and no-clobber repository
materialization.
Studio tests cover the application/server boundary. Simulator tests
inject command executors and child processes, so the ordinary suite does not
require macOS, Xcode, or a booted device.

Run an isolated contact-sheet smoke test without opening a browser
automatically:

```sh
STUDIO_SMOKE_ROOT="$(mktemp -d)"
TOHSENO_HOME="$STUDIO_SMOKE_ROOT/factory" \
TOHSENO_SHOTS_DIR="$STUDIO_SMOKE_ROOT/shots" \
bun run tohseno -- studio --port 4747 --no-open
```

With `--no-open`, open the printed owner-only launcher file—not the unscoped
base URL—to establish the private browser session. Confirm that the empty
contact sheet loads, create a Shot from typed text or Markdown plus reference
images, watch structured progress, and verify that the resulting repository
appears under `$STUDIO_SMOKE_ROOT/shots`. In another terminal, point
`TOHSENO_SHOTS_DIR` at the same directory and run the source CLI’s `list`,
`verify`, `open`, or explicit `create --file` command; Studio should observe
the external creation without that CLI process contacting the server.

Studio must continue to reject non-loopback Host/Origin requests, mutation
and private-read requests without its path-scoped cookie, bootstrap requests
without the launcher token and exact same-origin headers, unknown multipart
fields, traversal, symlinks, oversized or mismatched uploads, and a second
simultaneous Studio creation. Closing it with `Ctrl-C` must delete the launcher,
clean staging, stop watchers, and await the active job. No manual test should
add a LAN bind or a general command endpoint.

## Rehearse the managed release

Rebuild the deterministic source artifact locally without replacing the
published release:

```sh
bun run tohseno:release
cat dist/tohseno-cli-0.5.0.json
```

The metadata identifies the current Git commit and marks a dirty worktree.
Ignored files are excluded from first-party release input. Dirty builds are
useful for local rehearsal but are never publication evidence; a published
artifact must be rebuilt from the exact clean frozen commit.

The installer test builds that artifact in a temporary directory, creates a
fake checksum-pinned Bun archive, removes Bun from `PATH`, installs to an
isolated home, creates and verifies a Blank Shot, proves it has no
inherited backend or credential requirement, re-runs the installer, and
confirms a bad checksum fails closed:

```sh
bun test packages/cli/tests/installer.test.ts
```

Supported script-only checks are:

```sh
sh -n apps/site/public/install.sh
sh apps/site/public/install.sh --help
sh apps/site/public/install.sh --dry-run
```

The default dry run reports the checksum-pinned public artifact without
installing it. No test modifies a real shell profile or contacts public
infrastructure.

## Exercise an isolated Shot manually

Use explicit temporary locations and skip agent launch:

```sh
SHOT_SMOKE_ROOT="$(mktemp -d)"
HOME="$SHOT_SMOKE_ROOT/home" \
TOHSENO_HOME="$SHOT_SMOKE_ROOT/factory" \
TOHSENO_SHOTS_DIR="$SHOT_SMOKE_ROOT/shots" \
bun run tohseno -- create docs-smoke \
  --no-launch --no-interactive

bun run tohseno -- verify docs-smoke
bun run tohseno -- run docs-smoke
```

## Develop the public site

```sh
bun run dev        # http://localhost:3000
```

The site is a stateless `Bun.serve` process with raw HTML/CSS and minimal
same-origin JavaScript. `.env.example` contains its only four settings:
`NODE_ENV`, `PORT`, `BASE_URL`, and `TRUST_PROXY`.

The site serves the checksum-pinned canonical installer at `/install.sh`.
There is no alternate bootstrap route; `/oneshot.sh` must remain absent:

```sh
bun test apps/site/tests/http-routes.test.ts
```

No local site command deploys Railway or publishes the CLI artifact.

## Develop the iOS kernel and templates

The project is XcodeGen-owned. After changing `project.yml` or adding,
removing, or moving Swift files:

```sh
cd templates/ios-kernel/overlay
xcodegen generate
```

Validate a changed manifest through the real CLI gate:

```sh
cd ../..
bun run validate templates/ios-kernel/overlay/app.manifest.json
```

On a Mac with an available iPhone simulator:

```sh
cd templates/ios-kernel/overlay
UDID=$(xcrun simctl list devices available --json | \
  bun -e 'const v=await Bun.stdin.json(); for (const ds of Object.values(v.devices)) for (const d of ds) if (d.isAvailable !== false && d.name?.startsWith("iPhone")) { console.log(d.udid); process.exit(0) }')
xcodebuild -project Shot.xcodeproj -scheme Shot \
  -destination "platform=iOS Simulator,id=$UDID" test
```

The neutral kernel and every bundled template must build with no keys. After
adding, removing, or moving Swift files in an overlay, regenerate its project
and any template project that explicitly replaces generated project files.

## Exercise Simulator run and live preview

Start with diagnostics:

```sh
bun run tohseno -- doctor
```

The native `run` path requires macOS, Xcode command-line tools, healthy
`xcrun simctl`, and an available iPhone Simulator. The interactive stream also
requires Apple Silicon, a native arm64 Node.js 20 or newer, and exact
[`serve-sim` 0.1.45](https://github.com/EvanBacon/serve-sim) compatibility.
`bun install` installs that pinned package. A paid Apple Developer Program
membership is not required for Simulator work.

When `doctor` reports a blocker, inspect the same owner-managed tools directly:

```sh
node --version
xcode-select -p
xcodebuild -version
xcrun simctl list runtimes
xcrun simctl list devices available
```

Open Xcode and install an iOS Simulator runtime if the last command has no
available iPhone. If command-line tools point at another Xcode installation,
set `DEVELOPER_DIR` for the Tohseno invocation or correct the owner-managed
Xcode selection, then rerun `doctor`. If `doctor` reports an x64 Node binary on
Apple Silicon, install or select a native arm64 Node rather than running it
through Rosetta. If only serve-sim compatibility fails,
rerun `bun install` in the repository and confirm the lockfile still resolves
exactly `0.1.45`; do not loosen the pin.

With a recognized generated Shot:

```sh
bun run tohseno -- run docs-smoke
bun run tohseno -- preview docs-smoke
```

`run` should start the pinned development runtime, build, install, and launch
the app in Apple Simulator, then attempt to write the real PNG capture to the
Shot’s gitignored `.tohseno/artifacts/screenshot.png`. If capture alone fails,
confirm that the command reports it while leaving the app running. `preview`
repeats that run,
opens the loopback interactive stream, and stays in the foreground until
`Ctrl-C`; test taps, typing, and a swipe before stopping it. The browser view is
the Mac’s real Simulator, not an in-browser emulator. Stopping the preview
stops its owned stream helper; use the existing native Simulator controls and
normal process controls when the app should also stop.

Only one live session is managed by a Studio process at a time. Confirm that
closing or replacing a preview terminates the exact previously owned helper
and that shutdown does the same. On an unsupported machine, confirm the
actionable `doctor` and preview diagnostics instead: the Studio contact sheet,
shared factory, CLI creation, and verification must remain usable.
