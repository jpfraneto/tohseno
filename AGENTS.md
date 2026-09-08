# Agent guidance

This repository has an explicit authority hierarchy. Read this before editing
anything that looks like documentation — several prose files here are governed
protocol material, not free-form docs.

This repository also has an explicit operating philosophy:

> Build the smallest correct thing, exercise the real user path, deploy from
> `main`, observe reality, and fix forward.

Do not turn ordinary product work into release ceremony.

---

## Where authority lives

1. **`protocol/`** — normative and authoritative over all prose.

   `protocol/SPECIFICATION.md`, `protocol/CONFORMANCE.md`, the schemas, and the
   test vectors define exact byte encodings and validation rules.

   If any prose file disagrees with `protocol/`, `protocol/` wins.

2. **`docs/adr/`** — accepted architectural decisions.

   ADR 0039 governs the One Shot living workshop projection across the native
   Mac and Companion products: one Mac factory, intended iPhone, Tohseno as
   workshop keeper, Companion as the paired human authority, real app objects,
   and a truthful network threshold.

   It migrates existing Apps, Registry, Updates, Profile, creation, and
   selected-app workbench capabilities into one shared scene without creating a
   second factory, restoring Studio, changing protocol/ABI/Claim/Ship/Update
   semantics, or treating visual motion as real installation or publication.

   ADR 0035 governs Claim, exactly one Ship followed by Updates, immutable
   per-Shot Claim Editions, the additive non-transferable `TohsenoClaimsV1`
   receipt, the Discover timeline, private Following/Updates, and the Companion
   circle ritual.

   Claims has a separate threshold-signed activation and remains dark until exact
   runtime, Registry, relayer, released clients, and owner-attended physical
   evidence agree.

   It changes no frozen protocol encoding or generation-0.8 ABI, and authorizes
   no price, transfer, wallet-connect, fake Claim/installation, or release-gate
   bypass.

   ADR 0036 makes the intended physical iPhone the private installation
   destination while USB and local-network reachability remain interchangeable
   observed transports.

   Current source consumes the private Companion-setup CoreDevice digest as a
   single-target bootstrap selector and never substitutes another visible phone;
   older records retain the exactly-one-reachable-device fallback.

   The full replacement/multi-target association still requires physical
   acceptance.

   ADR 0037 defines future network trust as exact-release Identity Bindings,
   Verification Reports, Release Attestations, and private social context.

   Neither decision changes Claim, frozen encodings, deployed ABIs, Apple
   security, or release gates, and no opaque safe score or inherited review is
   permitted.

   ADR 0034 governs the person-to-person native software network.

   The Mac is the factory, Companion is the human authority holding the
   non-exportable Builder DeviceKey, and the active generation-0.8 Registry plus
   signed off-chain catalog is the public witness.

   Public source requires explicit Companion approval; recipients independently
   verify and build with their own Xcode signing identity.

   It changes no frozen protocol encoding or deployed ABI, and permits no fake
   receipt, physical evidence, generic relayer, or release-gate bypass.

   ADR 0024 governs the app-local `.tohseno/` Git boundary: the directory is
   integral and never blanket-ignored, while exact private and transient paths
   remain ignored.

   ADR 0025 governs the native macOS product transition: `Tohseno.app` is the
   primary surface over the one existing Rust factory; Companion setup,
   successful-day qualification, subscription gating of local/BYO execution,
   and npm/browser first run are no longer consumer requirements.

   Managed inference instead uses an append-only balance, a constrained TOHSENO
   proxy, and Bankr behind explicit consent and a hard reservation.

   ADR 0025 changes no public protocol encoding and authorizes no external
   billing, Bankr, signing, notarization, or release activation.

   ADR 0026 governs the keyboard-first native creation surface, the optional
   truthful local Registry/Builder track-record destination, and the fail-closed
   one-line native installer.

   Plain Return sends from a focused intention composer while Shift-Return
   inserts a line.

   The Registry may show verified local Shot heads and local/test-only identity
   status; ADR 0034 now separately authorizes the implemented public Registry
   path only when its signed manifest and fresh chain evidence agree.

   `/install` and `/download` remain unavailable until the exact immutable
   notarized DMG URL and SHA-256 are activated.

   ADR 0027 governs the native selected-app workspace: Build/App/Source tabs,
   bounded owner-local semantic activity and changed-file projection, an honest
   non-interactive Simulator capture, a permanent cable handoff card, and a
   button-gated keyboard-first evolution composer.

   It does not restore the deleted Studio dashboard or expose internal phases,
   identities, raw harness output, prompts, or protocol controls on the normal
   path.

   ADR 0028 governs the Finder-first native handoff and first-open welcome:

   the one-liner asks only for Enter or Escape, visibly downloads and verifies
   the pinned DMG, reveals its exact Downloads location, and leaves the familiar
   drag into Applications to Finder.

   It also replaces the empty first-run placeholder with the small TAKE A SHOT
   invitation while keeping Create an App as the primary-path action.

   ADR 0029 supersedes only that passive welcome composition: before an empty
   factory appears, TAKE A SHOT is the real existing creation composer, accepts
   up to eight picked or dropped PNG/JPEG references, and offers an explicit
   persisted Skip beside Create App.

   It adds no second factory or command path.

   ADR 0030 supersedes the one-line command only as the website's normal
   consumer door: the landing page detects the visitor's system and links
   directly to the fail-closed macOS DMG route.

   The immutable HTTPS artifact, exact SHA-256, Developer ID, notarization,
   Gatekeeper, Finder handoff, and publication gates remain mandatory; no
   release is activated by that decision.

   ADR 0031 permits an explicitly owner-authorized, visibly labeled public
   release-candidate DMG channel so clean-Mac acceptance can exercise the real
   website-to-Finder path.

   The exact candidate must already be signed, notarized, stapled, digest-pinned,
   and origin-verified; stable promotion remains closed until acceptance and the
   remaining release gates pass.

   ADR 0022 governs optional app naming: a supplied name is authoritative; when
   omitted, local machinery reserves a technical slug and the one existing
   implementation model chooses the user-facing product name from the intent.

   ADRs 0021 and 0020 still govern their retained installer, cable, entitlement,
   and receipt compatibility mechanisms, but ADR 0025 supersedes their npm-first,
   Companion-first, qualification, and subscription-gate product decisions.

   ADR 0019 governs the bounded intent-to-usable-app transition: one
   implementation harness, at most one code/build repair, one shared wall-clock
   budget, and one private State Transition Receipt.

   ADR 0017 governs how a birth runs: the engine composes and accepts the Genome
   itself and the one harness invocation reads the exact intention, so there is
   no Conception phase and no `.tohseno/CONCEPTION.md`.

   Do not reintroduce a planning round trip in front of the build.

   ADR 0016's App → Intent → App on your iPhone abstraction and
   six-state/deletion constraints remain current; ADR 0025 makes the native Mac
   app its primary projection while Studio and Companion are optional support
   projections.

   ADR 0015 governs the persistent local factory and private companion boundary
   beneath it while preserving ADR 0014's recording format.

   ADR 0006 governs the successor (0.8) contract generation and public-witness
   design.

   Generation 0.8.0 is deployed and is the current client-trusted active
   generation under `release/contract-activations/`.

   ADR 0034 connects it to secure Builder bootstrap, constrained registry RPC,
   receipts, source hosting, catalog discovery, and download without changing
   that generation.

   ADR 0016 is a deletion decision as much as an addition: the Studio dashboard,
   its execution-pipeline renderer, its Feedback/Marketing forms, and its
   exact-Version binding controls are gone deliberately.

   `studio/tests/` asserts they stay gone.

   Do not rebuild them.

3. **`MASTER_PROMPT.md`** — the *historical* constitutional center of the frozen
   v0.7 lineage.

   It says so itself: it is superseded implementation input and must not be used
   as current protocol or deployment authority.

4. **`genome/LAWS.md`** — historical agent-facing planning law retained for
   verification and compatibility with app-factory records.

   Its statements must match the engine code in
   `engine/src/protocol_lifecycle.rs`; do not edit it as if it were ordinary
   prose.

5. **`docs/STATE.md`** — plain-prose snapshot of what currently ships, what is
   inactive, and what is deferred.

The web-to-local handoff in ADR 0011 is transport, not protocol law.

Keep its terms distinct:

- Browser Draft
- Pending Relay Intention
- Local Pending Intention
- Shot
- Evolution

A relay record is never a Shot.

Production handoff must stay fail-closed until the matching immutable
claim-capable release is published and the public installer pin is verified.

---

# Owner operating model

The owner is currently optimizing Tohseno for **learning from real use**, not for
release-process sophistication.

Unless the owner explicitly requests otherwise, use the following operating
model.

## Work directly on `main`

- Work directly on `main`.
- Do not create branches by default.
- Do not create staging environments by default.
- Do not introduce preview environments, release trains, promotion ladders, or
  deployment ceremony unless they solve a demonstrated problem.
- Preserve meaningful working states with ordinary commits.
- Push normal commits when appropriate.
- Never force-push published history.
- Never discard unexplained owner work.
- Never reset a dirty working tree merely to obtain cleanliness.

The desired loop is:

```text
understand
  -> change
  -> smallest relevant verification
  -> commit
  -> deploy
  -> exercise the real user path
  -> observe
  -> fix forward
```

The existence of a more elaborate possible process is not a reason to introduce
it.

---

# Reality is the primary acceptance surface

Whenever practical, prefer exercising the intended product path over creating
additional simulated confidence around it.

For Tohseno, examples of especially valuable evidence include:

- a real Builder using their Companion DeviceKey;
- a real `tohseno deploy`;
- a real public Shot;
- a real `tohseno.com/<slug>` route;
- a second human opening that route;
- that human Claiming the exact release;
- their own Mac retrieving and verifying the source;
- their own Xcode/Apple identity signing the result;
- their intended physical iPhone receiving the app;
- a real Update subsequently traversing the same path.

A simulator, fixture, unit test, mocked receipt, locally edited Registry record,
or prose report must never be represented as equivalent to those facts.

However, the absence of every possible automated test is also not a reason to
delay trying the real path when the relevant invariants are already protected.

When choosing between:

```text
A. spending another hour proving that the product should work

B. safely exercising the actual user flow and seeing what happens
```

prefer **B**, unless A protects a concrete authority, security, data-loss, signing,
protocol, or irreversible-production boundary.

---

# Scope before ceremony

Before beginning work, identify the smallest surface that actually needs to
change.

Do not expand the task merely because adjacent systems exist.

In particular, do not opportunistically add:

- new trust subsystems;
- new identity systems;
- new reputation models;
- new deployment environments;
- new CI layers;
- new release abstractions;
- new protocol generations;
- generalized machinery for a one-time operational problem.

If a manual operator step is acceptable for the current real-world experiment,
prefer documenting the manual step over spending hours automating it prematurely.

If a rough edge does not prevent the real user path, record it and continue.

Polish comes after evidence.

---

# Verification philosophy

Verification is required.

**Exhaustive verification is not required for every change.**

The repository contains many valuable test suites. They are a toolbox, not one
mandatory sequential ritual that must run before every useful action.

The agent must choose verification according to the surface changed and the risk
introduced.

## Default rule

For ordinary product work:

1. Identify the changed surface.
2. Run the smallest relevant formatter, compiler, test, typecheck, or integration
   check that can reasonably catch regressions in that surface.
3. If that passes, exercise the real path when practical.
4. If the real path works, continue.
5. Fix forward when real use exposes a defect.

Do not automatically run the full repository matrix because `AGENTS.md` lists
the available commands.

Do not rerun expensive evidence that already passed for the **same source state**
unless:

- the relevant source changed afterward;
- the environment changed in a way that could invalidate the result;
- there is evidence of flakiness or corruption;
- an authoritative release requirement explicitly requires a fresh run; or
- the owner explicitly requests a complete verification pass.

## When exhaustive verification is warranted

A broader or complete matrix is appropriate when the change touches things such
as:

- frozen protocol encodings;
- protocol validation rules;
- deployed contract semantics;
- cryptographic authority;
- Builder DeviceKey handling;
- Apple identity/signing boundaries;
- source-integrity verification;
- production write authorization;
- irreversible migration logic;
- install-target correctness where another device could receive software;
- release artifacts being promoted under an authoritative gate that explicitly
  requires the matrix.

Even then, run the tests because they protect a named invariant — not merely
because they exist.

---

# Long-running commands and remote jobs

The owner's interactive time is valuable.

Do not silently disappear into a long-running process.

Before deliberately starting something expected to take more than approximately
five minutes, state in the working transcript:

- what is being run;
- what invariant or goal it protects;
- whether it blocks the next real user action;
- the expected order of magnitude of its duration.

If the command is not necessary to unblock the owner's current goal, do not run
it yet.

## Do not babysit remote CI

Do not spend an interactive Codex session repeatedly polling GitHub Actions or
another remote job for tens of minutes.

If a remote job has been successfully started and there is no productive local
work that depends on intermediate output:

1. report the job/run identifier;
2. state what it is doing;
3. state whether the real product path is blocked by it;
4. return control to the owner.

One status check to determine whether a remote job started correctly is
reasonable.

Repeated status polling is not productive work.

If the job later fails, inspect the failing step rather than automatically
restarting the entire workflow.

## Timeouts are evidence, not commands

A timeout means:

> this process exceeded its allowed duration.

It does **not** automatically mean:

> rerun every preceding test.

When a long workflow times out:

1. identify the exact incomplete/failing step;
2. determine whether earlier outputs/artifacts remain valid;
3. determine whether the failure represents a product defect or process defect;
4. rerun only the smallest necessary portion;
5. continue from preserved valid work whenever possible.

Do not destroy valid artifacts or checkpoints simply to obtain a cosmetically
clean rerun.

---

# Fix forward

The default recovery strategy during real-product development is **fix forward**.

When a real flow breaks:

1. reproduce or inspect the concrete failure;
2. identify the smallest actual blocker;
3. change the responsible surface;
4. run the smallest relevant verification;
5. commit the fix;
6. deploy it;
7. retry the same real flow.

Do not respond to a local defect by inventing a generalized subsystem unless the
defect demonstrates that the subsystem is actually necessary.

Do not restart a multi-hour process when a retained artifact or successful prior
step can safely be reused.

---

# Production and deployment

There is no staging-by-default product workflow.

When the owner asks to see whether something works, the normal destination is
the real Tohseno system.

Deployment directly from reviewed `main` is normal.

This does **not** authorize bypassing architectural or security invariants.

In particular, preserve real requirements around:

- exact release identity;
- immutable source/release digests;
- Builder DeviceKey authority;
- recipient-local Apple signing;
- Developer ID signing where required;
- notarization where required;
- Gatekeeper where required;
- exact published installer pins;
- constrained Registry/Claims write authority;
- Claim semantics;
- Ship-versus-Update semantics;
- intended-device installation;
- deployed contract compatibility;
- fail-closed behavior where authority is ambiguous.

The principle is:

> remove ceremony, not integrity.

If an existing gate protects one of these concrete invariants, satisfy it.

If a process step exists only because a generalized release workflow happens to
run it, determine whether it is actually necessary for the current goal before
blocking on it.

---

# Owner-attended boundaries

Some actions require the owner or another human because they involve real-world
authority.

Examples include:

- approving with the physical Companion;
- using the real Secure Enclave Builder DeviceKey;
- entering or authorizing production secrets;
- opening constrained write capability;
- authorizing irreversible production mutations;
- Apple Account interaction;
- Trust / Developer Mode interaction;
- physically pairing a device;
- confirming the intended iPhone;
- performing a real Claim ritual.

When one of these is reached:

1. stop;
2. explain the exact action required in plain language;
3. give the smallest command or UI instruction necessary;
4. wait for the resulting evidence.

Do not replace human authority with test keys or fabricated evidence.

Do not continue designing unrelated systems while waiting for the owner.

---

# Product-path priority

For the current person-to-person network, the highest-value path is:

```text
Builder
  -> tohseno init
  -> tohseno deploy
  -> Companion approval
  -> one Ship
  -> public Registry
  -> tohseno.com/<slug>
  -> recipient
  -> exact Claim
  -> recipient Mac
  -> exact source verification
  -> local Xcode build
  -> recipient Apple signing
  -> intended iPhone
  -> installed app
```

When this path has not yet succeeded in external reality, prefer work that makes
it succeed over work that adds secondary abstractions.

Do not allow:

- identity enrichment;
- social graphs;
- reputation;
- economics;
- cosmetic verification;
- dashboards;
- release-process sophistication;
- extensive generalized automation

to displace the next concrete blocker in this path.

---

# Available verification toolbox

Use these commands when relevant to the surface being changed.

They are **not** one unconditional checklist.

## Rust

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
```

For a narrow Rust change, prefer a narrower package/test invocation when it gives
adequate coverage.

## Apple identity

```sh
swift build --package-path apple-identity
swift test --package-path apple-identity
```

Use when Apple identity, Keychain, Secure Enclave, signing identity, or related
boundaries change.

## Fascia

```sh
swift test --package-path fascia/apple
```

Use when that package or its contracts change.

## CompanionKit

```sh
swift test --package-path sdk/apple/TohsenoCompanionKit
```

Use when CompanionKit behavior or contracts change.

## Companion

```sh
swift test --package-path companion/apple/TohsenoCompanion
```

Use when Companion behavior changes.

## macOS app

```sh
swift test --package-path macos/Tohseno
```

Use when native Mac behavior changes.

## Contracts

```sh
forge build --root contracts
forge test --root contracts -vvv
```

Use when contracts or assumptions requiring contract verification change.

Do not redeploy contracts merely because these tests run.

## Studio deletion/static boundary

```sh
node --test studio/tests/static_assets.test.mjs
```

Use when work could affect deliberately deleted Studio surfaces or their static
boundary.

## Website

```sh
(cd website && bun run typecheck && bun test)
```

Use when website, public Registry rendering, routes, aliases, installer
projection, or related web behavior changes.

Prefer narrower relevant tests when available and sufficient.

## Lifecycle and integration suites

```sh
./scripts/test-ontology-lifecycle.sh
./scripts/test-local-companion-e2e.sh
./scripts/test-macos-service-lifecycle.sh
./scripts/test-network-e2e.sh
```

These are expensive integration evidence.

Run the specific suite when the changed surface or risk justifies it.

Do **not** automatically run all four after every source change.

Do **not** block an unrelated website, copy, UI, or isolated product change on
all lifecycle suites merely because they are listed here.

The three lifecycle scripts use isolated service, Shot, relay, LaunchAgent, and
temporary Keychain fixtures.

They do not call the developer's real LaunchAgent, change the user's Keychain
search list, or remove unrelated Keychain records.

That isolation makes the tests safer to run; it does not make them mandatory for
every task.

---

# Release artifacts

A release artifact must tell the truth about the source it contains.

Do not reuse an older candidate as evidence for newer source.

When producing a distributable macOS candidate, preserve the exact requirements
that are materially part of the product's trust and install path, including as
applicable:

- exact source commit;
- Developer ID signature;
- hardened runtime;
- notarization;
- stapling;
- Gatekeeper acceptance;
- immutable artifact digest;
- origin/download-byte agreement.

Do not add unrelated release ceremony around those requirements.

If the artifact is valid and a separate nonessential CI suite is slow or flaky,
do not automatically destroy the artifact and restart from zero.

Determine whether the failing process actually invalidates the artifact.

---

# Evidence discipline

Never claim something happened because source code appears to implement it.

Keep these categories distinct:

1. **designed**
2. **implemented**
3. **locally verified**
4. **deployed**
5. **observed on the real system**
6. **physically accepted by another human/device**

Tests can support implementation claims.

They cannot fabricate deployed or physical evidence.

A real failure is useful evidence.

Record it accurately and fix forward.

---

# Communication with the owner

Keep progress legible.

Do not produce repeated status prose whose only content is that a long command is
still running.

Prefer messages such as:

```text
Running: notarization submission
Why: required for the DMG users will actually open
Expected: several minutes
Blocks: public Mac download
```

or:

```text
GitHub integration suite is still running remotely.
It does not block the local build, so I am not going to sit here polling it.
Run: <identifier>
```

When blocked, state the concrete blocker.

When the owner must act, state the concrete action.

When something is merely nice to have, say so and continue.

Avoid turning operational uncertainty into architectural work.

---

# What not to optimize for yet

Until the core handoff works reliably between real people, do not prioritize:

- branch strategy;
- staging infrastructure;
- elaborate environment promotion;
- comprehensive release dashboards;
- generalized release orchestration;
- exhaustive test execution after every change;
- perfect alias governance;
- perfect onboarding polish;
- complete multi-device management;
- reputation systems;
- social proofs;
- token incentives;
- broad trust scoring.

These may become valuable after reality demonstrates the need.

For now:

> make the real path work, preserve the invariants that make it honest, and learn
> from what breaks.

---

# Final rule

Do not confuse rigor with ceremony.

Tohseno must remain cryptographically, operationally, and semantically honest.

But the purpose of the machinery is to let software move between people.

If the next useful fact can be learned by safely putting the actual product in
front of a real person, prefer that over another hour of generalized
verification.

**Reality first. Integrity always. Ceremony only when it earns its keep.**

No new contract-generation or deployment ceremony is active on `main`;
`scripts/deploy-candidate.sh` fails closed by design.

Do not add one unless a later authoritative decision explicitly requires it.
