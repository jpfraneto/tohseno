# Extraction roadmap from Anky

Anky is the first continuity reference application and an important source of contracts. It is not a directory of production code ready to move. TOHSENO should extract executable knowledge before implementation.

The full architectural study is preserved verbatim in [research/anky](research/anky/README.md). The roadmap below applies its findings to this repository without modifying or copying Anky production code.

## Findings that govern extraction

1. A `.anky` file is a private artifact, not a complete `ContinuityEvent` or `ContinuityProof`.
2. Artifact hashes currently serve too many roles and can change around reflection preparation.
3. Practice identity is cryptographically portable, but content, backup, subscription, ownership, and recovery semantics are not equivalent.
4. Parallel iOS and Android source does not prove live runtime parity; composition roots differ.
5. Reflection is local-first but raw content currently crosses backend/provider boundaries.
6. Existing request types contain method/path while some signers reconstruct fixed `POST /anky`, so authorization is not safely generic.
7. Moving shared-looking code first would freeze canonical-byte, lifecycle, identity, and recovery drift into a framework.

## Protected invariants

Extraction work must preserve:

- exact historical `.anky` bytes and readable hashes;
- stable access to existing local artifacts, reflections, and backups;
- offline action and draft recovery;
- existing Anky practice-key derivation through a named compatibility adapter;
- absence of raw writing from server logs and durable control-plane storage;
- the distinction among content integrity, event identity, request authorization, and proof;
- staggered native/server upgrade compatibility;
- explicit export and ejection;
- Anky's ritual as product policy rather than a generic default.

## Phase 1 — contract harness (current foundation)

Build language-neutral schemas and golden fixtures before moving code. The current TOHSENO harness establishes draft shapes for events, artifacts, reflections, proofs, and signed requests, with Unicode, empty-content, canonical-byte, mutation, timing, and route-signing cases.

Next, adapt equivalent fixtures into Anky's TypeScript, Swift, and Kotlin test surfaces without changing production behavior. Characterize live first-launch, action, checkpoint, seal, reflection consent, deletion, and recovery composition roots. Expected platform divergences must be named rather than normalized silently.

Exit criteria:

- the same canonical byte and signature statements have agreed cross-language results;
- old behaviors that differ are labeled with product decisions;
- no fixture contains real user writing or credentials;
- Anky production paths remain untouched until the test boundary is reviewed.

## Phase 2 — compatibility decisions and internal ports

Resolve the minimum decisions needed for internal interfaces:

- stable event ID and immutable sealed artifacts;
- relationship between old pre/post-reflection hashes and an event;
- endpoint-bound signed envelope migration;
- explicit completed/interrupted/seal outcomes;
- reflection consent and incomplete-fragment policy;
- identity import as a migration plan rather than a key overwrite;
- concrete deletion inventory.

Introduce narrow internal ports beside current code for action policy, artifact codec, event repository, identity signing/recovery, reflection policy/provider/repository, and post-commit effects. Preserve old routes and bytes behind adapters.

Exit criteria: characterization outputs stay byte-for-byte stable, dependency tests keep app/vendor concerns outside the domain, and old clients remain supported during a planned compatibility window.

## Phase 3 — sidecar event identity and local transaction boundary

Add stable event envelopes alongside unchanged `.anky` artifacts. Local commit should become the durable boundary:

```text
persist sealed artifact
→ append immutable event with independent event ID
→ clear checkpoint/reconcile
→ notify idempotent reflection, accumulation, proof, sync, and invitation subscribers
```

Subscriber failure may not invalidate the local event. Rebuildable projections are not canonical records. Crash tests must kill the app at every dual-write step and prove deterministic recovery.

Do not rewrite legacy artifact bytes, rename their hashes, or force content re-encryption as part of this phase.

## Phase 4 — versioned authorization and deliberate native alignment

Deploy server support for `SignedRequestEnvelopeV1` before switching native clients. The new statement binds protocol version, actual method, actual path, exact body hash, timestamp, nonce, signer, and signature. Cross-route, changed-body, expired, and replay cases must fail.

Address Android/iOS onboarding, preferences, fragment retention, reflection trigger, recovery, and deletion only through explicit Anky product decisions and separate reviewed releases. Source parity is not an acceptance criterion; composition-root behavior is.

## Phase 5 — a second proof application

The repository includes a Daily Observation manifest—photograph one living thing and add one sentence—to test that the schema is not renamed timed writing. It is an example contract, not yet a second implemented proof app.

The actual second proof application remains an open product decision. Whichever is selected must differ from Anky in input, completion, interruption, artifact media, and accumulation. It should work offline, commit one private local event, and avoid importing `.anky`, Anky prompts, gates, paintings, Base identity, or subscription logic by default.

Exit criteria: the shared kernel becomes simpler for both apps without app-name branches, and media/encryption/recovery behavior is tested on its real target platforms.

## Phase 6 — stabilize the native scaffold

Only after Anky and the second proof application pass the same contracts should TOHSENO publish stable native-domain/storage/identity APIs or claim a compiler. At that point:

- generate a local-first action vertical slice from a confirmed manifest;
- provide native adapters with tested lifecycle/storage guarantees;
- ship a reference reflection service only when declared;
- produce reproducible builds, store materials, deployment and rollback plans;
- exercise a complete ejection.

Proof, sync, payments, gates, blockchain, paintings, and subscriptions remain optional modules whose claims are independently tested.

## Phase 7 — harden generation and operations

Move the current agent capsule from an instructed coding workflow toward deterministic compiler stages. Every generated diff must trace to a manifest property. Add reproducibility, migration compatibility, dependency policy, source/binary provenance, and upgrade/ejection tests.

Automation still stops for paid resources, DNS, production credential changes, and store submission. The outside may approach one button; the inside remains auditable and approval-bound.

## Release gates

| Gate | Required evidence |
|---|---|
| Canonical data | Existing artifacts and fixture hashes remain readable and unchanged |
| Lifecycle | Offline action/checkpoint/commit and crash recovery pass on live shells |
| Privacy | Every disclosure/deletion location is enumerated; raw content is absent from logs |
| Identity | Recovery does not silently reassign content, subscription, or server ownership |
| Authorization | Actual route/body statements and replay negatives pass across languages |
| Genericity | A materially different app uses the contracts without Anky conditionals |
| Ejection | Source, data, identities, infrastructure, credentials, and runbooks transfer cleanly |

## Recommended next engineering step

Extend the current contract fixtures into a cross-language characterization suite against Anky's existing TypeScript, Swift, and Kotlin implementations, including live native composition-root tests. Do not move production code first.
