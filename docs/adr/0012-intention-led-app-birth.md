# ADR 0012: Interpret human intention before app birth

Status: accepted

Date: 2026-08-05

## Context

The Apple factory could accept protocol-conformant software without showing
that the software fulfilled the human intention. Initial creation constructed a
generic deterministic Genome before any selected intelligence read the thick
intention. Its generic expression organs—installation identity, persistence,
native navigation, and version feedback—mixed protocol infrastructure with the
product embodiment. The harness could then build a smaller product, call
`tohseno evolve`, and let one conformance boolean stand in for every organ and
for product completion.

The Anky dogfood Shot exposed both a release-identity failure and this deeper
ordering failure. The accepted record identifies factory source commit
`ba94806fd64ad87db7711a6db36d2d397a3a105d`, factory version `0.7.0`, and
accepted Genome digest
`0x1e45352176ec73ebddb96bdb757e863e5e59cbad615f52fca94e76db05d1c832`.
Its embedded `genome/LAWS.md` hashes to
`b73e72884382358aa2bd2c21fb8e27637127cd9a7c07b8abd9dd20acea163ff9`,
exactly the bytes at that source commit. Those bytes allow notifications but
say camera, microphone, and the other protected capabilities do not pass the
gates. The generated task and generic accepted Genome therefore omitted the
camera/AR/microphone product, and generated Memory recorded a screen-space Tier
C substitute. Repository HEAD already had less restrictive prose, but the
installed binary correctly used its older compiled bundle. Stale embedded
bytes were therefore involved; assuming HEAD documentation governed the run
would have been false. Even a current bundle would not have fixed the generic
pre-intelligence Genome or conformance-only acceptance.

The source scanner compounded the problem by looking for raw substrings in
files that could contain comments, string literals, documentation, asset XML,
or innocent identifiers. A method named `connect`, `eyeSocket`, or
`violetCurls` could be interpreted as network use, while a privacy-key string
inside Swift could masquerade as a real Info.plist declaration.

## Decision

The initial accepted application is a **birth**. Protocol ordinals and the
existing Evolution/Version wire terms remain unchanged for compatibility, but
ordinal 1 is accepted only as a complete production-quality expression of its
bounded intention. Internal repair passes are not Evolutions.

The engine separates these responsibilities:

- **Intention** is the exact human source of truth: prompt, references,
  requirements, actors, life change, exclusions, and constraints. Generated
  interpretation never replaces it.
- **Factory Constitution** is the static cross-Shot law for integrity,
  privacy, security, Apple platform truth, provenance, and honest claims. A
  non-universal product preference is not constitutional law. Current hard
  boundaries are classified as `protocol_integrity`, `privacy_or_security`,
  `apple_platform_requirement`, `distribution_requirement`, or
  `factory_capability_gap`.
- **App-specific Genome** is the intelligence's accepted interpretation of
  what must remain true for this intention to become itself: target users,
  promise, journeys, behavioral/experiential/aesthetic/privacy/safety
  invariants, completion, non-goals, capabilities, and valid runtime fallbacks.
- **Apple Capability Profile** is a local snapshot of the current stable SDK,
  Xcode, Simulator runtimes, sanitized connected physical-device facts, and
  sanitized last-known-device facts. Capability states distinguish support,
  permission, entitlement, hardware specificity, Simulator absence, physical
  uncertainty, current-SDK absence, and factory absence. Unknown and
  Simulator-unavailable never mean forbidden.
  A paired network device is live only when `devicectl` reports an active
  tunnel; otherwise its sanitized model/OS facts are last-known context and
  cannot satisfy physical acceptance.
- **Birth Plan** is strict `tohseno.birth-plan/1` output bound to the exact
  Intention digest and capability-profile digest. It contains specific actors,
  a stable requirement ledger with origins, Apple materials and purposes,
  journeys, embodiment, completion, non-goals, and forbidden substitutions.
- **Protocol Substrate** contains universal installation identity, signed
  continuity, and embedded provenance concerns. Its organs cannot claim a
  product requirement, journey, or capability.
- **App-specific Organs** embody the product. Each binds Genome invariants,
  requirement IDs, capabilities, state, inputs/outputs, target-user journeys,
  and independently evaluated acceptance criteria.
- **Experience Contract** is strict `tohseno.experience-contract/1`, derived
  during conception. Its target-actor scenarios name initial state,
  environment, gestures, expected states, requirement/capability coverage,
  completion, evidence classes, and physical-device requirements.
- **Materialization** produces the complete Release implementation and
  deterministic test adapters where needed. A fixture may make Simulator
  trials repeatable but cannot satisfy structural evidence for the real
  Release capability path.
- **Experience Trial** is strict `tohseno.experience-trial/1` evidence from
  Release build, tests, multi-state Simulator traversal, intelligent review,
  persistence/log inspection, and required physical-device scenarios. The
  engine validates every referenced local byte and digest, boots its own
  Simulator destination, and independently reruns the checked-in Release
  XCTest/XCUITest action. The engine test log is a receipt criterion separate
  from the harness's scenario assertions.
- **Fascia** is a deterministic membrane of Apple compatibility, permissions,
  entitlements, storage, data movement, identity, and provenance. The Birth
  Plan supplies intent-level purpose; lexical/structured source and artifact
  inspection supplies observation; the engine reconciles both into the final
  mechanical declaration. The Fascia does not author the product.
- **Conformance** proves protocol and platform facts. **Intent Fidelity** proves
  requirement-to-organ/evidence mappings and absence of forbidden
  substitutions. **Experience Verification** proves required journeys and
  required physical trials. None is an alias for another.
- **Accepted Birth** requires all three dimensions and no blocking typed
  incompleteness. A `product_gap`, a must-level verification gap, a missing
  physical trial, or a failed independent criterion leaves an unsealed
  candidate. **Evolution** remains a later intention applied to an already
  accepted living app.

`tohseno create` retains one-thread UX. It now preserves the Intention,
discovers capability context, runs the selected harness in conception mode,
validates strict output, applies the existing acceptance intent to that actual
proposal, writes an app-specific materialization task, runs target-user trials,
issues focused bounded repair passes, and asks the engine—not the harness—to
seal. Reaching the repair bound never turns failure into acceptance.

The reusable v0.7 Apple Fascia tree remains byte-for-byte frozen because its
digest is part of historical records. Its legacy definition still contains
old application-shell preferences; this decision neither rewrites those bytes
nor treats them as current conception authority. The additive profile makes
the frozen verifier's real limits explicit. In particular, extension products
and protected capability families that the current artifact boundary cannot
yet inspect honestly resolve as `unsupported_by_factory` before
materialization. The existing rejection of uninspected third-party runtime
dependencies is reported as an exact `factory_capability_gap`, not disguised
as universal product law. A successor Fascia would require its own versioned
protocol decision rather than mutation of the frozen tree.

Every conception task, materialization task, and Birth Receipt exposes engine
version, engine source commit, static Constitution bundle digest, accepted
Shot Genome digest where one exists, and Apple Capability Profile digest. The
same accepted receipt is available to local inspection. A source-built binary
therefore cannot silently govern work with unidentified compiled prose.

Swift capability analysis uses the shared lexer that removes comments and
string-literal bodies while retaining identifier boundaries and locations.
Runtime URL strings are considered only in executable-network context.
Info.plist, entitlements, Bonjour arrays, Xcode settings, and package references
are inspected structurally. Actual `NWConnection`, `URLSession`, camera,
ARKit/RealityKit camera, microphone, and speech pipelines remain fail-closed;
innocent vocabulary and XML namespaces do not create capabilities. Diagnostics
name the gate, category, file, exact token or structural fact, expected
declaration, blocking reason, and app/factory classification.

The new planning formats live under `engine/schemas/private-planning/`. They
are intentionally not added to `protocol/schemas/`: conception and detailed
experience artifacts may evolve faster than the canonical lineage contract.
Their digests are authenticated through existing signed VerificationResult
evidence references and final Version provenance. No existing field is
repurposed, and no historical byte calculation changes.

## Alternatives considered

- Expanding the old deterministic Genome with prompt keyword rules still
  fabricates understanding and becomes an implicit product whitelist.
- Letting the harness write uncontrolled prose makes deterministic validation,
  traceability, and focused repair impossible.
- Treating every planning artifact as a new canonical protocol action would
  impose premature serialization law and risk historical compatibility when
  existing authenticated artifact references already bind the needed digests.
- Accepting Simulator substitutes for hardware paths makes the factory's test
  environment define the iPhone and changes the product without authority.
- Keeping raw substring scanning and merely adding exceptions would continue
  turning product vocabulary into a gate-avoidance exercise.

## Consequences

Creation has one additional intelligence pass before materialization and more
strict local artifacts. Acceptance takes longer because a build is no longer
treated as experience evidence and the engine repeats the candidate's test
action rather than trusting a claimed pass, but failed work remains honestly
unsealed and repair stays within one birth. If no usable Simulator exists, the
state is `acceptance_pending_simulator_environment`, not a product reduction.

Simulator fixtures can prove deterministic behavior and visual states while
the real native path remains in Release. When a compatible connected iPhone is
available, a hardware-critical completion contract requires physical
build/install/launch and the relevant target-user evidence. Without that
evidence the explicit state is
`implementation_complete; acceptance_pending_physical_experience`; the engine
does not downgrade or seal.

The capability catalog is data-driven and can grow with stable SDKs without a
monolithic decision switch. A true unsupported must capability becomes a
visible factory gap before substantial materialization. Scanner precision
reduces false positives without weakening sensitive-capability, entitlement,
privacy, endpoint, or secret boundaries.

Historical lineages remain valid. Frozen commitments, accepted Evolution
directories, signatures, canonical serialization, v1/v2 fixtures, and
Evolution semantics are unchanged. Old Versions are verified under their own
recorded factory provenance; they are not retroactively required to contain a
Birth Plan or Birth Receipt.
