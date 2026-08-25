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

```bash
npm i -g tohseno
tohseno
```

The first run guides you through connecting the phone, Apple development
signing, installing the TOHSENO Companion, and pairing it securely with your
Mac. It does not require `sudo`.

Then make something deliberately small:

```bash
tohseno create water-walk
```

TOHSENO opens Studio. Describe the app, optionally attach reference images,
and send the intention. The work continues in the local service if you close
the browser or Terminal. When the app is ready, TOHSENO installs and launches
it on the connected iPhone.

After you have used it:

```bash
tohseno evolve water-walk
```

Describe what should change. The update follows the same path into the same
app.

## Where your work lives

Apps are visible folders under `~/Desktop/Tohseno`, with one Git repository per
app. Private factory state, execution records, and pairing data live under
`~/.tohseno`. The installed service listens only on the Mac's loopback
interface.

The iPhone Companion is a remote control for the factory on your Mac. It can
send create and evolve intentions and receive encrypted status updates. It
does not receive source code or private harness output. When the optional relay
is used, it carries signed encrypted envelopes that the relay cannot read.

## Use it from the Terminal

The interactive path is the default, and the same operations are scriptable:

```bash
tohseno create my-app --prompt "An app that..."
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

The complete verification matrix is in [`AGENTS.md`](AGENTS.md). The current
stable release is **1.0.0**, available through npm and the public installer.

More detail:

- [Current runtime architecture](docs/ARCHITECTURE.md)
- [App → Intent → App decision](docs/adr/0016-app-intent-app-on-your-iphone.md)
- [Bounded build lifecycle](docs/adr/0019-bounded-intent-to-usable-app.md)
- [Cable setup, trial, and npm front door](docs/adr/0020-cable-genesis-earned-pro-npm-front-door.md)
- [Privacy boundary](docs/PRIVACY.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Protocol specification](protocol/SPECIFICATION.md)
