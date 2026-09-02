# Evolution progress

Started: 2026-09-01

This is the working log for the evolution requested in
`EVOLUTIONARY_PROMPT.md`. It records observed evidence, changes, and verification
without treating planned or source-only behavior as released behavior.

## Initial repository state

- Branch: `main` at `82241f6` (`site: add a dark token routing prototype`).
- The only pre-existing dirty path was `EVOLUTIONARY_PROMPT.md`; it is owner
  input and is being preserved.
- Protocol bytes remain governed by `protocol/`. ADR 0034 governs the current
  person-to-person source network and ADR 0035 governs Claim, exactly one Ship,
  later Updates, and the separately activated Claims contract.
- Native 1.2.0 remains a release candidate. `release/V1_2_0_READINESS.json`
  records that RC6 is signed, notarized, stapled, and publicly downloadable,
  but Registry and Claims writes remain dark outside an owner-attended window.
  No canonical physical Claim or second-person install has been proven.

## What the repository already implements

- An exact signed catalog release binds one Shot, immutable release digest,
  current public checkpoint, source archive digest, Xcode recipe, and declared
  install/fork permissions.
- First Ship atomically opens the immutable Claim Edition; subsequent public
  releases are Updates and cannot inherit or replace that edition.
- The Companion Claim link includes both ShotID and exact release digest. The
  Companion resolves that immutable release before presenting it and a
  canonical Claim durably queues preparation on the paired Mac.
- The Mac verifies public evidence, downloads exact source, builds and signs
  under the recipient's Apple environment, retains the verified artifact when
  the phone is absent, and retries installation without rerunning the coding
  harness.
- CoreDevice inventory already accepts an active paired `localNetwork` tunnel
  as reachable. USB, wired, and direct transports also work. Multiple reachable
  phones fail closed rather than selecting the first device.
- Public Registry pages exist at canonical `/s/<ShotID>` routes. Builder-local
  `/<handle>/<app-slug>`-style routing is represented as `/@handle/app-slug`.
  Before this pass, global `/<alias>` requests were signed by the Builder
  DeviceKey but stopped at `pending_policy_review`; no bounded approval
  mechanism existed and the site server did not dispatch root aliases.

## Gaps found against the requested mission

- `tohseno deploy` currently signs `app_slug: null` and cannot request a human
  app slug, so the friendly route cannot be prepared during Ship.
- A global `tohseno.com/app-slug` alias has request validation but no safe,
  auditable operator approval path.
- The Companion alias form silently chooses the first published app owned by
  the Builder, which is unsafe once a Builder has multiple apps.
- The public app page offers Claim/source links but does not explain the simple
  friend journey: Mac requirement, Tohseno installation, Companion pairing,
  Claim, recipient-local Xcode signing, and reachable-iPhone delivery.
- Universal Links cannot honestly be assumed for locally re-signed Companion
  builds. Apple requires the website association to name the exact application
  identifier prefix plus bundle identifier and a matching Associated Domains
  entitlement. Each recipient may sign under a different Apple team, so the
  current stable handoff remains the explicit `tohseno://claim/...` button.
- The runtime already detects wireless reachability, but current domain and UI
  language still calls an unreachable device `CableMissing` and tells users
  every install happens over a cable.
- Initial Companion setup already persisted a one-way digest of the exact
  CoreDevice that received Companion, but later app delivery ignored it. That
  meant multiple phones stopped safely while one different visible phone could
  be selected when the intended phone was absent.

## Implemented in this pass

- [x] Record additive architectural decisions for destination-driven Apple
  delivery and network-mediated exact-release trust.
- [x] Add a deploy-time app slug while preserving canonical ShotID identity.
  The slug is validated, signed into the catalog display, persisted after
  publication, and cannot change on a later Update.
- [x] Add a bounded, authenticated, append-only global-alias approval path.
  Approval revalidates the stored signature, original signed time window,
  current DeviceKey authority, current installable Shot, and alias ownership.
  The site server now dispatches approved root aliases such as `/your-app`.
- [x] Add an authenticated read-only alias-review projection so the operator
  can inspect the exact request, signer, current release, and status before the
  one permitted approval mutation.
- [x] Make Companion alias requests select the exact published app. Companion
  defaults to that release's signed app slug, validates the returned pending
  receipt, and exposes the request ID needed for review.
- [x] Turn the public app page into a precise, simple friend handoff with exact
  release/provenance facts and no unimplemented trust claims.
- [x] Replace cable-only delivery language where the implementation already
  supports Xcode wireless reachability; retain cable instructions only for the
  Apple-controlled bootstrap cases that still require them.
- [x] Consume the existing private Companion-setup device digest during every
  living-project and claimed-app install. The intended phone now wins across
  USB/Wi-Fi and inventory order; if it is absent, no other phone is substituted.
  Pre-association records retain the exactly-one-device fallback.
- [x] Align the signed global-alias bound with the signed catalog app-slug bound
  at 64 characters and reject oversized public route paths before filesystem
  lookup.
- [x] Make `tohseno status` show the canonical published release, stable slug,
  prospective friend route, and exact Companion alias-review action instead of
  suggesting another deploy immediately after publication.
- [x] Make the same status command verify the immutable alias pointer, the
  Registry's current public release digest and canonical Shot route, and the
  rendered root page before calling a friend route live. A mismatched Shot or
  stale/different release is labeled conflict with an explicit **do not send**
  instruction; 404 remains the normal awaiting-review state.
- [x] Add focused tests, run the relevant suites, and update current-state and
  operational documentation honestly.
- [x] Isolate software-test Apple identities and service LaunchAgents from the
  developer's login Keychain and production service label. Apple identity tests
  and all three lifecycle scripts now create, scope, and delete one exact
  temporary Keychain without changing the user's Keychain search list.
- [x] Produce `EVOLUTION_REPORT.md` and `NEXT_STEPS.md` as the final
  decompression and owner handoff.

## Deliberately not claimed

- ADR 0036's full versioned, multi-target, Companion-ID-bound association and
  attended replacement/reset ceremony are not implemented. Current source has
  one persisted bootstrap target derived from the physical Companion install;
  older records still require exactly one reachable eligible iPhone.
- Verification Reports, human Release Attestations, Identity Bindings,
  Farcaster context, GitHub proof, and Base/economic context are designed in
  ADR 0037 but not implemented or rendered as if present.
- No production service, relayer, alias, Claim, notarized client, or physical
  installation was activated by this source change.
- The actual first Ship and friend's physical Claim/install remain an
  owner-attended operation requiring the paired iPhones, enabled constrained
  relayers, production configuration, and new released client bytes.

## Verification log

- `cd website && bun run typecheck` — passed before edits.
- Apple documentation checked on 2026-09-01: Universal Links require a matching
  Associated Domains entitlement and AASA application identifier; Xcode's
  current Device Hub guidance supports paired Wi-Fi delivery and documents a
  cable fallback for iOS versions that cannot perform first wireless pairing.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
  — passed.
- `cargo test --locked --workspace --all-targets --all-features` — passed on
  the final source state; the CLI ran 137 tests, including exact current-release
  agreement and stale-Update refusal for the friend route.
- `cd website && bun run typecheck && bun test` — passed, 134 tests. The signed
  alias integration now also proves the public pointer and the exact
  Shot-plus-release Companion Claim URI rendered by the approved root page.
- `swift test --package-path companion/apple/TohsenoCompanion` — passed, 32
  tests.
- `swift test --package-path macos/Tohseno` — passed, 31 tests, including the
  final command-copy adjustment.
- `swift test --package-path fascia/apple` — passed, 9 tests.
- `swift test --package-path sdk/apple/TohsenoCompanionKit` — passed, 24 tests,
  including the catalog/global-alias bound agreement.
- `forge build --root contracts && forge test --root contracts -vvv` — passed,
  100 tests; existing non-failing Forge lint notes remain.
- `node --test studio/tests/static_assets.test.mjs` — passed, 21 tests.
- `./scripts/test-network-e2e.sh` — passed, including exact Claim/offline queue,
  Registry, signed alias/root-page exact Claim handoff, Solidity, Swift,
  Simulator-build, and site slices.
- Live read-only production check on 2026-09-01: health is ready; the published
  Mac channel is still `v1.2.0-rc.6` at its recorded SHA-256; generation-0.8
  Registry reads are ready with an empty timeline and its relayer disabled;
  Claims activation/code/indexer agree and its funded relayer is disabled.
  This confirms the safe starting state, not readiness of the new source.
- In-app browser discovery returned no available browser surface, so no visual,
  Messages/Farcaster/X/Safari, or real custom-scheme interaction evidence is
  claimed for the new root page. Responsive HTML/routes remain test-covered;
  visual and physical deep-link acceptance stays in the owner runbook.
- `swift build --package-path apple-identity && swift test --package-path
  apple-identity` — passed, 8 tests. The suite uses a scoped temporary file
  Keychain and never needs the locked login Keychain; it also proves configured
  path scoping, symlink refusal, and Secure Enclave refusal in verification
  mode.
- `./scripts/test-ontology-lifecycle.sh` — passed.
- `./scripts/test-local-companion-e2e.sh` — passed, including the real local
  Companion Relay, Workspace Service, and encrypted simulator exercise.
- `./scripts/test-macos-service-lifecycle.sh` — passed, including isolated
  LaunchAgent install, health, restart, release-pointer switch, and uninstall.
  These lifecycle fixtures use unique verification service labels and delete
  only their exact temporary Keychains.
- Final authority/cleanup audit — `git diff --check` passed, `protocol/` has no
  diff, shell/Python fixtures parse, no verification Keychain remained under
  the temporary root, no fixture service process remained, and the user
  Keychain search list still contained only the login Keychain.

## Completion audit

| Objective requirement | Authoritative evidence | State |
| --- | --- | --- |
| Read and evolve toward `EVOLUTIONARY_PROMPT.md` | ADRs 0036/0037; source changes across CLI, Mac delivery, Companion, Registry, and website; passing regression matrix | Implemented source slice |
| Keep a working progress record | This file records starting state, discrepancies, decisions, changes, verification, and blockers | Complete |
| Explain changes and evolved state | `EVOLUTION_REPORT.md` distinguishes source, designed behavior, released behavior, and physical evidence | Complete |
| Give the owner exact next steps | `NEXT_STEPS.md` starts from live production state and carries the governed release, Ship, alias, friend, evidence, and shutdown sequence | Complete |
| Deploy an app with a stable human slug | CLI/catalog/Companion/Registry tests prove preparation, stable signing, exact selection, review inspection, and approval behavior | Source-ready; no real Ship |
| Serve `tohseno.com/app-slug` with simple instructions | Root routing and page tests prove exact release, Mac download, platform-labeled Companion action, four steps, and provenance language | Source-ready; not deployed or visually accepted |
| Friend Claims and installs on their iPhone | Existing offline Claim/network integration plus the new private intended-phone selector prove bounded software behavior | Not physically executed |
| Release truth | Live endpoints still expose RC6 with Registry and Claims relayers disabled and no public timeline event | New release required |

The objective is therefore not complete in external reality. Remaining work is
not another source inference: produce and release the exact new signed
candidate, deploy the matching website configuration, and perform the
owner-attended second-person Claim plus physical install recorded in
`NEXT_STEPS.md`.
