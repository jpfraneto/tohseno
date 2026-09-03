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
[`ADR 0041`](docs/adr/0041-workshop-runtime.md),
[`ADR 0040`](docs/adr/0040-public-app-media-and-network-home.md),
[`ADR 0039`](docs/adr/0039-one-shot-living-workshop.md),
[`ADR 0038`](docs/adr/0038-npm-cli-init-first.md),
[`ADR 0035`](docs/adr/0035-claiming-software.md) and
[`ADR 0034`](docs/adr/0034-person-to-person-native-software.md), building on
[`ADR 0033`](docs/adr/0033-living-project-connection.md),
[`ADR 0025`](docs/adr/0025-native-macos-app-factory-managed-balance.md) and
[`ADR 0032`](docs/adr/0032-native-companion-onboarding-and-product-presence.md).
The native SwiftUI Mac application is the primary product over the same Rust factory.
It removes npm/browser first run, successful-day qualification, and
subscription gating of local/BYO execution. First setup installs and pairs the
Tohseno Companion as the real iPhone readiness proof. `v1.2.0-rc.10` is the
current signed, notarized, digest-pinned GitHub prerelease, built from exact
commit `a87bed012902dd11f78ea3922fa6fed25ed98dac`; its public download was
verified byte-for-byte. The production website's visibly labeled
release-candidate download now serves those same exact RC10 bytes. No RC10
physical-device behavior has been represented as observed.
[`docs/STATE.md`](docs/STATE.md) records the exact evidence and remaining human
and physical boundaries.

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

Claims is active only under its exact threshold-signed activation and live
Registry/runtime checks. One real Builder Ship and immutable Claim Edition are
recorded; the second person's canonical Claim, recipient-local build/signing,
and intended-iPhone installation remain unobserved and are not inferred from
source or tests.

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

The primary creation and evolution path automatically uses an installed,
authenticated coding provider already available on the Mac. One Advanced
disclosure allows an exact detected provider/model choice. Settings reports
provider availability directly and keeps custom executables and loopback
OpenAI-compatible endpoints subordinate. Local/BYO work has no Tohseno
subscription, trial, qualification, or balance gate. Tohseno-hosted
intelligence is coming soon; the incomplete managed-credits purchase surface is
not presented as a usable product.

The signed, notarized, origin-verified `v1.2.0-rc.10` DMG is active only on the
public release-candidate channel for independent acceptance. Stable activation still
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

When the exact paired devices are nearby, the Mac and Companion can also form a
separate authenticated local Workshop Session for low-latency capability
snapshots and ephemeral app events. It uses the existing pairing identities but
cannot perform or replace durable commands, Claim, Ship, Update, installation,
publication, payment, or revocation. The small Shot-facing package is
[`sdk/apple/TohsenoWorkshopKit`](sdk/apple/TohsenoWorkshopKit/); a Shot with no
Workshop declaration remains an ordinary focused app.

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
  Companion, durable private SDK, and ephemeral Workshop SDK.
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
swift test --package-path sdk/apple/TohsenoWorkshopKit
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
