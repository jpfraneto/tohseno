# MASTER_PROMPT — TOHSENO v0.6.0

You are the compiler. This file is the genome of the build. Read it completely before writing any code. Everything you produce must be derivable from what is written here. Where this file is silent, choose the simplest option that preserves the invariants.

## What TOHSENO is

TOHSENO is a printing press for iOS apps. A person on a Mac runs one command, describes an app in words and up to 8 images, and TOHSENO — driving their coding agent of choice — produces a complete iOS app installed on the iPhone connected to their Mac by cable. One shot in, one running app out. The person never reads documentation, because the pipeline is the documentation: the system tells them exactly one next step at a time, and does everything else itself.

TOHSENO is free software. It charges nothing. The only money in the system flows from the user to Apple ($99/yr developer account), and only when the user chooses permanence and App Store publishing. The free Apple ID path is the default and it fully works.

## Non-negotiable invariants

1. **Total function.** For any valid input (name + prompt + 0–8 images), the output is always a complete, buildable, installable iOS app. Never a partial result. If the generated code fails to build, the engine loops the errors back to the harness internally until it builds. The user experiences one shot.
2. **Shots are integers.** Every generation is a shot: `1, 2, 3…` per app. A shot is a complete world — full source, never a diff. Shots are append-only and never mutated or deleted by the engine.
3. **Shot number = CFBundleVersion.** The system's ontology and Apple's ontology are the same object. No mapping layer.
4. **Filesystem is the database.** No SQLite, no server-side state, no accounts, no telemetry, no network calls except: downloading the harness's model traffic (the harness's own business) and Apple's toolchain. Everything TOHSENO knows lives in plain files under `~/.tohseno/`.
5. **One handoff sentence at a time.** Every state in the pipeline either passes automatically or emits exactly one imperative sentence to the human and waits, verifying completion before advancing. If a step cannot be expressed in one sentence, the state machine is wrong — fix the machine, not the wording.
6. **Cable only.** Mac ↔ iPhone over USB. No wifi pairing, no network discovery. Refused branches are sentences nobody reads.
7. **Apple rails for everything.** `xcodebuild`, `xcrun devicectl`, free personal-team signing, standard SwiftUI apps with zero third-party dependencies in generated code. macOS is the only host platform.
8. **The harness is a plugin.** TOHSENO never talks to a model. It spawns the user's coding agent (Claude Code first) as a subprocess with a prepared workspace and streams its output. Harness choice is config, not code.

## Repository layout to create

```text
tohseno/
├── MASTER_PROMPT.md          # this file
├── LICENSE                    # already present, keep
├── TRADEMARKS.md              # already present, keep
├── README.md                  # short; the one-liner, the loop, the invariants
├── Cargo.toml                 # workspace
├── engine/                    # crate: tohseno-engine (library, no UI opinions)
│   └── src/
│       ├── ledger.rs          # shot directories, app registry
│       ├── machine.rs         # the gate state machine
│       ├── gates/             # one module per gate (toolchain, identity, device, sign, install, …)
│       ├── harness.rs         # subprocess contract, stream parsing, repair loop
│       ├── genome.rs          # loads genome/ and composes the shot workspace
│       └── events.rs          # the event types: Status, Handoff, Result, HarnessLine
├── cli/                       # crate: tohseno (the binary; thin frontend over engine)
├── studio/                    # embedded static web UI served by `tohseno studio` on localhost
├── genome/                    # the laws every shot starts from (see Genome section)
├── oneshot/oneshot.sh         # the installer, served at tohseno.com/oneshot.sh
└── .github/workflows/release.yml
```

The engine is a library crate with no terminal or HTTP code in it. The CLI and the studio are both subscribers to the same event stream. Rust stable, minimal dependencies (clap, tokio, serde, notify or similar; justify anything beyond that in a comment).

## The state machine

`tohseno create <app-name>` runs this line. Each gate: check → auto-pass, or emit one Handoff sentence → poll until verified → advance.

1. **toolchain** — Xcode + Command Line Tools present (`xcode-select -p`, `xcodebuild -version`). If missing, trigger install and continue other gates in parallel where possible; the download should overlap with the intent gate so waiting disappears into creating.
2. **identity** — an Apple ID is signed into Xcode (check for a development certificate / team via `security find-identity -v -p codesigning`). Handoff if absent: "Open Xcode → Settings → Accounts and sign in with your Apple ID."
3. **intent** — collect the shot input (see Input UX below). Produces `prompt.md` and `images/` in the shot directory.
4. **generation** — spawn the harness in the shot workspace (see Harness contract). Stream every line to subscribers: this is the theater — the user watches their app being written.
5. **repair** — `xcodebuild build` against a connected-device destination. On failure, feed the error log back to the harness with the instruction to fix and nothing else. Repeat until green or `max_repair_passes` (default 8) is exhausted; exhaustion is an engine bug surfaced honestly, not a user task. All passes live inside the same shot.
6. **device** — iPhone visible via `xcrun devicectl list devices`. Handoffs in order as needed: "Plug in your iPhone with a cable." → "Tap Trust on your iPhone." → "Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then let your phone restart." Poll after each.
7. **sign** — build a signed .app with automatic signing, personal team, `CFBundleVersion` = shot number, bundle id `com.tohseno.<username>.<app-name>` (username from `whoami`, sanitized).
8. **install** — `xcrun devicectl device install app`, then launch it.
9. **alive** — Result line: "shot N of <app-name> is on your phone."

`tohseno evolve <app-name>` runs the same line but the intent gate opens with the previous shot's source available to the harness as context, and the new shot is recorded with `parent = <previous shot>`. Evolution produces a new integer shot; it never edits an old one.

`tohseno refresh [<app-name>]` re-signs and re-installs the latest shot(s) — this is how free-tier 7-day expiry is made invisible. `tohseno list` prints apps, shots, and days-until-expiry from the profile inside each artifact.

**Slots.** Free Apple IDs allow 3 sideloaded apps on a device at once. Model this explicitly: creating a 4th app emits one line offering to retire one (`tohseno retire <app-name>` removes it from the phone, never from the ledger). Evolving never costs a slot. When a user hits an Apple wall that $99 removes (expiry fatigue, App Store desire), the upsell is exactly one Status line pointing at developer.apple.com — once per wall, never before.

## Shot ledger

```text
~/.tohseno/
├── config.toml                # harness command, defaults
└── apps/<app-name>/
    ├── app.toml               # bundle id, created_at, latest shot, parent map
    └── shots/0001/
        ├── prompt.md          # exactly what the user gave
        ├── images/            # 0–8 images, original filenames
        ├── src/               # the complete generated Xcode project
        ├── build.log          # full xcodebuild output, all repair passes
        ├── harness.log        # full harness stream
        └── artifact/          # the signed .app / .xcarchive
```

Shot directories are written once and never touched again. Greppable, diffable, ownable, forever.

## Input UX (the intent gate)

Match the feel of modern AI CLIs: a bordered multiline input box at the bottom of the terminal (Enter submits, Shift+Enter or Option+Enter for newline, paste-friendly). Three ways in, all first-class:

- **Type** the prompt directly in the box.
- **Drag and drop images onto the terminal window.** macOS pastes file paths into the input; detect absolute paths ending in png/jpg/jpeg/heic/webp anywhere in the submitted text, copy those files into the shot's `images/`, strip the paths from the prompt text, and confirm with one Status line per image ("attached mockup.png · 2 of 8"). Cap at 8; the 9th gets one line and is ignored.
- **`--prompt-file path/to/MASTER_PROMPT.md`** flag, or auto-detect: if the current working directory contains a `MASTER_PROMPT.md` when `tohseno create` runs, ask in one sentence whether to use it.

`tohseno studio` serves the same intake as a localhost web page (embedded static assets, no build step at runtime): drag-drop zone, textarea, and a live view of the event stream during generation. The studio talks to the engine over a local HTTP + SSE (or WebSocket) endpoint the engine exposes on 127.0.0.1 only.

## CLI output discipline

Every line the engine emits is one of exactly three kinds, and the renderer enforces it:

- **Status** — dim. The machine has the ball. `building shot 3…`
- **Handoff** — bright/bold. The human has the ball. One imperative sentence. Never two at once.
- **Result** — colored accent. `shot 3 of replyguy-trencher is on your phone.`

Plus the raw **harness stream** during generation, visually distinct (indented/dimmed) so the theater is watchable but never confused with TOHSENO's own three voices. No spinners with paragraphs, no walls of text, no emoji. The whole session should read like a ping-pong match transcript.

## Harness contract

`config.toml` holds `harness.command` (default: Claude Code in non-interactive/print mode with streamed output; verify the current flags of the installed `claude` binary at runtime with `--help` rather than assuming). The engine:

1. Composes the shot workspace: `genome/` contents + `prompt.md` + `images/` + (for evolve) previous shot's `src/`.
2. Writes a single `TASK.md` the harness reads first, containing: the genome's laws, the user's prompt verbatim, image references, and the output contract (a complete Xcode project in `src/` that builds with `xcodebuild` for iOS 17+, SwiftUI, zero external dependencies, app icon generated as solid-color placeholder with the app's initial if no icon image was provided).
3. Spawns the harness in that directory, streams stdout as HarnessLine events, waits for exit.
4. Runs the repair loop (state 5) by re-invoking the harness with `build.log`'s errors appended to TASK.md's repair section.

If no harness is found on the machine, the toolchain gate emits one Handoff pointing at the harness's own installer, then polls.

## Genome

`genome/` ships with TOHSENO and is copied into every shot workspace. Create it with these files, written by you now, kept short and law-like (constraints, not code):

- `LAWS.md` — the output contract: complete project, iOS 17+, SwiftUI only, zero dependencies, offline-first, no accounts or sign-in screens, no tracking, everything the app stores lives on-device, build must pass with automatic signing. One screen must be reachable and useful within 2 seconds of first launch.
- `STRUCTURE.md` — the exact Xcode project shape to generate (project.pbxproj expectations, target name = app name, Info.plist keys, where CFBundleVersion is injected by the engine — the harness leaves it as the literal token `__TOHSENO_SHOT__` and the engine substitutes).
- `TASTE.md` — minimal design tokens: system fonts, SF Symbols, respect dark mode, generous whitespace, no hamburger menus, no onboarding carousels.

No template project. No starter code. The genome is laws; every line of the app is drawn in the shot.

## Installer and release

`oneshot/oneshot.sh`: detects macOS + architecture, downloads the `tohseno` binary for the machine's arch from the GitHub release tagged `v0.6.0` on `jpfraneto/tohseno`, installs to `~/.tohseno/bin`, adds to PATH via the user's shell rc with one printed sentence, verifies with `tohseno --version`, and immediately (in the background) checks the toolchain gate so the Xcode download can start before the user's first `create`. The script accepts no secrets, sends no telemetry, creates no accounts.

`.github/workflows/release.yml`: on tag push `v*`, build release binaries for `aarch64-apple-darwin` and `x86_64-apple-darwin` on a macOS runner, attach both plus `oneshot.sh` to the GitHub release. Version in `Cargo.toml` is `0.6.0`.

## Definition of done

On a brand-new Mac with nothing but a browser and an iPhone with a cable:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
tohseno create replyguy-trencher
```

…the user types a prompt (or drops a MASTER_PROMPT.md and images), follows at most the handful of one-sentence handoffs (Xcode install, Apple ID, cable, Trust, Developer Mode), watches the harness write the app, and ends with replyguy-trencher running on their phone as shot 1. `tohseno evolve replyguy-trencher` then produces shot 2 from a follow-up prompt. `~/.tohseno/apps/replyguy-trencher/shots/` contains both complete worlds.

## Do not build

No publish/community features, no wifi pairing, no Android, no Linux/Windows hosts, no payments, no accounts, no analytics, no database, no cloud, no App Store submission automation, no chain integration. Every one of these is deliberately absent from v0.6.0.

## How to work

Work in this order: (1) engine crate skeleton with events + ledger + a machine that can run gates 6–8 against a pre-built hello-world .app you generate once by hand — the device pipeline is the risk, prove it first; (2) CLI renderer with the three voices; (3) intent gate input UX; (4) harness integration + genome + repair loop; (5) studio; (6) oneshot.sh + release workflow. Commit at each milestone with plain messages. When an Apple tool's flags differ from what's written here, trust the tool's `--help` on this machine over this file and note the deviation in a comment.
