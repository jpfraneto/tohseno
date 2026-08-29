# ADR 0027: The native app workspace makes the build and iPhone visible

Status: accepted

Date: 2026-08-28

Supersedes:

- ADR 0016 only where it moves every cable affordance out of permanent chrome
  and limits normal progress to a single status sentence. Its App → Intent →
  App abstraction, six-state projection, simple vocabulary, Details boundary,
  and deletion of the Studio dashboard and internal execution pipeline remain
  accepted.
- ADR 0025 only where its initial native detail screen places the evolution
  composer and an optional preview in one long undifferentiated surface. Its
  one-factory architecture, native-session boundary, readiness, managed
  inference, packaging, and release gates remain accepted.

## Context

The native app made creation dramatically simpler, but an app being built
still looked inert. The selected-app screen led with another large intention
box while the factory's human-readable progress, changed source, Simulator
capture, and automatic cable handoff were either absent or secondary. That
made the product ask for the next change before making the current work
understandable.

The factory already records a bounded semantic event journal and computes the
source tree changed from each execution's own Git baseline. It also boots an
iPhone Simulator, installs and launches the generated app, and captures its
first screen. Those owner-local facts can make the app legible without
restoring the administration concepts ADR 0016 deleted.

Apple does not provide a supported public API for embedding its interactive
Simulator application inside another app. A static or refreshed capture must
therefore never claim to be an interactive embedded Simulator.

## Decision

### One native workspace with three small destinations

Selecting an app opens a native **Build / App / Source** workspace.

- **Build** is the default while work is happening. It shows a four-part
  owner-facing path — Intent, Source, Simulator, Your iPhone — derived only
  from the accepted six-state presentation. It is not an internal execution
  pipeline.
- **App** shows the accepted app and makes **What should change?** an explicit
  action. That action opens the one keyboard-first evolution composer; it does
  not add a second evolution path.
- **Source** shows the latest bounded file projection and opens the app's real
  owner-local Git working tree.

The normal tabs do not show Shot, Expression, Version identity, execution
identity or phase, harness, model, route, prompt, raw harness output, private
log text, acceptance internals, or exact protocol controls. Those recorded
facts remain behind the deliberate Details disclosure where ADR 0016 placed
them.

### Owner-local activity is bounded and truthful

The existing loopback activity response may add up to 200 changed source
files. During work they are computed from the current captured Git tree
against that execution's durable baseline. After work they come from the
immutable completion record. TOHSENO's private execution directory is
excluded by the same source-tree boundary, and pre-existing owner work is part
of the baseline rather than attributed to the request.

The Build log renders at most the existing 200 semantic journal reports. It
does not render raw harness output or expose the internal phase carried beside
each report. Additive file fields remain optional in the native decoder so a
new app can safely adopt a locally installed helper from immediately before
this decision.

### The iPhone is a permanent stage

The selected-app workspace always keeps an iPhone stage visible. Before a
verified capture exists it shows the rotating TOHSENO mark and the current
human state. Once a verified app exists it shows the latest Simulator capture
inside an iPhone frame and explicitly says that the capture is not
interactive.

The adjacent cable card is permanent and state-aware. It asks the owner to
connect and unlock the iPhone before the build needs it, explains automatic
installation when the app is ready, and offers **Open on iPhone** only after
installation. There is still no Install, Resume, or Continue button while the
factory waits for a device; reconnecting the cable resumes the existing
durable operation automatically.

## Consequences

The selected-app surface now explains present work before asking for future
work. The keyboard remains the primary interface inside the focused creation
or evolution composer, while the rest of the window becomes calm,
owner-readable state.

This is a native projection over the existing application service, not a
second factory, restored Studio dashboard, protocol inspector, source editor,
or interactive Simulator host. It changes no public protocol encoding,
schema, frozen vector, identity, signature, lineage, acceptance rule,
contract, Registry authority, billing activation, signing, notarization,
upload, installer pin, or public release state.
