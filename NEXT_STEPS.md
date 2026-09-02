# Next steps: first real app-slug handoff

Target date: 2026-09-02

Mission:

> Ship one real app to the Registry, make its reviewed
> `https://tohseno.com/app-slug` page live, and have a friend Claim and install
> that exact release on their iPhone through their own Mac and Apple identity.

This is an owner-attended release/production/physical operation. The source is
prepared for it, but the operation is not complete until every item below has
real evidence. Do not use RC6 to claim the new slug/alias UX: RC6 predates these
source changes.

## Observed starting state

The live read-only check on 2026-09-01 established this baseline:

- [`/healthz`](https://tohseno.com/healthz) reports ready;
- [`/api/distribution/v1/macos`](https://tohseno.com/api/distribution/v1/macos)
  still serves labeled candidate `v1.2.0-rc.6` with the recorded digest;
- [`/api/registry/v1/status`](https://tohseno.com/api/registry/v1/status)
  reports generation 0.8 available and the Registry relayer disabled;
- [`/api/registry/v1/timeline`](https://tohseno.com/api/registry/v1/timeline)
  is empty; and
- [`/api/registry/v1/claims/status`](https://tohseno.com/api/registry/v1/claims/status)
  reports verified activation/runtime/indexer and a funded but disabled Claims
  relayer.

Preserve this dark-write posture until the exact new release and attended
window below are ready.

## Stop conditions

Stop and leave writes dark if any of these is false:

- the complete local lifecycle matrix passes from the clean reviewed commit
  using its isolated temporary-Keychain fixtures;
- the exact new Mac candidate is built from a clean reviewed commit, Developer
  ID signed, notarized, stapled, digest-pinned, and origin-verified;
- website, Companion, Mac, CLI, Registry, Claims activation, and relayer
  coordinates all describe the same released source;
- the Builder's physical Companion announces a non-test Secure Enclave
  DeviceKey and approves the exact request;
- both constrained relayers are funded, correctly configured, and enabled only
  for the attended window;
- the alias review token is separate, high entropy, stored only in the production
  secret manager, and available to the attending operator; and
- there is exactly one intended eligible physical iPhone at each install step.

No source fixture, Simulator run, local alias pointer, edited receipt, or manual
filesystem record can substitute for these facts.

## 1. Confirm the final local gates

This source pass removed the login-Keychain dependency from software-test
verification. Apple identity tests and all three lifecycle scripts passed using
one exact temporary Keychain per run, without changing the user's Keychain
search list or touching the real LaunchAgent. Rerun them sequentially from the
clean reviewed release commit:

```sh
swift test --package-path apple-identity
./scripts/test-ontology-lifecycle.sh
./scripts/test-local-companion-e2e.sh
./scripts/test-macos-service-lifecycle.sh
```

Expected result: every suite exits zero and cleanup removes only its isolated
fixtures. A failure is a release blocker. Do not redirect a fixture to the
login Keychain, weaken DeviceKey persistence, switch production to
software-test keys, or delete unrelated Keychain items. The real Builder path
still requires the physical Companion's non-exportable Secure Enclave
DeviceKey.

Then rerun the remaining final gates from `AGENTS.md`, including:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
(cd website && bun run typecheck && bun test)
swift test --package-path companion/apple/TohsenoCompanion
swift test --package-path macos/Tohseno
./scripts/test-network-e2e.sh
```

## 2. Produce the matching release candidate

Follow [NATIVE_MACOS_DISTRIBUTION](docs/runbooks/NATIVE_MACOS_DISTRIBUTION.md)
and the current 1.2 readiness rules. Record at minimum:

- clean source commit and tree;
- full passing test matrix;
- universal Mac build and embedded CLI/Companion source agreement;
- Developer ID Team `84V63LKV45` designated requirement;
- hardened runtime, notarization submission, stapled ticket, and Gatekeeper;
- immutable HTTPS DMG URL and SHA-256;
- GitHub-origin byte round trip; and
- website byte round trip after candidate activation.

Create a new readiness entry. Do not rewrite RC6 evidence or promote stable from
source inference.

## 3. Deploy the website slice without opening writes yet

Deploy the reviewed website code with its existing durable `REGISTRY_ROOT`,
active Robinhood RPC, released Claims activation, and relayers initially off.

Create a separate high-entropy alias review token in the production secret
manager. Configure only its lowercase SHA-256 as:

```text
REGISTRY_ALIAS_REVIEW_TOKEN_SHA256=<64 lowercase hex characters>
```

Do not put the raw token in source, a shell transcript, deployment logs, the
Registry root, or the readiness JSON. Verify with relayers still dark:

```sh
curl -fsS https://tohseno.com/healthz
curl -fsS https://tohseno.com/api/distribution/v1/macos
curl -fsS https://tohseno.com/api/registry/v1/timeline
```

Also verify that `/install`, `/download/macos`, Claims reads, canonical origin,
and the exact DMG digest still match the new candidate. A random one-component
path must return 404, not a fabricated app.

## 4. Prepare the Builder and app

On the Builder Mac:

1. Install and launch the exact new candidate.
2. Open the physical Companion so its non-exportable Builder DeviceKey is
   announced to the Mac.
3. In the app's source directory, adopt the exact Xcode target if needed:

   ```sh
   tohseno init
   ```

4. Choose the final lowercase slug before Ship. It cannot change after the
   first completed publication. Run a non-mutating packaging check:

   ```sh
   tohseno deploy --dry-run --app-slug app-slug
   ```

5. Review the file count, source size/digests, build classification, scheme, and
   high-confidence-secret result. Fix the source or name if any fact is wrong.

## 5. Open the attended write window and Ship once

Enable the existing constrained Registry and Claims relayers only according to
the governed physical-acceptance runbook. Do not deploy a contract or change
generation coordinates.

For an open first edition, make the policy explicit:

```sh
tohseno deploy --app-slug app-slug --claim-edition open
```

For a limited or timed edition, use the existing CLI flags and review the exact
immutable bounds before continuing. Do not guess a policy after Ship.

On Companion, verify and approve:

- app name and release slug;
- canonical `/s/<ShotID>` route;
- source file/byte summary;
- Install and Fork permissions;
- checkpoint `#1` and Ship status;
- exact permanent Claim Edition; and
- active BuilderID/DeviceKey context.

Wait for the CLI to report canonical completion. Record the transaction hashes,
canonical blocks, release digest, source digest, ShotID, Claim Edition state,
and exactly one `shot.shipped` event. Do not proceed on a pending job or a page
that is not independently revalidated.

## 6. Request and approve `tohseno.com/app-slug`

After the release appears in Companion:

1. Open **Profile → Global alias request**.
2. Select the exact newly shipped app.
3. Leave Alias empty to use the slug signed into the release, or enter the same
   reviewed slug explicitly.
4. Tap **Sign Alias Request**.
5. Copy the selectable `0x…` review request ID shown by Companion.

The attending operator first retrieves the bounded, revalidated review view and
compares its alias, Builder, Shot, signer key, current release digest, and
checkpoint with Companion and the public page:

```sh
curl --fail-with-body \
  -H "Authorization: Bearer $alias_review_token" \
  "https://tohseno.com/api/registry/v1/alias-reviews/0xREQUEST_ID"
```

Only then approve exactly that request using the same raw secret from the
production secret manager:

```sh
curl --fail-with-body \
  -H "Authorization: Bearer $alias_review_token" \
  -H 'Content-Type: application/json' \
  --data '{"decision":"approve"}' \
  "https://tohseno.com/api/registry/v1/alias-reviews/0xREQUEST_ID"
```

Expected status is `201` for the first approval and `200` only for an identical
idempotent retry. Record the returned alias, route, ShotID, request ID, and
approval time. A `409`, mismatched Shot, changed Builder key, or unavailable app
is a stop condition—not something to repair by editing storage.

Back in the exact Builder project directory, run:

```sh
tohseno status
```

It must say `Friend route: https://tohseno.com/app-slug ✓` and **send this
exact link**. That verdict independently checks the alias, Shot, current public
release digest, canonical Shot route, and human page. `awaiting review` means
the approval is not canonical yet; `conflict` means the route names another
Shot or a different current release and must never be sent; `unavailable` means
recheck Registry health before continuing.

Verify in a clean browser and on the friend's iPhone:

```sh
curl -fsS https://tohseno.com/app-slug
curl -fsS https://tohseno.com/api/registry/v1/aliases/app-slug
```

The page must show the intended app, one exact release/checkpoint, current
Claim Edition, provenance, the honest absence of human attestations, and the
four recipient steps. Its Claim action must contain the exact release digest.

## 7. Friend's simple path

The friend should be able to follow only this explanation:

1. **On the Mac:** open `https://tohseno.com/app-slug`, download the exact
   Tohseno candidate, drag it into Applications, and open it. Full Xcode and an
   Apple Account in Xcode are required.
2. **Pair once:** install/launch Companion through Tohseno and pair the intended
   iPhone. Enable Trust and Developer Mode when Apple asks. Use a cable if this
   iOS/Xcode combination requires first pairing; otherwise use Apple's
   [Device Hub](https://developer.apple.com/documentation/xcode/pairing-your-devices-with-your-mac)
   wireless pairing.
3. **On the iPhone:** open the same app page, tap **On iPhone: open in
   Companion**, inspect the exact Builder/release, and draw the Claim circle.
4. **Let the Mac prepare it:** for the offline proof, keep the Mac unavailable
   until the canonical Claim exists. Then bring the Mac online. It must verify
   the exact release/source, build and sign with the friend's Apple identity,
   retain the artifact if the phone is absent, and install only when the one
   intended paired iPhone is reachable and unlocked.

After any required first cable pairing, remove the cable before the decisive
delivery. Confirm CoreDevice reports the target through its supported local-
network path. Keep another eligible iPhone out of this first mission to reduce
operational variables; source now refuses to substitute it when the persisted
Companion target is absent, but that selector does not yet have physical
multi-phone acceptance evidence.

## 8. Capture acceptance evidence

The acceptance record should contain, without private keys or device identifiers:

- Builder and recipient identity distinction;
- ShotID, exact release digest, checkpoint digest/sequence, source digest, and
  app slug;
- Registry and Claim Edition transactions plus canonical blocks;
- alias request and append-only approval receipts;
- exact HTML route and Claim deep-link release parameter;
- second identity's canonical Claim number and receipt route;
- proof that Claim succeeded while the recipient Mac was offline;
- proof that preparation resumed later without rerunning Builder publication;
- recipient Xcode team context without Apple credentials;
- retained artifact state while the phone was unreachable;
- local-network CoreDevice reachability after cable removal;
- exact target install plus post-install bundle inventory; and
- explicit statements that Claim is not review/install and that no Release
  Attestation exists.

Screenshots and prose may supplement, but must not replace canonical receipts,
digests, logs, and physical bundle inventory.

## 9. Close the window

Immediately disable both constrained write relayers after the attended actions.
Keep Claims/Registry reads and the approved alias available only if their exact
runtime, deployment, catalog, index, and website evidence still agree.

Update `release/V1_2_0_READINESS.json`, `docs/STATE.md`, `PROGRESS.md`, and the
new candidate evidence with only observed facts. Stable promotion remains a
separate decision after every remaining release gate passes.

## If tomorrow stops early

- **An isolated lifecycle fails:** stop before release packaging; preserve its
  bounded fixture diagnostics, fix the source or verification environment, and
  rerun the full local matrix. Never point software-test verification at the
  login Keychain.
- **Builder DeviceKey absent:** open the paired physical Companion; never use a
  software-test key for production.
- **Relayer unavailable or underfunded:** leave the request durable and writes
  dark; do not bypass it with a generic transaction sender.
- **Alias conflict:** choose a new slug only before the first Ship. After Ship,
  preserve the signed app slug and request a separately reviewed convenience
  alias without rewriting release history.
- **Friend has multiple reachable phones:** a current setup must select only
  the persisted Companion target. If the setup predates that target record,
  Tohseno stops; disconnect the other phones and retry. Any substitution or
  list-order selection is a release-blocking defect.
- **Wi-Fi tunnel absent:** recheck same-network, Device Hub pairing, Trust,
  Developer Mode, Mac wake state, and Apple diagnostics. USB is an honest
  fallback, but it does not prove the wireless acceptance item.
- **Claim or release digest differs:** stop. Refresh canonical evidence; never
  silently move the person to a newer release.

The next engineering milestone after this physical proof is to turn the one
bootstrap target into ADR 0036's versioned private association: bind it to the
paired Companion ID and each install intention, add attended replacement/reset,
and physically pass a two-visible-phone test. Verification Reports and human
attestations should follow only after this distribution path is real.
