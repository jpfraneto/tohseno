# Tohseno

Tohseno is a person-to-person native software network. A builder publishes an
exact, signed, buildable iPhone project; another person's Mac verifies it,
builds it with Xcode, signs it with that recipient's Apple identity, and
installs it on their iPhone.

This skips App Store submission and review for the direct person-to-person
path. It does not skip Xcode, code signing, provisioning, Trust, Developer
Mode, or Apple's operating-system security boundary.

The Mac is the factory. The iPhone Companion holds the non-exportable Builder
DeviceKey and approves public actions. The generation-0.8 Robinhood Chain
ShotRegistry plus the signed off-chain catalog are the shared public witness.
The generated-app factory and durable iPhone-to-Mac evolution path remain part
of the same product.

This direction is governed by
[`ADR 0038`](docs/adr/0038-npm-cli-init-first.md),
[`ADR 0035`](docs/adr/0035-claiming-software.md) and
[`ADR 0034`](docs/adr/0034-person-to-person-native-software.md), building on
[`ADR 0033`](docs/adr/0033-living-project-connection.md),
[`ADR 0025`](docs/adr/0025-native-macos-app-factory-managed-balance.md) and
[`ADR 0032`](docs/adr/0032-native-companion-onboarding-and-product-presence.md),
makes a native SwiftUI Mac application the primary product over the same Rust factory.
It removes npm/browser first run, successful-day qualification, and
subscription gating of local/BYO execution. First setup installs and pairs the
Tohseno Companion as the real iPhone readiness proof. Public candidate
`v1.0.2-rc.1` passed clean-Mac download and Gatekeeper but failed product
acceptance and is disabled. Replacement candidate `v1.0.2-rc.2` is the active
public prerelease for a second clean-Mac walkthrough. One physical-iPhone birth
and evolution has passed; that proof predates the ADR 0032 replacement, so
[`docs/STATE.md`](docs/STATE.md) records the exact remaining boundary.

The implementation and release state are described in
[`docs/STATE.md`](docs/STATE.md) and
[`docs/LIVING_CONNECTION.md`](docs/LIVING_CONNECTION.md).

The current 1.2 source adds Claim: a Companion-authorized, public,
non-transferable receipt for encountering one exact Shot release. Every Shot
ships once, later releases are Updates, and first Ship opens one immutable
Claim Edition. Registry is a canonical Discover timeline with private local
Following and a durable high-signal Updates inbox. Claim then durably asks the
recipient Mac to prepare that exact release; installation remains separate,
private, recipient-signed physical evidence.

Claims is intentionally inactive in current builds. No Claims address or
environment toggle is trusted without the separate threshold-signed activation
and live Registry/runtime checks. The preserved 1.1 candidate and currently
published installer are not silently relabeled as 1.2.

## What we care about

- **Contact before imagination.** A working app and one noticed change are a
  better starting point than a blank product prompt.
- **Ownership.** Each app is ordinary SwiftUI and Xcode source, with no
  proprietary runtime and no Tohseno account required.
- **Local execution.** Builds, recipient signing, and installation stay on the
  Mac. Source becomes public only through an explicit Companion-approved Ship.
- **Real completion.** A generated file is not the finish line. The app must
  build, pass its checks, install, and launch on the phone.
- **Bounded automation.** One intention gets one implementation attempt and,
  only for a concrete code or build defect, at most one focused repair.
- **Honest records.** TOHSENO records what happened and does not turn missing
  evidence into a success claim.

## Start here

You need macOS 14 or later and full Xcode. An iPhone, cable, Trust, Developer
Mode, and an Apple Personal Team are needed for Companion and generated-app
installation; Tohseno never collects Apple credentials.

Install the CLI from npm when you already have an Xcode project to publish:

```bash
npm install --global tohseno
```

The npm install has no postinstall download or GUI launch. Then enter the
project and follow the guided terminal path:

```bash
cd ExistingApp
tohseno init
tohseno deploy --app-slug your-app
```

Interactive `init` explains one step at a time and waits for Enter before
continuing. Before adoption, it checks the intended iPhone's real installed-app
inventory for the exact Tohseno Companion bundle and requires its private
pairing. If either is missing, it stops and directs you to
`tohseno companion install`; it never treats another phone or a remembered
local state as proof. It then adopts without restructuring or changing Git. `deploy`
snapshots safe source and waits for exact Companion approval before the first
Ship or a later Update. The optional slug is signed into the release and remains stable; after
the exact app's separate Companion-signed alias request and operator review it
can become `https://tohseno.com/your-app`. First Ship also fixes the Shot's one
Claim Edition. Once Claims is
separately activated, a recipient Claims the exact encounter on Companion;
canonical confirmation durably queues preparation on their Mac, which still
independently verifies the release before any build.

The app restores admitted work across window closure, service restart, and
ordinary phone/Mac relaunch. Plain Return sends from Mac intention composers;
Shift–Return adds a line. **Create App** remains a secondary path when there is
no existing project.
Registry shows real signed software events. Claim, Install, and Fork deep links carry
only immutable ShotID and release digest; the Mac resolves and verifies every
security-sensitive fact independently. Profile changes and global-alias
requests are signed on Companion. Aliases remain permissioned convenience
routes and never replace Shot identity.

The creation screen visibly lists detected subscription-backed coding tools,
including Codex, and advanced settings allow a
bounded custom executable, or configure an explicitly consented loopback
OpenAI-compatible endpoint. Optional Tohseno-managed intelligence uses prepaid
creation balance and always shows the server-priced estimate, privacy tier, and
hard maximum before source is sent. Local/BYO work has no Tohseno subscription,
trial, qualification, or creation-balance gate.

The signed/notarized `v1.0.2-rc.1` DMG is rejected. The signed, notarized, and
origin-verified `v1.0.2-rc.2` DMG is active only on the public
release-candidate channel for independent acceptance. Stable activation still
requires the evidence in
[`docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md`](docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md).
`Tohseno.app` remains the native Mac product and the normal website action
downloads its signed, notarized DMG directly. On a
Mac the page labels it **Download for this Mac**; while acceptance is pending,
its detail also says **Release candidate**. On another system it states the
real macOS 14-or-newer requirement. The retained shell installer is a
compatibility path, not the consumer door. That native-app install requires no
Terminal, npm, Node, Bun, or Homebrew.
Developers can build the unsigned universal bundle with:

```bash
macos/Tohseno/Packaging/build-app.sh
macos/Tohseno/Packaging/verify-app.sh dist/native/Tohseno.app unsigned
```

## Where your work lives

Adopted source stays exactly where the owner selected it. Its versioned private
pointer, stable Tohseno project ID, build/install observations, and evolution
history live under `~/.tohseno/service/living-projects-v1`. Generated apps are
still visible folders under `~/Desktop/Tohseno`. Private factory state,
execution records, and pairing records live under `~/.tohseno`; identities and
secrets use Keychain. The installed service listens only on Mac loopback.

Each app's `.tohseno/` directory is durable app-local metadata, not a cache and
not blanket-gitignored. Safe identity and integrity views may travel with the
repository. Exact intentions, inline-private lineage, references, feedback,
execution records, logs, and `.tohseno/private/` remain explicitly ignored;
publishing a Git repository is never allowed to silently publish them.

The iPhone Companion is the normal request surface for an adopted app. It sends
durably queued evolution requests and receives encrypted status/history. It
does not receive source code, raw harness output, credentials, or signing
material. The current transport uses the existing content-blind relay; it
carries signed end-to-end-encrypted envelopes that the relay cannot read.

## Advanced recovery and automation from Terminal

The interactive adoption path is the default. Generated Shot creation and
evolution remain scriptable recovery/secondary operations:

```bash
tohseno create --prompt "An app that..."
tohseno create my-app --prompt-file intention.md --wait
tohseno evolve my-app --prompt "Make the first-run screen clearer" --wait
tohseno studio
tohseno service status
tohseno service logs
```

Existing app folders can also use the historical explicit recording layer:

```bash
tohseno recording init my-app
tohseno recording record my-app --note "Describe these exact files"
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
- [`network/`](network/) defines signed catalog, deterministic source, build
  safety, and public release evidence.
- [`website/`](website/) serves the public site, Registry/catalog/blob service,
  constrained transaction relayer, and encrypted relays.
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
./scripts/test-network-e2e.sh
```

The complete verification matrix is in [`AGENTS.md`](AGENTS.md). Publishing a
signed native artifact, enabling managed Stripe/Bankr service, or activating
the public download remains an explicit owner action backed by external
evidence.

More detail:

- [Current runtime architecture](docs/ARCHITECTURE.md)
- [Living connection implementation and test](docs/LIVING_CONNECTION.md)
- [App → Intent → App decision](docs/adr/0016-app-intent-app-on-your-iphone.md)
- [Bounded build lifecycle](docs/adr/0019-bounded-intent-to-usable-app.md)
- [Native Mac product and managed balance](docs/adr/0025-native-macos-app-factory-managed-balance.md)
- [Keyboard-first Registry and native installer](docs/adr/0026-keyboard-first-local-registry-and-native-installer.md)
- [Native distribution runbook](docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md)
- [Managed-compute runbook](docs/runbooks/MANAGED_COMPUTE.md)
- [Privacy boundary](docs/PRIVACY.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Protocol specification](protocol/SPECIFICATION.md)
