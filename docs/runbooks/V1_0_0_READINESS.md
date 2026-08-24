# TOHSENO 1.0.0 readiness and owner actions

Repository source targets native 1.0.0 and npm bootstrap 1.0.0. It does not
authorize or claim a Git tag, GitHub release, release manifest, npm publication,
billing, relay/APNs, DNS, public installer repin, or contract action.

## Automated source gates

Run the commands in `AGENTS.md`, plus:

```sh
./scripts/test-1.0.0-golden-path.sh
(cd packages/cli && npm pack --dry-run)
```

The golden path covers fresh private entitlement state, genesis gating,
five injected successful dates, qualification lock, asymmetric test receipt,
Pro unlock, unqualified expiry, Studio replacement surfaces, npm isolation,
and Companion entitlement UI. It uses no real phone, Keychain, LaunchAgent,
global npm prefix, relay, APNs, or billing credentials.

## Physical iPhone smoke path

From a clean source checkout with Xcode installed, use isolated development
state and run:

```sh
TOHSENO_INSTALL_ROOT="$(mktemp -d)/install" \
TOHSENO_LAUNCH_AGENTS_DIR="$(mktemp -d)/launch-agents" \
TOHSENO_DEVELOPMENT_SERVICE=1 \
TOHSENO_COMPANION_RELAY_ORIGIN=https://companion.tohseno.com \
cargo run --locked -p tohseno --
```

Then follow exactly the one instruction Studio presents: connect the cable,
Trust, enable Developer Mode, add the intended Apple Account in Xcode, and
install Companion. Verify twelve recovery words appear only on the iPhone,
pairing completes after confirmation, and the first full-product trial screen
appears. Inspect `tohseno doctor` without recording full device identifiers.
This command builds/signs/installs/launches on the selected physical phone; do
not run it in automated CI. It completes pairing only when the separately
authorized official relay is healthy; otherwise the operation fails closed
after local installation and must not be recorded as a successful smoke.

Free Personal Teams remain supported, including their three-app and roughly
seven-day provisioning constraints. Paid teams enable Apple's longer-lived and
distribution capabilities; TOHSENO Pro never purchases or guarantees Apple
membership.

## External owner gates

- Build native archives from one clean captured commit with `scripts/release.sh`.
- Create and independently verify `native-v1.json` at the fixed official URL.
- Complete `BILLING_1_0_0.md` only if billing is being activated.
- Complete `NPM_1_0_0.md` manually; do not change a live dist-tag automatically.
- Do not activate the production Companion Relay/APNs until its separate
  claim-capable immutable release and installer pin are independently verified.
- Do not deploy contracts or add a deployment ceremony on `main`.

Rollback preserves `~/.tohseno/service`, apps, identities, pairings, entitlement
evidence, and receipts. Remove or repoint only installer-owned release files;
never erase private state to resolve an activation failure.
