# TOHSENO

Some useful apps are too specific to become products. They can still exist.

TOHSENO is an open-source iOS app factory that runs on your Mac. You describe a
small app you want; TOHSENO uses your existing Codex or Claude Code setup,
builds a native SwiftUI project, checks it, and installs it on your iPhone. The
source lives in an ordinary Git repository that belongs to you and can continue
without TOHSENO.

When using the app teaches you something, describe what should change. TOHSENO
evolves the same project and installs the new version on your phone.

## What we care about

- **Personal software.** An app can be worthwhile even when only one person
  needs it.
- **Ownership.** Each app is ordinary SwiftUI and Xcode source, with no
  proprietary runtime and no TOHSENO account required.
- **Local work.** Prompts, source, coding harnesses, builds, signing, and device
  installation stay on your Mac.
- **Real completion.** A generated file is not the finish line. The app must
  build, pass its checks, install, and launch on the phone.
- **Bounded automation.** One intention gets one implementation attempt and,
  only for a concrete code or build defect, at most one focused repair.
- **Honest records.** TOHSENO records what happened and does not turn missing
  evidence into a success claim.

## Start here

You need a Mac running macOS 13 or later, Xcode, an iPhone, and an authenticated
Codex or Claude Code installation.

The complete factory is free during a private trial that starts after iPhone
setup. The trial ends at the first of five successful distinct days or seven
calendar days. If you complete five successful days, you may choose Pro at
$9.99/month or $99/year to keep creating and evolving apps. If the trial ends,
your source, installed apps, Git repositories, and accepted history remain
yours; only new factory work locks. TOHSENO Pro does not include or require a
paid Apple Developer Program membership.

```bash
npm i -g tohseno
```

The install opens first run automatically and guides you through connecting the
phone, Apple development signing, installing the TOHSENO Companion, and pairing
it securely with your Mac. It does not require `sudo`. If you install with
npm's lifecycle scripts disabled, run `tohseno` once afterward.

Then make something deliberately small:

```bash
tohseno create
```

TOHSENO opens Studio. Describe the app, optionally name it and attach reference
images, and send the intention. If the name is blank, the implementation model
chooses one from the app's purpose. The work continues in the local service if you close
the browser or Terminal. When the app is ready, TOHSENO installs and launches
it on the connected iPhone.

After you have used it:

```bash
tohseno evolve water-walk
```

Describe what should change. The update follows the same path into the same
app.

## Where your work lives

Apps are visible folders under `~/Desktop/Tohseno`, with one initialized Git
repository and first commit per app. Private factory state, execution records, and pairing data live under
`~/.tohseno`. The installed service listens only on the Mac's loopback
interface.

Each app's `.tohseno/` directory is durable app-local metadata, not a cache and
not blanket-gitignored. Safe identity and integrity views may travel with the
repository. Exact intentions, inline-private lineage, references, feedback,
execution records, logs, and `.tohseno/private/` remain explicitly ignored;
publishing a Git repository is never allowed to silently publish them.

The iPhone Companion is a remote control for the factory on your Mac. It can
send create and evolve intentions and receive encrypted status updates. It
does not receive source code or private harness output. When the optional relay
is used, it carries signed encrypted envelopes that the relay cannot read.

## Use it from the Terminal

The interactive path is the default, and the same operations are scriptable:

```bash
tohseno create --prompt "An app that..."
tohseno create my-app --prompt-file intention.md --wait
tohseno evolve my-app --prompt "Make the first-run screen clearer" --wait
tohseno studio
tohseno service status
tohseno service logs
```

Existing app folders can also use the explicit recording layer:

```bash
tohseno init my-app
tohseno record my-app --note "Describe these exact files"
```

## Find your way around the repository

This repository contains the whole product:

- [`cli/`](cli/) provides the command-line surface.
- [`engine/`](engine/) runs the build, verification, recording, and delivery
  lifecycle.
- [`studio/`](studio/) is the local browser interface.
- [`companion/`](companion/) and [`sdk/apple/`](sdk/apple/) contain the iPhone
  Companion and its shared SDK.
- [`website/`](website/) serves the public site and the encrypted relays.
- [`protocol/`](protocol/) defines the exact public recording format and
  conformance rules.
- [`docs/adr/`](docs/adr/) records the accepted product and architecture
  decisions.

If you want a current plain-language map, begin with
[`docs/STATE.md`](docs/STATE.md). For the system boundaries, read
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). If you change governed behavior,
read [`AGENTS.md`](AGENTS.md) first: `protocol/` is authoritative over prose.

## Develop locally

The most useful first checks are:

```bash
cargo test --locked --workspace --all-targets --all-features
swift test --package-path companion/apple/TohsenoCompanion
(cd website && bun run typecheck && bun test)
```

The complete verification matrix is in [`AGENTS.md`](AGENTS.md). Release 1.0.2
is prepared as one coherent native and npm version; publishing its signed
native artifacts, public manifest, npm package, and website remains an
explicit owner release action.

More detail:

- [Current runtime architecture](docs/ARCHITECTURE.md)
- [App → Intent → App decision](docs/adr/0016-app-intent-app-on-your-iphone.md)
- [Bounded build lifecycle](docs/adr/0019-bounded-intent-to-usable-app.md)
- [Cable setup, trial, and npm front door](docs/adr/0020-cable-genesis-earned-pro-npm-front-door.md)
- [One-command npm installation](docs/adr/0021-npm-install-enters-first-run.md)
- [Privacy boundary](docs/PRIVACY.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Protocol specification](protocol/SPECIFICATION.md)
