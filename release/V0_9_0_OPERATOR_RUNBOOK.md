# TOHSENO 0.9.0 release and activation runbook

This runbook prepares the external actions required for 0.9.0. It is not
authorization to perform them. Stop before **Publication ceremony** unless the
repository owner explicitly authorizes a tag, GitHub release, production
relay, APNs, DNS, and public-installer change. Record the authorization in
`V0_9_0_READINESS.json`; do not infer it from source acceptance.

Immutable 0.9.0 artifacts must never be replaced in place. A defect after
publication is corrected in a new release.

## 1. Source and authority preflight

From the repository root:

```bash
git status --short
git rev-parse 'HEAD^{commit}'
git log -1 --show-signature --format=fuller
jq -e '
  .schema == "tohseno.release-readiness/1"
  and .version == "0.9.0"
  and .channel == "stable"
  and .ready == true
  and .authorization.authorized == true
  and ((.authorization.authorized_by | type) == "string")
  and (.authorization.authorized_by | length) > 0
  and ((.authorization.authorized_at | type) == "string")
  and ((.authorization.authorization_form | type) == "string")
  and (.authorization.authorization_form | length) > 0
  and .protection.immutable_release_policy_verified == true
  and .protection.release_tag_protection_verified == true
  and ((.protection.release_tag_ruleset_id | type) == "number")
  and .protection.release_tag_ruleset_id > 0
  and ((.required_local_evidence | type) == "object")
  and ([.required_local_evidence[] | select(. != true)] | length) == 0
' release/V0_9_0_READINESS.json
```

The tree must be clean, the intended commit must be reviewed, and the
readiness record must name the actual owner authorization. Before marking it
ready, confirm all source version checks target 0.9.0 while the published
installer copies still deliberately target 0.8.5.

```bash
rg -n '0\.8\.5|0\.9\.0' \
  Cargo.toml cli/Cargo.toml node/Cargo.toml website/package.json \
  scripts/release.sh .github/workflows/release.yml oneshot/oneshot.sh \
  docs release studio sdk
cmp website/apps/site/public/oneshot.sh website/apps/site/public/install.sh
```

Any unexpected old version in source release validation is a blocker. The
intentional 0.8.5 public pin and historical evidence are not rewritten.

## 2. Complete local verification

Run the required suites from the clean commit and record exact results:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features

swift build --package-path apple-identity
swift test --package-path apple-identity
swift test --package-path fascia/apple
swift test --package-path sdk/apple/TohsenoCompanionKit

forge build --root contracts
forge test --root contracts -vvv

(cd website && bun run typecheck)
(cd website && bun test)

./scripts/test-ontology-lifecycle.sh
./scripts/test-local-companion-e2e.sh
./scripts/test-macos-service-lifecycle.sh
./scripts/test-installer.sh
./scripts/test-installer-claim-args.sh
./scripts/test-release-package-integrity.sh
```

The two named companion and macOS lifecycle commands above are the checked-in
end-to-end tests. They use temporary install, data, Shot, relay, LaunchAgent,
and fake-launchctl roots while exercising the real service process. They do
not replace `HOME`: the macOS login Keychain remains available, and each test
deletes only the exact temporary Keychain references it created. The lifecycle
fixture never invokes `/bin/launchctl` or alters the operator's real
LaunchAgent.

Required evidence includes fresh install, reinstall, update, failing-update
rollback, Terminal exit with continuing service health, restart, unsafe path
and symlink rejection, ordinary uninstall, and app/identity/journal/pairing
preservation. Do not mark readiness true if any test is skipped because a
local code path is disconnected.

## 3. Build release-candidate artifacts

Build each supported target from the same clean commit on the controlled macOS
release runner:

```bash
TOHSENO_RELEASE_TARGET=aarch64-apple-darwin ./scripts/release.sh
TOHSENO_RELEASE_TARGET=x86_64-apple-darwin ./scripts/release.sh
```

`scripts/release.sh` must capture the commit, reject a dirty tree, build from
an immutable clean snapshot, verify Mach-O architecture and deployment target,
and publish only beneath:

```text
dist/release-candidate/aarch64-apple-darwin/
dist/release-candidate/x86_64-apple-darwin/
```

Verify both package manifests and release identities:

```bash
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  package="dist/release-candidate/$target"
  python3 scripts/release-package-integrity.py verify-manifest --root "$package"
  jq -e \
    --arg target "$target" \
    --arg commit "$(git rev-parse 'HEAD^{commit}')" \
    '.schema == "tohseno.release/1"
     and .version == "0.9.0"
     and .target == $target
     and .source_commit == $commit
     and .dirty == false
     and .channel == "stable"
     and .prerelease == false' \
    "$package/RELEASE.json"
done
```

Archive and checksum through the pinned release workflow; do not hand-edit a
manifest. Save the candidate manifest digests, full source commit, runner
identity, and suite results in the readiness evidence.

## 4. Independent checksum verification

Use a second clean machine or separately controlled account. After artifacts
exist on the immutable draft release:

```bash
audit_dir="$(mktemp -d /tmp/tohseno-0.9.0-audit.XXXXXX)"
chmod 0700 "$audit_dir"
gh release download v0.9.0 --repo jpfraneto/tohseno --dir "$audit_dir"
(
  cd "$audit_dir"
  shasum -a 256 -c SHA256SUMS
  ./tohseno-aarch64-apple-darwin --version
  ./tohseno-apple-identity-aarch64-apple-darwin --version
  jq -e '.schema == "tohseno.release/1" and .version == "0.9.0"' \
    tohseno-release-aarch64-apple-darwin.json
  jq -e '.schema == "tohseno.release/1" and .version == "0.9.0"' \
    tohseno-release-x86_64-apple-darwin.json
)
```

Expect exact `tohseno 0.9.0` and `tohseno-apple-identity 0.9.0` output. Verify
the tag resolves to the recorded source commit and the release asset set has
no extras or omissions. Record the independent verifier and each SHA-256;
do not rely only on the producing workflow's green status.

## 5. Publication ceremony — owner authorization required

Only after the owner authorization and immutable-tag protection are recorded:

```bash
release_commit="$(git rev-parse 'HEAD^{commit}')"
git tag -s v0.9.0 "$release_commit" -m 'TOHSENO 0.9.0'
git push origin refs/tags/v0.9.0
gh run watch --repo jpfraneto/tohseno --exit-status
gh release view v0.9.0 --repo jpfraneto/tohseno --json tagName,isDraft,isPrerelease,url
```

The pinned release workflow, not an operator's ad hoc upload, must build and
attach the exact binaries, helper, released materials, release JSON,
per-target package checksums, installer, and aggregate `SHA256SUMS`. Keep the
release draft until the independent verification in step 4 succeeds. Publish
the immutable release only after that verification; never overwrite an asset.

## 6. Production Companion Relay canary — separate authorization required

Deploy the reviewed source with the relay disabled first. Configure the host's
normal TLS termination, bounded connections, monitoring, backups, and an
absolute owner-controlled durable data root. Then set these application values
for a no-APNs canary:

```text
NODE_ENV=production
BASE_URL=https://<approved-companion-relay-origin>
HOST=<approved-bind-host>
PORT=<approved-port>
TRUST_PROXY=<true only behind the reviewed proxy>
COMPANION_RELAY_ENABLED=true
COMPANION_RELAY_ACTIVATION_READY=true
COMPANION_RELAY_ROOT=<absolute durable private path>
COMPANION_RELAY_PUSH_MODE=noop
```

Set explicit production capacity, retention, catch-up, and source/global rate
limits rather than accepting infrastructure defaults. Run
`bun run companion-relay:cleanup` independently on a supervised schedule.

Verify HTTPS health, one-use pairing, opaque online delivery, offline catch-up,
acknowledgement cleanup, duplicate rejection, bounded expiry, cursor reset,
rate/capacity refusal, content-free logs, and revocation with the 0.9.0 Mac and
simulated Companion. Inspect storage to confirm it contains only outer
routing data and ciphertext. Record the canary; do not expose a broad client
population yet.

## 7. Optional APNs activation — separate authorization required

Foreground and launch reconciliation are the initial production-safe mode.
If the owner separately authorizes APNs, store an Apple provider `.p8` key as
an owner-only, non-symlinked, bounded regular file and set:

```text
COMPANION_RELAY_PUSH_MODE=apns
COMPANION_RELAY_APNS_ENVIRONMENT=production
COMPANION_RELAY_APNS_TEAM_ID=<10-character-team-id>
COMPANION_RELAY_APNS_KEY_ID=<10-character-key-id>
COMPANION_RELAY_APNS_TOPIC=<approved-companion-bundle-id>
COMPANION_RELAY_APNS_PRIVATE_KEY_PATH=<absolute-owner-only-p8-path>
```

Restart and confirm startup fails when any credential is absent or malformed.
Send a canary wake-up and verify the payload contains no Shot, workspace,
command, content, or stable routing identifier. Confirm token values never
appear in logs. If APNs is not authorized, keep `noop` and record that choice
as intentional rather than degraded service.

## 8. Public installer pin — last activation step

Do not perform this step until the immutable release and relay canary are
verified. Update the canonical installer to exact v0.9.0 artifact names,
checksums, and binary/service health assertions, run its acceptance suite, then
copy that exact reviewed byte sequence to both public aliases:

```bash
cp oneshot/oneshot.sh website/apps/site/public/oneshot.sh
cp oneshot/oneshot.sh website/apps/site/public/install.sh
cmp oneshot/oneshot.sh website/apps/site/public/oneshot.sh
cmp oneshot/oneshot.sh website/apps/site/public/install.sh
./scripts/test-installer.sh
./scripts/test-installer-claim-args.sh
```

Deploy the website only with owner authorization. From an independent machine,
download—not pipe—the live installer and compare it with the reviewed source:

```bash
live_installer="$(mktemp /tmp/tohseno-live-installer.XXXXXX)"
curl -fsSL https://tohseno.com/oneshot.sh -o "$live_installer"
cmp oneshot/oneshot.sh "$live_installer"
shasum -a 256 oneshot/oneshot.sh "$live_installer"
```

Then use an isolated macOS test home to run the downloaded installer. Verify
the reported release and service versions are 0.9.0, Studio health is bound to
loopback, the installer process exits, the service remains healthy after its
Terminal closes, and ordinary uninstall preserves app and pairing state.

## 9. Rollback

If a staged local update fails health, the installer must automatically restore
the previous `current` pointer, restart that service, verify rollback health,
and exit nonzero. Preserve the failed staged evidence; do not delete private
service state.

If production activation fails:

1. restore the website from the reviewed commit whose installer pins immutable
   0.8.5 and verify byte identity from an independent machine;
2. set `COMPANION_RELAY_ENABLED=false` and
   `COMPANION_RELAY_PUSH_MODE=noop`, then restart the relay process;
3. preserve relay storage and logs under the incident retention policy rather
   than deleting evidence;
4. leave the v0.9.0 tag and release immutable;
5. document the failure and publish a corrected v0.9.1 only after a new
   readiness and authorization ceremony.

Never “roll back” by mutating a release asset, moving an existing tag, deleting
user app folders, resetting Builder identity, or removing command journals and
pairing state.
