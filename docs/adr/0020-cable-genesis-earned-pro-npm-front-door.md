# ADR 0020: The cable is genesis; five successful days earn Pro

Status: accepted

Date: 2026-08-22

Supersedes ADR 0016 only where first-device setup and factory availability are
described. ADR 0016's App → Intent → App on your iPhone surface, deletion of
the dashboard, and thin Studio/Companion projections remain accepted. ADRs
0015 and 0019 continue governing the private channel, durable application
service, and bounded in-flight operation.

## Context

TOHSENO 0.9.0 has the correct factory rails but no coherent fresh-Mac door.
The public npm name exists without a working product, Mac↔Companion first run
uses a QR ceremony even though physical Apple installation already requires a
cable, and factory availability has no private trial or subscription
authority. Treating those as separate onboarding, pairing, and billing
projects would introduce extra concepts and likely extra authorities.

The existing device gate already distinguishes cable absence, Apple trust,
Developer Mode, Xcode/signing readiness, free and paid development teams,
physical install, launch, and accepted delivery. The existing Companion
invitation is signed, expiring, single-use, workspace-scoped, capability
granting, and encrypted. Those are the rails to project, not replace.

## Decision

TOHSENO 0.9.9 has one front door and one product loop:

```text
npm i -g tohseno
    → tohseno
    → cable genesis
    → App → Intent → App on your iPhone
```

### Cable genesis

The first Mac↔iPhone relationship begins physically. The normal sequence is:
pick up the iPhone, connect it by cable, trust the Mac, enable Developer Mode,
install Xcode when needed, add an Apple Account in Xcode, install the branded
Companion, complete the private relationship, and take the first Shot.

Studio presents one immediate instruction and at most one primary action.
Observable steps advance from the existing device, toolchain, signing,
installation, launch, and pairing gates; it does not ask a person to attest to
facts the Mac can inspect. When CoreDevice cannot inspect an earlier step
until Xcode exists, the surface guides the Xcode action and then returns to the
deferred check without claiming it passed.

The service builds the existing Companion target with the strongest usable
Apple development team selected by the existing paid/free preference,
installs it with CoreDevice, and launches it with Xcode `devicectl`'s supported
`--payload-url` option. The payload is the existing signed, bounded, expiring,
single-use `tohseno://pair/v1/…` invitation. The cable is the private delivery
origin for that one use; USB trust does not replace the Companion device
identity, invitation signature, key proof, encrypted capability grant,
revocation, or content-blind relay.

Recovery words remain a twelve-word, recoverable Companion identity shown
exactly once on the iPhone. Pairing pauses until the person records them. The
words never return to the Mac and never enter the launch URL, process
arguments, environment, build settings, logs, relay records, or Studio.

Mac↔Companion QR scanning is removed from normal genesis and reconnect. ADR
0018's distinct future browser-linking QR decision is unchanged and remains
unimplemented. Existing paired installations keep working and are not forced
through genesis again.

### Complete trial and earned purchase

The trial begins only after Companion installation, secure pairing, and a
durable genesis record all succeed. It is the complete product: no reduced
factory, separate app, watermark, sample path, or card requirement.

Private, versioned entitlement state on the Mac represents:
`genesis_incomplete`, `trial_active`, `trial_qualified`, `trial_expired`,
`pro_monthly`, `pro_yearly`, and `pro_lapsed`. This is private product state,
not a Shot, Version, public node object, contract object, or lineage action.
The phone receives only the minimum signed private projection needed to render
the same human state.

A successful day is one local calendar date on which a durable factory command
produces a new accepted Version after the existing physical build, install,
launch, and acceptance gates. Create and Evolve count equally. Failures,
waiting, source generation, harness exit, opening a surface, and genesis do not
count. Multiple results on one date count once; command, execution, and Version
bindings make retries idempotent without storing intention bytes.

The trial ends at the first of five successful distinct days or seven calendar
days after genesis. The fifth accepted operation reaches its terminal result,
then new mutations lock. Qualified people may consciously choose TOHSENO Pro
at $9.99 per month or $99 per year. Seven-day expiry without five successful
days does not offer purchase. There is no subscription and no cancellation
before a qualified person completes checkout.

### Hard boundary and preservation

Factory admission is enforced in `ShotApplicationService`, below Studio, CLI,
and Companion. New Create/Evolve mutations are rejected before the durable
command journal when genesis or entitlement is locked. Work already admitted
may reach its deterministic terminal result and may count its successful day;
the next admission observes the boundary. Read-only integrity, diagnostics,
export, renewal/receipt refresh, billing recovery, and safe uninstall remain
available, and the Local Workspace Service remains running.

The paywall never deletes or rewrites app folders, source, Shots, accepted
Versions, journals, identities, pairing state, or installed applications.
Generated apps contain no entitlement client or remote kill switch and remain
ordinary independent apps.

### Pro and Apple membership are independent

TOHSENO Pro unlocks the local factory. Apple development membership governs
provisioning and distribution. A Pro user may use a free Personal Team,
including its approximately weekly provisioning and three installed
development-app limit. Replacing an installed app remains possible at the
limit. A paid Apple Developer Program team enables Apple's longer-lived
provisioning, wider device/app capacity, TestFlight, App Store distribution,
and additional capabilities. It is not called “Apple Pro,” is never required
to buy TOHSENO Pro, and TOHSENO never claims it can purchase or guarantee it.

### Billing boundary

Qualified local installations create privacy-minimal, opaque, one-use checkout
claims for the existing website/server boundary. Hosted checkout supports the
two explicit plans. Verified, idempotent webhooks produce server-signed,
installation-bound entitlement receipts; the Local Workspace Service verifies
them against a pinned public key. Browser redirects never prove payment and no
billing signing secret ships in the CLI, npm package, Companion, Studio, or
repository. Cancellation at period end preserves access through the paid
date; lapse locks new mutations without deleting anything. Production billing
configuration fails closed. Test providers and clocks are debug/verification
only and cannot be activated accidentally in release builds.

### npm front door and release authority

`packages/cli` contains `tohseno@0.1.0`, a dependency-free Node 20+ ESM
bootstrap. It supports installation/opening/doctor/help/version, then delegates
unknown commands to the explicit installer-owned native launcher at
`~/.tohseno/bin/tohseno`. It never reimplements the factory or recursively
invokes itself.

The npm bootstrap accepts native artifacts only through a versioned official
HTTPS manifest with allowlisted origins, exact size and SHA-256, architecture,
layout, minimum npm compatibility, and expected signing metadata. It reuses
the repository's release-package integrity and no-sudo, user-owned,
rollback-safe installer layout. npm publication, the native release, manifest
publication, and installer authorization are separate owner actions.

### Migration

An already-paired pre-0.9.9 installation retains pairing, identity,
capabilities, workspace, accepted history, and in-flight commands. Its first
0.9.9 service observation is a deterministic trial anchor with zero successful
days; app count never fabricates days. Isolated development verification may
use compile/debug-gated fixtures, but release builds cannot inherit a test or
grandfather flag.

## Consequences

The product adds a private commercial availability boundary without altering
public protocol bytes or generated apps. The cable reduces first run to the
physical Apple truth already required, while the cryptographic Companion model
remains the relationship authority after bootstrap.

This decision does not authorize npm publication, a tag or GitHub release,
installer repinning, production billing, relay/APNs/DNS activation, contract
generation or deployment, or credential use. Each external activation remains
fail-closed until its existing release and owner gates are satisfied.
