# TOHSENO

Some useful apps are too specific to become products. They can still exist.

TOHSENO is an open-source iOS app factory that runs on your Mac. You describe a
small app you want; TOHSENO uses an explicitly selected local, bring-your-own,
or managed intelligence route, builds a native SwiftUI project, checks it, and installs it on your iPhone. The
source lives in an ordinary Git repository that belongs to you and can continue
without TOHSENO.

The current transition, governed by
[`ADR 0025`](docs/adr/0025-native-macos-app-factory-managed-balance.md) and
[`ADR 0030`](docs/adr/0030-system-aware-direct-native-download.md),
makes a native SwiftUI Mac application the primary product over the same Rust factory.
It removes npm/browser first run, mandatory Companion setup, successful-day
qualification, and subscription gating of local/BYO execution. The local 1.0.2
candidate now has Developer ID signing and Apple notarization evidence, but it
is not yet independently clean-Mac accepted, published, or the current public
download. One physical-iPhone birth and evolution has passed;
that proof predates the Registry-bearing candidate, so
[`docs/STATE.md`](docs/STATE.md) records the exact remaining boundary.

When using the app teaches you something, describe what should change. TOHSENO
evolves the same project and installs the new version on your phone.

## What we care about

- **Personal software.** An app can be worthwhile even when only one person
  needs it.
- **Ownership.** Each app is ordinary SwiftUI and Xcode source, with no
  proprietary runtime and no TOHSENO account required.
- **Local by default.** Builds, signing, device installation, and generated
  source stay on your Mac. A managed model sees admitted source only after an
  explicit privacy/cost choice and hard maximum.
- **Real completion.** A generated file is not the finish line. The app must
  build, pass its checks, install, and launch on the phone.
- **Bounded automation.** One intention gets one implementation attempt and,
  only for a concrete code or build defect, at most one focused repair.
- **Honest records.** TOHSENO records what happened and does not turn missing
  evidence into a success claim.

## Start here

You need macOS 14 or later and full Xcode. An iPhone, cable, Trust, Developer
Mode, and an Apple Personal Team are needed only for the final phone install;
TOHSENO never collects Apple credentials. Companion pairing is optional.

The normal product is `Tohseno.app`: open it, follow one readiness instruction
at a time, describe a deliberately small app, and press **Create App**. The app
restores admitted work across window closure and service restart. When the
deterministic gates pass, it installs and launches the result directly on the
connected iPhone. After using it, open the same app and describe the change.
Plain Return sends from every intention composer; Shift–Return adds a line.
The optional Registry tab shows verified local Shots and the identity that
accepted them while explicitly separating that private track record from the
not-yet-connected public Registry.

Advanced settings detect supported subscription-backed coding tools, allow a
bounded custom executable, or configure an explicitly consented loopback
OpenAI-compatible endpoint. Optional TOHSENO-managed intelligence uses prepaid
creation balance and always shows the server-priced estimate, privacy tier, and
hard maximum before source is sent. Local/BYO work has no TOHSENO subscription,
trial, qualification, or creation-balance gate.

The signed/notarized DMG is deliberately not claimed as published yet. Release
activation requires the evidence in
[`docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md`](docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md).
After activation, the normal website action downloads the signed, notarized
DMG directly. On a Mac the page labels it **Download for this Mac**; on another
system it states the real macOS 14-or-newer requirement. The download endpoint
currently fails closed rather than serving an unactivated artifact. The
retained shell installer is a compatibility path, not the consumer door. The
normal install requires no Terminal, npm, Node, Bun, or Homebrew.
Developers can build the unsigned universal bundle with:

```bash
macos/Tohseno/Packaging/build-app.sh
macos/Tohseno/Packaging/verify-app.sh dist/native/Tohseno.app unsigned
```

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

The optional iPhone Companion is a remote control for the factory on your Mac. It can
send create and evolve intentions and receive encrypted status updates. It
does not receive source code or private harness output. When the optional relay
is used, it carries signed encrypted envelopes that the relay cannot read.

## Advanced recovery and automation from Terminal

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
- [`macos/Tohseno/`](macos/Tohseno/) contains the primary native Mac app and
  distribution tooling.
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
swift test --package-path macos/Tohseno
swift test --package-path companion/apple/TohsenoCompanion
(cd website && bun run typecheck && bun test)
```

The complete verification matrix is in [`AGENTS.md`](AGENTS.md). Publishing a
signed native artifact, enabling managed Stripe/Bankr service, or activating
the public download remains an explicit owner action backed by external
evidence.

More detail:

- [Current runtime architecture](docs/ARCHITECTURE.md)
- [App → Intent → App decision](docs/adr/0016-app-intent-app-on-your-iphone.md)
- [Bounded build lifecycle](docs/adr/0019-bounded-intent-to-usable-app.md)
- [Native Mac product and managed balance](docs/adr/0025-native-macos-app-factory-managed-balance.md)
- [Keyboard-first Registry and native installer](docs/adr/0026-keyboard-first-local-registry-and-native-installer.md)
- [Native distribution runbook](docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md)
- [Managed-compute runbook](docs/runbooks/MANAGED_COMPUTE.md)
- [Privacy boundary](docs/PRIVACY.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Protocol specification](protocol/SPECIFICATION.md)
