# Menlo

Menlo is a permissionless distribution network for iOS apps. A builder publishes
exact source and shares an app link; another person's Mac verifies the release,
builds it with Xcode, signs with that recipient's Apple identity, and installs
it on their intended iPhone. The direct path does not require App Store
submission, but it preserves Apple's provisioning, Trust, Developer Mode and
operating-system security requirements.

Menlo runs on Tohseno. The `tohseno` CLI, package names, bundle identifiers,
protocol encodings and existing domains retain their names. Companion holds
the non-exportable Builder DeviceKey and approves public actions. The Mac is
the one local factory; generation 0.8 remains the active public Registry witness.

The production website and Mac/Companion appearance use Menlo. The current
[Mac download](https://tohseno.com/download/macos) is **1.2.1-rc.1**, build
**10008**, from native source `9bd350d221b9cb04e632d9d40045d83adc87a3de`.
Its app and DMG were signed, notarized, stapled and Gatekeeper-verified; bytes
downloaded through the production route matched the immutable published digest.
It remains a release candidate, with clean-Mac and physical-iPhone acceptance
of this exact candidate unobserved.

**One upload per Builder is sponsored.** Retries of the same publication job
reuse its reservation. Later uploads, including Updates, require ETH funding.
The cap is deployed; paid-wallet setup and its address/Copy interface remain
unfinished, so subsequent uploads currently stop with a funding-required
message. This does not impose a fee on private local/BYO execution or change
Claim semantics. The existing BuilderAccount is not an ETH deposit wallet.

Start with the [Menlo documentation](https://docs.tohseno.com/). Exact rollout
evidence is in [`docs/MENLO_REBRAND.md`](docs/MENLO_REBRAND.md), the current
snapshot is [`docs/STATE.md`](docs/STATE.md), and the repository authority
hierarchy is [`AGENTS.md`](AGENTS.md). Frozen protocol bytes remain governed by
`protocol/`; accepted architectural decisions remain in `docs/adr/`.

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

The signed, notarized, origin-verified `v1.2.1-rc.1` DMG is active only on the
public release-candidate channel for independent acceptance. Stable activation still
requires the evidence in
[`docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md`](docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md).
Menlo retains the technical `Tohseno.app` bundle name, and the website action
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
