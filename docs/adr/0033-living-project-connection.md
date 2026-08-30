# ADR 0033: Tohseno maintains living iPhone projects

Status: accepted

Date: 2026-08-30

Supersedes:

- ADR 0025 and ADR 0032 where they describe app creation as the primary
  consumer value;
- ADR 0026 where the local Registry is a primary native destination; and
- ADR 0029 and ADR 0032 where first open culminates in a blank or seeded Shot
  creation composer.

## Context

A coding agent can create an iOS source project from a prompt. Asking a new
Tohseno owner to imagine and define an app before the product provides value
therefore creates unnecessary work and hides the part Tohseno can uniquely
own: the durable relationship between an app in use on an iPhone, its source
on the owner's Mac, its intentions and history, the coding harness that can
change it, and the build and installation path back to that phone.

The repository already contains useful lower-level machinery: a persistent
loopback service, authenticated and encrypted Companion commands, durable
offline outboxes, device revocation, one supervised coding-harness path,
physical-device observation, real Xcode signing, and `devicectl` installation.
Those mechanisms currently converge on protocol Shots created inside the
factory. Treating an arbitrary adopted Xcode project as a Shot would fabricate
public lineage facts and destructively imposing a Shot layout on its source
tree would violate owner-local repository boundaries.

## Decision

Tohseno's primary promise is: **create an iPhone app with any coding agent;
Tohseno keeps it connected, installable, and evolvable from your phone.**
Normal use starts from contact with an existing app and follows one loop:
request a concrete change in the Companion, durably deliver it to the one
associated source project, run the configured harness there, build and verify
the result, install it on the associated reachable iPhone, and retain the
request and result as that project's history.

Adoption is a first-class, non-destructive private product operation. It
inspects an owner-selected `.xcodeproj` or `.xcworkspace`, asks only for an
ambiguous app scheme when inference cannot resolve one, and stores a stable
random project identity in versioned private service storage. The record
points to the source tree and includes the resolved container, scheme, bundle
identifier, deployment target, signing-team identifier when Xcode exposes it,
Git state when present, relevant repository-instruction descriptors, harness,
device/build/install observations, and append-only evolution records. Moving
or losing the folder never silently creates a new identity. Adoption writes
nothing into the selected repository.

An adopted project is not a protocol Shot. Private Mac and Companion models
represent it as an adopted project, and the Companion sends a distinct signed
`project.evolve.request` carrying the stable project identity and observed
source-state token. Existing Shot creation and evolution remain available as
the secondary **Create app** path and retain their governed encodings.
No public protocol schema, lineage action, digest domain, contract activation,
or Registry authority changes.

The one existing harness supervisor is reused, with Codex as the first
concrete detected adapter. The execution packet tells the harness to inspect
before editing, follow repository instructions, preserve unrelated and dirty
work, implement only the request, test it, report changes, avoid destructive
Git operations, and stop honestly for credentials or a material decision.
Tohseno never commits, pushes, publishes, or deploys adopted source by default.

The delivery pipeline uses the selected project's own Xcode signing settings.
It builds with `xcodebuild`, verifies the produced app signature, installs with
the supported `xcrun devicectl` interface, and verifies the exact bundle in
the device's app inventory before recording **Installed**. A successful build
without device verification is **Ready to install**. Locked, unavailable,
untrusted, Developer-Mode-disabled, signing, source, test, build, and install
failures remain distinct durable outcomes with the smallest truthful recovery
action. Apple Account credentials are entered only through Xcode's own UI.

Onboarding establishes the connection rather than demanding an idea: explain
the product, check only missing Mac tools, select one usable harness, connect
and prepare an iPhone, install/open and securely pair the Companion, prove an
authenticated workspace exchange, then offer **Adopt existing app** as the
recommended action and **Create app** as the secondary action. The normal Mac
surface is the apps/projects list, connected personal iPhones, queued/current
work, and per-project history. The Companion's primary objects are those apps;
it is not a generic chat or creation dashboard.

Pairing remains the existing short-lived invitation, authenticated key
agreement, Keychain identity, encrypted local state, per-device capability,
durable relay queue, idempotent receipt, and real revocation path. Same-network
operation is the initial reliability target. The transport remains behind the
existing relay abstraction so an away-from-home route can be added without
changing project identity or evolution records.

## Consequences

The existing factory, capsule, and public-lineage machinery remains useful for
apps created by Tohseno and for provenance/restoration, but its terminology is
not required during ordinary adopted-project use. The local Registry is an
optional truthful detail rather than primary navigation. Studio's deleted
dashboard and internal execution phases remain deleted.

The private living-project store and signed Companion command are versioned
and migration-tested. Production code contains no mock success. Tests may
replace process/device boundaries with explicit fixtures, while release and
physical-device acceptance must exercise the real tools.

This decision does not authorize external billing, a cloud relay expansion,
Bankr, public distribution, registry publication, signing, notarization,
release activation, or deployment.
