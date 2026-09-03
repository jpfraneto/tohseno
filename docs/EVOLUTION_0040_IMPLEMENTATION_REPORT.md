# Evolution 0040 implementation report

Status: RC10 GitHub prerelease shipped; not website-activated or physically accepted.

## Baseline and scope

- Baseline HEAD: `898353b` (`docs: record RC9 production evidence`).
- Work stayed on `main` and preserved the owner's pre-existing changes to
  `AGENTS.md`, `EVOLUTIONARY_PROMPT.md`, `EVOLUTION_REPORT.md`, and
  `tohseno-new-landing/`.
- No frozen protocol encoding, deployed ABI, Claim, Ship, Update, Registry,
  release activation, or production state changed.
- Evolution 0040 could not use ADR number 0040 because that number already
  governs public Shot media. The accepted runtime decision is therefore
  [ADR 0041](adr/0041-workshop-runtime.md).

## What changed

- Added `sdk/apple/TohsenoWorkshopKit`, containing typed devices,
  capabilities, optional Shot surface declarations, deterministic resolution,
  authenticated Session messages, encryption, replay protection, Bonjour
  transport, reconnect state, and the small `TohsenoWorkshop.current` API.
- Added a native-token-only Workshop host authorization endpoint to the one
  existing Rust workspace service. The existing workspace and paired Companion
  records remain the source of identity and revocation truth.
- Connected the native Mac and Companion products to the shared runtime. Both
  project real connection state, peer state, last-event time, and pulse
  round-trip time; Companion also produces the physical haptic when a real
  pulse arrives.
- Added the Workshop Session strip to the living Mac scene and a subordinate
  Workshop Pulse chapter to Companion. Unavailable, discovering,
  authenticating, connected, reconnecting, and rejected states have direct
  copy and do not overwrite durable pairing/authority state.
- Removed unfinished managed-inference credit, consent, estimate, and balance
  controls from the primary creation/evolution path. Intelligence readiness is
  now derived only from installed and authenticated local/BYO providers;
  `Tohseno Intelligence — Coming soon` is informational.
- Added compact optional-Workshop guidance to new-birth and evolution harness
  input. An absent declaration remains a focused Shot and triggers no network,
  permission, or project-topology rewrite.
- Updated Mac/Companion package manifests and local-network usage metadata,
  native packaging scripts, the installer fixture, architecture/state docs,
  README, and build metadata (`10006`, local RC10 default naming).

## Runtime architecture actually implemented

The durable plane remains unchanged: Companion DeviceKey authorization,
pairing, revocation, commands, Claim, Ship, Update, and Registry evidence use
their existing paths.

The new Session plane is local and ephemeral:

1. The Mac app asks its authenticated local Rust service for a host credential
   using a fresh 32-byte challenge and Session ID.
2. Rust returns a two-minute workspace-signed credential and a peer only when
   exactly one non-revoked paired Companion is unambiguous. Otherwise the Mac
   remains visibly unavailable and advertises nothing.
3. The Mac advertises `_tohseno-ws._tcp` with no device identifiers in Bonjour
   metadata. Companion discovers it with `Network.framework`.
4. Companion verifies the credential against its persisted workspace binding,
   then signs a proof over the credential digest, fresh client nonce, exact
   Session, Companion device ID, and revocation epoch with its existing
   DeviceKey identity.
5. Mac verifies that proof against the exact active paired record. Missing
   pairing and stale/revoked epochs are distinct rejection states.
6. Both sides derive the same 32-byte Session key from the existing X25519
   pairing agreement, host challenge, Session ID, workspace ID, Companion ID,
   and revocation epoch using HKDF-SHA256. A Rust/Swift interoperability vector
   fixes the byte contract.
7. Direction-specific HKDF keys protect canonical versioned envelopes with
   ChaChaPoly. Monotonic sequence numbers and Session binding reject replay and
   cross-Session traffic. Payloads are bounded to 1 MiB.

The Shot-facing `WorkshopSession.send` accepts typed app events only. Runtime
`workshop.*` messages are reserved, and authority-shaped namespaces such as
Claim, Ship, Update, install, payment, publication, Registry, and revocation
are rejected. Session loss does not mutate durable authority state.

## Device and capability model

`WorkshopDevice` keeps stable identity, display name, platform, connection,
hardware availability, permission, reachability, and Session authorization as
separate facts. Current built-ins cover compute, display, touch, camera,
microphone, motion, haptics, and intelligence.

An optional `tohseno.workshop-shot/1` declaration can describe required and
preferred capabilities per surface. The resolver fails closed when a required
real capability is absent. With no declaration, it returns the ordinary
focused mode and remains runnable without a Workshop Session.

## Workshop Pulse proof

The pulse buttons use the encrypted local Session transport itself:

- Mac to iPhone sends `workshop.pulse`; Companion replies on the same Session
  and invokes haptics when the real event arrives.
- iPhone to Mac follows the reverse direction and reports measured round-trip
  time.
- Both products show peer, last event, interruption, retry, and rejection
  truth. No Registry or durable authority command is used as the realtime bus.

This behavior is implemented and unit-tested. It has not been claimed as
physically observed in this source session.

## Verification

Passing evidence:

- `swift test` in `sdk/apple/TohsenoWorkshopKit`: 9 tests.
- `swift test --package-path sdk/apple/TohsenoCompanionKit`: 25 tests.
- `swift test --package-path macos/Tohseno`: 40 tests.
- `swift test --package-path companion/apple/TohsenoCompanion`: 41 tests.
- `cargo test --locked -p tohseno`: 143 tests.
- The initial full `tohseno-engine` run passed 257 tests and exposed one
  generated-task length regression. After compacting the Workshop guidance,
  `cargo test --locked -p tohseno-engine genome::tests` passed both affected
  tests.
- `cargo clippy --locked -p tohseno -p tohseno-engine --all-targets
  --all-features -- -D warnings` passed.
- `cargo fmt --all -- --check` passed.
- `./scripts/test-installer.sh` passed.
- An unsigned arm64 iOS Simulator build of the actual Companion Xcode project
  succeeded after the final SDK change.
- An unsigned universal Mac app was assembled at
  `dist/rc10-local/Tohseno.app`; `Packaging/verify-app.sh` passed and verified
  the embedded WorkshopKit and Companion source.
- Both changed property lists passed `plutil -lint`; changed shell scripts
  passed `sh -n`; the implementation diff passed `git diff --check`.
- The deterministic Mac living-workshop render passed at the shipping window
  size and was visually inspected for clipping and state legibility.

These checks established implementation and local build evidence. The later
owner-authorized GitHub release evidence below establishes the signed and
notarized artifact; neither category establishes physical-device behavior.

## Owner-authorized GitHub release

- Exact clean source commit: `a87bed012902dd11f78ea3922fa6fed25ed98dac`.
- Universal app: Developer ID signed with hardened runtime, stapled, and
  Gatekeeper accepted as `Notarized Developer ID`.
- Apple notary submission: `7777bf52-9b8f-4514-8c74-3d15998eea81`, accepted.
- Public prerelease: `v1.2.0-rc.10` at
  `https://github.com/jpfraneto/tohseno/releases/tag/v1.2.0-rc.10`.
- DMG: 53,585,271 bytes with SHA-256
  `5f111f3a2d6eb96ae69c034fc3fa9766536a84d80808c4506303be39b9e43686`.
- A fresh download from the public GitHub asset URL matched the retained DMG
  byte-for-byte and its mounted app passed signature, notarization, bundle,
  architecture, and Gatekeeper verification.

The initial GitHub publication did not change the production website pin. The
owner subsequently authorized that upload after observing the RC9 download,
and production deployment `9c650c3c-1858-47ea-9d69-6ec710d88be6` switched the
visibly labeled release-candidate route to RC10. Its API and redirect expose the
exact URL and digest, and a complete download through the website matched the
53,585,271-byte artifact and SHA-256. This still does not claim a clean-Mac
launch, intended-iPhone installation, physical haptic, or live two-device
Workshop Session. Exact machine-readable evidence is in
`release/V1_2_0_RC10_GITHUB_EVIDENCE.json` and
`release/V1_2_0_RC10_WEBSITE_EVIDENCE.json`.

The automatic CI run for the tagged commit later failed in two known places:
the candidate CLI smoke still expected version `1.2.0` although this source's
CLI reports `1.2.1`, and the GitHub macOS toolchain rejected a `Bool?` switch as
non-exhaustive while building Companion. The Rust tests, Clippy, formatting,
contract jobs, local Xcode 26.3 Companion tests, and local Simulator build had
passed. The remote run was inspected once and not restarted. Its result does
not undo the separate exact-byte Apple release checks, but it leaves
older-toolchain Companion compatibility outside the RC10 evidence.

## Deliberately not implemented or claimed

- No standalone Shot is silently enrolled into the Session. The small SDK
  `join()` fails unless a host product or future explicit grant installs a real
  authenticated Session.
- No general multi-target association, remote relay, background wake system,
  durable event log, managed Tohseno inference provider, credit purchase, or
  protocol/ABI change was added.
- No RC10 stable promotion or physical acceptance was performed. RC10 is the
  truthful website and GitHub prerelease candidate.
- No physical iPhone, haptic, local-network reconnection, intended-device
  selection, or revocation behavior was represented as observed.

## Minimal owner-attended physical path

1. Build this source's Companion target onto the intended paired iPhone with
   the owner's Apple identity, and run this source's Mac app against the same
   existing factory.
2. Complete ordinary Companion pairing if that exact phone is not already the
   single active pair, then place both devices on the same local network.
3. Confirm both live strips move through discovery/authentication to Connected
   while durable Companion authority still reads independently as ready.
4. Send Mac to iPhone Pulse and physically observe the haptic plus Mac RTT;
   send iPhone to Mac Pulse and observe the Mac event/RTT.
5. Temporarily interrupt local reachability, verify Reconnecting is visible,
   restore it, and confirm the Session returns without fabricating a new durable
   pairing or command receipt.

Revocation should be exercised only if the owner intends to mutate that real
pairing; the expected result is immediate Session rejection/unavailability, not
a fallback to another visible phone.
