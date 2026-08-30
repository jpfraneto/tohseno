# ADR 0032: Native onboarding installs the Companion and keeps Tohseno present

Status: accepted

Date: 2026-08-30

Supersedes:

- ADR 0025 only where it makes Companion setup unnecessary on the primary
  consumer path;
- ADR 0028 and ADR 0029 only where their first-open compositions omit a plain
  product explanation, device setup progress, and guided starting points; and
- ADR 0031's acceptance of `v1.0.2-rc.1`. That candidate failed independent
  clean-Mac product acceptance and must remain disabled and immutable.

## Context

The first public release candidate proved that a fresh Mac could obtain and
open the notarized bytes from `tohseno.com`, but it did not provide the product
experience expected at that door. Its DMG did not open as a familiar
drag-to-Applications composition, consumer-visible branding used `TOHSENO`
instead of `Tohseno`, and first open did not explain what the product does.

The same path installed and removed a disposable iPhone readiness app even
though the repository already contains the real Tohseno Companion build,
installation, launch, pairing, and private workspace synchronization
machinery. Installation progress was represented only by a spinner. A running
Mac factory had no persistent menu-bar presence. The creation composer also
opened on an empty text area without enough help for someone who does not
already have an app idea.

Codex discovery is additionally too narrow for a persistent LaunchAgent. It
checks the service's restricted `PATH`, a small set of home-relative paths,
and only one numeric NVM default alias. A real authenticated Codex installation
can therefore be invisible to the factory.

The public Registry remains a separate truth boundary. Generation 0.8.0 is
active, but Builder publication, registry RPC, catalog discovery, and secure
source download are not implemented. A local Shot is not an uploaded app.

## Decision

`Tohseno` is the consumer-visible product spelling. The Mac bundle, Finder
artifact, window and menu titles, onboarding, Companion display name, and
ordinary UI copy use that spelling. Existing uppercase protocol domain
separators, environment variables, test-vector bytes, and other governed
machine identifiers do not change.

The native DMG contains `Tohseno.app` and an Applications alias and persists a
Finder icon-view layout with a bounded window and obvious left-to-right drag
placement. Opening the mounted image presents that Finder window. Distribution
verification checks this exact bundle name and alias target.

First open is one native onboarding sequence that:

1. explains that Tohseno turns an intention into a native iPhone app whose
   source and history stay on the owner's Mac;
2. observes Xcode, cable, trust, Developer Mode, Apple signing, and the
   connected physical iPhone without claiming unobservable success;
3. builds, signs, installs, and launches the existing Tohseno Companion instead
   of a disposable readiness app;
4. creates the existing bounded pairing invitation and advances only after the
   Companion has proved pairing to this local workspace; and
5. shows a truthful staged progress bar while building, installing, launching,
   and pairing.

No second factory or command path is introduced. The Companion projects the
same local workspace and existing pairing protocol. Existing paired devices
remain valid.

While the Mac application process is running, macOS shows a menu-bar item made
from the repository's existing Tohseno SVG logo. The item reports whether the
local factory is opening, ready, or needs attention and can open the factory
window or quit Tohseno. It is product presence, not a claim that a background
service or public network is healthy.

Before an empty intention composer, the native creation surface offers a small
set of selectable iPhone-oriented starting capabilities. The connected device
name and product type may personalize the explanation when the locally
observed device gate provides them. Selecting capabilities deterministically
seeds editable natural-language intention text and submits through the one
existing Shot creation command. It does not create a second template factory
or infer hardware features that were not observed.

First-class harness discovery includes bounded, non-shell traversal of current
supported user install families, including the standalone `~/.local/bin`
location, Homebrew/global locations, Volta, npm-global, Bun, and installed NVM
Node versions even when NVM's default alias is absent or nonnumeric. An option
is usable only when its executable and existing authentication evidence both
pass the current checks.

The Registry labels locally verified records as **Apps on this Mac**. A
separate **Published apps** area may list only records returned and verified by
a real public registry implementation. Until that implementation exists, it
shows the explicit unavailable reason and no fabricated app cards, counts,
Builder authority, upload state, or publication controls.

## Consequences

`v1.0.2-rc.1` remains a rejected immutable prerelease and the public download
stays disabled. A replacement candidate requires a new version/tag, a clean
build, Developer ID signing, Apple notarization, stapling, exact hashing,
origin verification, clean-Mac acceptance, and an explicit candidate
activation under ADR 0031.

This decision changes native product composition and private setup state only.
It changes no public protocol encoding, Shot or Evolution semantics, registry
authority, contract activation, billing, managed-inference consent, or stable
release gate.
