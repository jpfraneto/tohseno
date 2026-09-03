# ADR 0039: One Shot is a living software workshop

Status: accepted

Date: 2026-09-03

This decision governs the native Mac and Companion product composition. It
supersedes earlier ADRs only where they project the same accepted capabilities
as a conventional sidebar, tab bar, app grid, settings shell, or progress
checklist. It preserves the one existing factory/service, adopted living
projects, one implementation harness, the selected-app Build/App/Source
workbench, Companion DeviceKey authority, publication and Claim flows, and all
protocol and release boundaries.

It changes no frozen encoding, generation-0.8 or Claims ABI, signed catalog,
Shot lineage, Ship-versus-Update rule, Claim semantics, intended-device rule,
Apple security boundary, or activation gate. It creates no alternate Studio,
factory, network, identity, or reputation system.

## Context

Tohseno's distinctive purpose is not to administer a list of projects. It is
to let software move between people: an intention becomes native software on
an iPhone, a Builder can Ship that exact release with the person holding the
Companion DeviceKey, and another person can Claim, verify, build, sign, and
install it with their own Apple identity.

The current native clients expose real capabilities, but largely arrange them
as navigation destinations, cards, forms, progress rows, and settings. That
composition makes the product resemble a project manager around the factory.
It obscures the physical relationship that the implementation already knows:
one Mac factory, an intended iPhone, apps that move between them, a Companion
holding human authority, and a public threshold crossed only by exact approved
evidence.

“One Shot” is the smallest coherent expression of that relationship. It is a
product model, not a new protocol term: one clear intention enters the existing
command path and produces one truthful attempt to make or change an app.

## Decision

The primary native projection is a single **living software workshop** shared
conceptually by the Mac and Companion.

The stable actors are:

- the **Mac factory**, where owner-local source, the one harness, Xcode builds,
  and distributable source are made;
- the **intended iPhone**, whose observed reachability, installation, and app
  inventory determine delivery truth;
- **Tohseno, the workshop keeper**, a state-derived guide whose presence,
  posture, and terse language can direct attention but can never sign, approve,
  or originate product truth;
- the **Companion human authority**, backed by the paired iPhone and its
  non-exportable DeviceKey, never an autonomous signer or second factory;
- **app objects**, each backed by an actual generated Shot or adopted living
  project; and
- the **network threshold**, which shows only locally known, pending, or
  independently verified public distribution state.

The scene derives entirely from existing service and Companion models. Unknown
device, build, installation, pairing, publication, or network state stays
unknown. Visual proximity, glow, motion, or sound cannot manufacture success.
Tests and preview fixtures are visibly fixtures and cannot become runtime
evidence.

### Mac workshop

The normal Mac window opens into the workshop rather than an administrative
sidebar. Apps appear as tangible objects in the scene; selecting one opens the
existing workbench capabilities without changing their source of truth. The
selected-app Build/App/Source views, activity, changed-file projection,
Simulator capture, cable handoff, evolution composer, and source reveal remain
available as the detailed workbench rather than becoming an execution-pipeline
dashboard.

The primary creation control is the **One Shot composer**. Its promise is
“Tohseno means One Shot”: write one concrete intention and choose **Take the
Shot**. It submits through the existing creation command and accepts the same
bounded image references. Plain Return submits while Shift-Return inserts a
line. No conception, planning, or second factory is inserted ahead of the
build.

The scene shows real chapter changes, not internal phases: bring the iPhone
into the workshop, take a Shot, build, ready to install, installed, or needs
attention. Tohseno's visible gesture and one short line derive from that same
chapter and cannot celebrate before its evidence. The six accepted application
states remain authoritative beneath that language. Pairing and first run are presented as bringing the intended
iPhone into the same workshop, with one concrete next action and optional
evidence detail rather than a celebratory progress checklist.

A compact command palette and an explicit list fallback preserve complete
capability access. `/` opens navigation/search only. Arrow keys move selection,
Return opens or submits the focused safe action, and Escape closes a transient
surface or returns to the workshop. No shortcut can Claim, Ship, Update,
install, revoke, spend, or otherwise cross an authority boundary.

### Companion workshop

Companion is the pocket view and controller of the same workshop. It remains a
private remote and human-authority surface, never the Mac factory. Its visible
capability migration is exact:

| Existing capability | Workshop destination |
| --- | --- |
| Apps | app shelf and selectable app objects |
| Registry | network threshold and public door |
| Updates | keeper inbox |
| Profile | keeper/authority bench |

Create remains the One Shot composer and existing signed commands remain the
only way it can ask the Mac to create or evolve software. Companion may show
offline, queued, awaiting approval, or reconnected truth from its durable
encrypted state, but visual continuity does not imply transport continuity.

### Visual and sensory rules

The first implementation uses replaceable, code-native vector forms and
native layout so the scene can evolve without introducing opaque binary truth.
Motion is restrained and state-driven. Sound and haptics are optional,
user-controllable acknowledgements and never the sole carrier of status.
Reduce Motion, contrast, VoiceOver labels, Dynamic Type where applicable,
keyboard focus, and a non-spatial list fallback are first-class. An accessible
label states the same real state that the visual actor represents.

## Capability preservation

The workshop is a projection migration. Registry/Discover, private Following
and Updates, Claim ritual, Builder profile, settings, recovery, source reveal,
adoption, physical delivery, and creation/evolution remain reachable. Advanced
configuration stays subordinate and does not become a room or fictional
actor. The deleted Studio dashboard, pipeline renderer, Feedback/Marketing
forms, protocol controls, raw harness output, and exact-Version UI remain
deleted.

The public path remains Builder → `tohseno init` → `tohseno deploy` → Companion
approval → one Ship → public route → another person → exact Claim → verified
source → recipient-local Xcode build and Apple signing → intended iPhone. A
workshop animation is never evidence that any step in that path occurred.

## Verification and release truth

Scene derivation is tested as pure state mapping. Native tests cover keyboard
contracts, capability reachability, accessibility labels, reduce-motion
behavior, and fixture compositions for empty, building, ready, installed,
offline, and attention states. Rendered screenshots may validate layout but are
not physical-device acceptance.

Release evidence must still distinguish implemented, locally verified,
deployed, observed on the real system, and physically accepted. This ADR does
not activate a DMG, Registry/Claims write path, R2 cutover, public release, or
installation. Those retain their own exact gates and owner-attended actions.

## Consequences

The native products gain one recognizable world aligned with Tohseno's core
mission: software moving from intention, through a person's own tools and
authority, to another person's iPhone. Existing capabilities may require fewer
top-level destinations, but none may disappear without an explicit later
decision. The scene can become richer only when real product state exists to
drive it; decorative simulation is not a substitute.
