# ADR 0036: Apple delivery is destination-driven

Status: accepted

Date: 2026-09-01

Supersedes:

- ADR 0027 only where its permanent selected-app handoff card assumes every
  installation travels over a cable;
- ADR 0033 only where a single currently visible iPhone is sufficient as the
  long-term installation destination; and
- current product copy only where it describes cable presence rather than
  CoreDevice reachability as the normal post-pairing delivery condition.

It retains ADR 0033's non-destructive living-project connection and verified
physical installation, ADR 0034's recipient-local Xcode signing and private Mac
factory, and ADR 0035's separation of Claim from Install. It changes no frozen
protocol encoding, public checkpoint, generation-0.8 ABI, Claims ABI, or public
installation fact.

## Context

Tohseno already discovers physical iPhones through Xcode's supported
`devicectl` JSON boundary. It treats an active paired local-network tunnel as a
real reachable device, retains a verified app when no usable phone is present,
and retries only installation. Multiple reachable devices stop the operation
instead of selecting the first result. The implementation is therefore already
partly wireless, while its state name and normal UI still say that the cable is
the destination.

That mismatch becomes unsafe as soon as more than one phone is visible. Initial
setup already persisted a one-way digest of the exact CoreDevice that received
and launched Companion, but app delivery did not consume that evidence. It
therefore stopped on multiple phones and, more seriously, could fall through to
one different reachable phone when the intended phone was absent. “The only
phone currently returned by Xcode” is a compatibility gate, not destination
identity.

Apple's current Xcode guidance distinguishes first pairing from later
reachability. A paired supported device can run apps through Xcode over Wi-Fi;
older iOS versions may still require a cable for initial pairing. Tohseno must
explain the condition Apple actually reports instead of promising wireless
behavior or demanding USB unconditionally.

## Decision

Installation is bound to an **Install Target**. USB and local network are
observed transports by which that target may be reachable. They are not the
identity of the target and are not public protocol facts.

The delivery sequence is:

```text
exact Claim or explicit Install intention
  -> exact release verification
  -> recipient-local Xcode build and signing
  -> retained verified artifact
  -> associated Install Target becomes reachable
  -> exact devicectl installation and bundle-inventory verification
```

The private Install Target model is versioned and supports more than one
target. It records only the minimum local facts required to select safely:

- a random local association ID;
- one paired Companion device ID;
- a keyed or one-way local digest of the stable CoreDevice identifier accepted
  by `devicectl`;
- a person-visible device name and product description;
- primary/disabled status, last-seen time, and last observed transport; and
- the explicit bootstrap evidence that created or replaced the association.

The raw CoreDevice identifier, device association, device name, Apple team,
and installation history remain private to the Mac and paired Companion. They
never enter a Claim, catalog release, Registry profile, relay plaintext, or
public analytics.

An association is created only through a bounded human-attended bootstrap:
the Mac must see exactly one eligible physical iPhone, the Companion command
must be attributable to one paired Companion device, and the person confirms
the named destination. A prior verified Companion installation on that exact
CoreDevice may be used as evidence, but Tohseno never infers an association
from display name, list order, USB presence, or the fact that only one device
was visible at some unrelated time.

Every install intention binds an association ID. When the target is not
reachable the artifact stays **Ready for your iPhone**. When it appears by any
supported transport, the Mac matches the observed CoreDevice identifier to the
persisted digest before installation. If the association is missing,
ambiguous, disabled, replaced, or mismatched, installation stops and asks for
explicit resolution. A request for phone A never falls through to phone B.

Reachability states describe actionable Apple facts: unknown target, target
unreachable, pairing or Trust required, Developer Mode required, locked,
reachable, installing, installed, and failed with a concrete reason. The exact
stored representation may stay smaller than this conceptual list. Transport is
secondary observation such as `localNetwork` or `usb`.

First setup may still ask for a cable when the observed OS/Xcode environment
requires it. After pairing, the normal language is **Connect your iPhone**,
**Ready for your iPhone**, or **reachable over Wi-Fi/USB**. The product does
not promise indefinite background installation: the Mac must be awake with the
Tohseno service available, and iOS/Xcode must make the intended phone reachable.

## Rollout and acceptance

The first additive source slice consumes the existing private Companion-setup
device digest as a single bootstrap association. If that target exists, every
app delivery matches it across all reachable CoreDevices regardless of USB or
local-network transport. If it is absent, another phone is never substituted.
Older records without that digest retain the exactly-one-reachable-device
compatibility fallback and fail closed for zero or multiple devices.

This bootstrap record does not yet implement the complete versioned,
multi-target, Companion-ID-bound association model above. In particular, it
has no attended replacement/reset operation or association ID on each install
intention. Those remain required migrations rather than facts inferred from
the bootstrap digest.

Source, unit tests, or Simulator behavior do not prove wireless delivery.
Acceptance requires owner-attended evidence for all of these:

1. initial cable pairing in an environment that requires it;
2. the cable removed and the same target reachable through Xcode's supported
   local-network path;
3. a retained exact-release artifact installed after later reachability;
4. multiple visible phones where only the associated target is eligible;
5. replacement/reset that disables the old association; and
6. restart of Mac app and service without losing the pending install target.

Until those facts are recorded, current state must say that destination-driven
association is designed or partially implemented, not released.

## Consequences

The Mac remains the private factory and delivery node. Companion remains human
authority. The relay transports durable intent but never chooses a physical
device. Apple retains control of pairing, Trust, Developer Mode, provisioning,
reachability, and installation.

The normal friend experience can truthfully say that the Mac prepares the app
and installs it when the intended iPhone is reachable. A cable remains an
Apple-controlled bootstrap or fallback mechanism, not the Tohseno product
identity.
