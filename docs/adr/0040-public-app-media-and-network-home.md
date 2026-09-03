# ADR 0040: Public app media and the network home

Status: accepted

Date: 2026-09-03

This decision extends ADR 0034's signed public catalog and clarifies ADR 0035's
public projections. It supersedes earlier website decisions only where `/` is
a static marketing or download page. It preserves ADR 0035's closed Discover
event set and rule that individual Claims do not flood `/registry`.

It changes no frozen protocol encoding, generation-0.8 ABI, Claims ABI,
Builder DeviceKey authority, source-integrity rule, Apple signing boundary,
Claim activation gate, or release gate.

## Context

A person deciding whether to Claim an app needs to recognize the actual app,
not only its name and prose description. The local factory already knows the
app's Xcode icon, and the Builder may have screenshots they intentionally want
to publish. Neither private source references nor the automatically captured
Simulator image may become public by implication.

The website root also needs to show what Tohseno is doing now. A static product
story hides the network's defining actions: someone Ships an app, someone Forks
an exact release into a new Shot, or someone Claims an exact encounter.

Claim must remain distinct from installation. The website cannot install an
app, fabricate a Claim, or treat a click as a receipt. Its useful action is to
send the exact Shot and release to Companion, where the human-authorized Claim
ritual occurs. Only canonical Claims may appear as events.

## Decision

### Signed presentation media

`tohseno.catalog-release/2` adds public presentation media to the existing
signed catalog release. Version 1 remains accepted byte-for-byte for existing
records.

Every version-2 release contains exactly one app icon:

- the CLI deterministically selects the largest valid PNG inside the app's
  Xcode `.appiconset`;
- it never substitutes Tohseno's private placeholder as app artwork;
- the release binds its SHA-256, exact byte length, and `image/png` media type;
  and
- preparation fails closed when no real bounded icon exists.

A Builder may explicitly choose zero through eight PNG or JPEG screenshots for
the exact Ship or Update. Each screenshot is a regular file of at most 10 MiB,
has unique bytes within the release, and is bound by SHA-256, exact byte length,
media type, and order. The Mac picker and repeated CLI `--screenshot` option are
the only normal selection surfaces.

Tohseno does not silently publish creation references, private source images,
the owner-local Simulator capture, prompts, paths, or other local material.
Companion approves the canonical version-2 release containing only digests and
bounded metadata. The Registry verifies image signatures and the signed
length/type before promotion, stores the bytes through the same create-only
durable blob boundary as source, and serves only catalog-referenced media from
immutable content-addressed URLs.

The canonical app page renders that release's icon and screenshots. An Update
may select a different set; older releases and their exact media remain
addressable and immutable.

### The root network timeline

`/` is the public Tohseno network home. It is a deterministic, newest-first
timeline containing only these canonical actions:

- **Ship**: the one first public release of a Shot;
- **Fork**: a new Shot whose first release binds an exact parent release; and
- **Claim**: one canonically confirmed `SoftwareClaimed` receipt for an exact
  release.

A Fork appears as one Fork item on the home timeline rather than a duplicate
Ship item. Updates remain visible in ADR 0035's `/registry` Discover timeline
and private Updates surfaces; they are not an additional root event type.
`/registry` otherwise retains ADR 0035's exact `shot.shipped`, `shot.updated`,
`shot.forked`, and `claim.edition_closed` projection, including its summarized
Claim counts and no-individual-Claim rule.

The home never inserts demo, optimistic, locally inferred, pending, orphaned,
or reorged events. When there is no canonical activity it says so.

### Claim sends the exact encounter to Companion

The app-page Claim action is an exact-release `tohseno://claim/...` deep link.
It opens that release in Companion; it is not an install button and the browser
click is not a Claim. Companion performs the existing disclosure and circle
ritual, signs through the person's Tohseno account authority, and waits for
canonical confirmation. Only then does the existing durable Companion path
queue that exact release for the person's Mac to verify, build, sign with their
Apple identity, and install on the intended iPhone when possible.

## Consequences

- New public releases use catalog version 2; all version-1 releases remain
  readable and verifiable without synthesized media.
- Public media adds no mutable CDN record, upload gallery, alternate catalog,
  or protocol generation.
- The Builder explicitly controls screenshot disclosure while the app icon is
  taken from the app being shipped.
- The website tells the Claim-versus-install truth at the decision point.
- Claims remain dark whenever the separately governed Claims activation or
  relayer is not live; the timeline does not compensate with inferred events.
