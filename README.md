# Tohseno

Tohseno keeps a native iPhone app connected to the Mac project and coding
harness that evolve it.

Create an iPhone app with any coding agent, adopt its Xcode project in
Tohseno, and use the app. When it needs something, request that concrete change
from Tohseno Companion. The Mac durably receives the request, evolves the exact
project, builds it with Xcode, and installs the verified update on the reachable
owner iPhone. If Apple requires Trust, Developer Mode, unlock, or a cable,
Tohseno preserves the build and names that one action instead of claiming
success.

The generated-app factory remains available as a secondary way to make a first
app. It is not the product's center. The primary value begins once a concrete
project and working app exist.

This direction is governed by
[`ADR 0033`](docs/adr/0033-living-project-connection.md), building on
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

The implementation and current limitations are described in
[`docs/LIVING_CONNECTION.md`](docs/LIVING_CONNECTION.md).

## What we care about

- **Contact before imagination.** A working app and one noticed change are a
  better starting point than a blank product prompt.
- **Ownership.** Each app is ordinary SwiftUI and Xcode source, with no
  proprietary runtime and no Tohseno account required.
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
Mode, and an Apple Personal Team are needed for Companion and generated-app
installation; Tohseno never collects Apple credentials.

The normal product is `Tohseno.app`: open it, connect one working coding
harness, complete the observable Apple/Companion steps, and choose **Adopt
Existing App**. Select an exact `.xcodeproj` or `.xcworkspace`; Tohseno infers
the iOS app scheme and asks only when more than one real candidate remains. It
does not restructure the selected repository. The adopted app appears on the
Mac and in the paired Companion. After using it, open it in Companion and send
one text, voice, or screenshot-backed change request.

The app restores admitted work across window closure, service restart, and
ordinary phone/Mac relaunch. Plain Return sends from Mac intention composers;
Shift–Return adds a line. **Create App** remains a secondary path when there is
no existing project.
The optional Registry tab shows verified local Shots and the identity that
accepted them while explicitly separating that private track record from the
not-yet-connected public Registry.

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
The normal website action downloads the signed, notarized DMG directly. On a
Mac the page labels it **Download for this Mac**; while acceptance is pending,
its detail also says **Release candidate**. On another system it states the
real macOS 14-or-newer requirement. The retained shell installer is a
compatibility path, not the consumer door. The normal install requires no
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
