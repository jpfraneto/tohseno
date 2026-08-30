# Native macOS distribution runbook

ADR 0025 makes `Tohseno.app` the primary consumer artifact. Repository source
is not release evidence. The public download stays disabled until the exact
candidate has been signed with Developer ID, notarized, stapled, independently
verified, uploaded, and activated by digest.

For 1.1.0, perform this artifact ceremony inside the ordering and production
acceptance defined by
[`PERSON_TO_PERSON_NETWORK.md`](PERSON_TO_PERSON_NETWORK.md). Historical 1.0.2
candidate evidence cannot authorize or describe a 1.1.0 artifact.

## Build an unsigned candidate

Use a clean checkout on a supported Mac with Xcode, Swift 6, Rust, both Apple
Rust targets, `lipo`, `iconutil`, and `sips`:

```sh
rustup toolchain install 1.88.0
rustup target add --toolchain 1.88.0 aarch64-apple-darwin x86_64-apple-darwin
export TOHSENO_WELCOME_COMPUTE_URL='mailto:welcome@example.com'
macos/Tohseno/Packaging/build-app.sh
macos/Tohseno/Packaging/verify-app.sh dist/native/Tohseno.app unsigned
```

The packaging script defaults to the repository's minimum Rust 1.88 toolchain
and invokes that toolchain's compiler explicitly, even when another `rustc` is
earlier on `PATH`. `TOHSENO_RUST_TOOLCHAIN` may select a newer, already-audited
toolchain for a candidate; record that override in release evidence.
`TOHSENO_WELCOME_COMPUTE_URL` is optional release-time configuration and must
be an HTTPS or `mailto:` destination. Release builds compile it into the helper
so an untrusted local process cannot redirect the contact action at runtime.
Record and review the exact destination; omit it to hide the action.

The build creates a universal app containing the native SwiftUI executable,
the Rust native-session/service helper, and a checksum-covered factory release.
It does not read or bundle operator credentials. First open verifies that
manifest before atomically selecting the bundled service release under
`~/.tohseno`; application folders, command state, identities, entitlements,
and Companion pairings stay outside the release payload.

## Sign and notarize

Keep the Developer ID certificate and notary profile in Apple Keychain. Do not
export either into the repository or CI logs.

```sh
export TOHSENO_DEVELOPER_ID_APPLICATION='Developer ID Application: … (TEAMID)'
export TOHSENO_DEVELOPER_TEAM_ID='TEAMID'
export TOHSENO_NOTARY_KEYCHAIN_PROFILE='tohseno-notary'
macos/Tohseno/Packaging/sign-and-notarize.sh dist/native/Tohseno.app notarize
macos/Tohseno/Packaging/verify-app.sh dist/native/Tohseno.app notarized
macos/Tohseno/Packaging/create-dmg.sh
```

Signatures are applied inside-out. Signing the nested factory executables
changes their bytes, so the factory manifest is regenerated before the outer
app is signed. The helper verifies that its own signature and the parent app
share this Team ID and that the parent identifier is exactly `com.tohseno.mac`.
Record the `notarytool` submission ID, stapler result, Team ID,
app designated requirement, DMG SHA-256, clean commit, and build-machine OS and
Xcode versions in the private release evidence.

## Clean-Mac acceptance

Use a Mac/user account with no Node, npm, Bun, Homebrew TOHSENO,
`~/.tohseno`, or existing LaunchAgent. Before stable publication, the exact
candidate may be distributed through ADR 0031's explicitly labeled public
release-candidate channel so this acceptance exercises the real website
download path:

1. Mount the DMG and confirm Finder restores the branded app-to-Applications
   drag window. Drag `Tohseno.app` to Applications, eject it, and open it
   through Finder. Gatekeeper must accept it without bypass instructions.
2. Confirm the app explains Tohseno before setup, uses `Tohseno` consumer
   spelling, shows the SVG menu-bar item, presents one readiness instruction at
   a time, and never asks for Apple credentials. Complete Xcode
   license/components, cable, unlock, Trust, Developer Mode, and Personal Team
   guidance.
3. Quit during setup and during admitted work; reopen and verify restoration.
4. Verify the readiness app is Tohseno Companion, the staged progress bar
   advances through build/install/launch/pairing, and the Mac does not advance
   until the Companion proves its private workspace pairing. Confirm apps on
   the Mac appear on the phone.
5. Confirm a signed-in Codex installation appears in the open **Choose
   intelligence** control. Select guided iPhone capabilities, edit the seeded
   intention, and create through that route. Verify an ordinary Git repository,
   deterministic build gates, and direct iPhone install/launch.
6. Evolve the same app, exercise stale-base refusal, unplug/replug recovery,
   Open Source, Open on iPhone, retry, and Details.
7. Confirm Registry lists locally verified apps under **Apps on this Mac** and
   shows no published app cards while registry RPC/catalog discovery is absent.
8. Install over an existing healthy CLI/Studio workspace. Verify in-place
   adoption and that apps, history, identity, private records, and pairings are
   unchanged. Force a service activation failure in an isolated fixture and
   verify rollback to the prior selected release.
9. Search the app and DMG for Stripe, Bankr, operator, signing, relay, and test
   secrets. Verify both architectures and the exact factory manifest again.

Physical iPhone installation, Developer ID signing, notarization, Gatekeeper,
and a clean-machine walkthrough cannot be replaced by unit tests. If any is
missing, describe it as unverified and do not promote the candidate to stable.
Only ADR 0031's bounded release-candidate download may remain active for the
acceptance walkthrough.

## Publish without guessing

Upload the immutable DMG to an HTTPS download origin, download it again, and
compare its SHA-256. For ADR 0031 acceptance use a public prerelease tag and set
`MACOS_DOWNLOAD_CHANNEL=release-candidate`; the website and API must identify
that channel. Then configure the website:

```text
MACOS_DOWNLOAD_ENABLED=true
MACOS_DOWNLOAD_CHANNEL=release-candidate
MACOS_DOWNLOAD_URL=https://…/Tohseno-1.0.2-rc.2.dmg
MACOS_DOWNLOAD_SHA256=<exact-lowercase-digest>
```

Verify `GET /api/distribution/v1/macos`, `HEAD /download/macos`, the landing
button, CDN behavior, and the downloaded digest. From a clean interactive Mac
terminal, verify that the canonical command accepts only Enter or Escape,
restores the terminal on exit, displays the download progress, verifies before
exposure, places the exact DMG in Downloads, and reveals it in Finder without
copying, replacing, or opening an application. Complete the ordinary Finder
drag and first-open walkthrough with the exact downloaded bytes. A source
merge, stable Git tag, successful local build, or notarization submission alone
is not permission to set `MACOS_DOWNLOAD_ENABLED=true`. ADR 0031 additionally
requires explicit owner authorization, exact origin round-trip verification,
prerelease labeling, and an unpublished stable tag.

After clean-Mac acceptance and every remaining release gate passes, publish the
protected stable tag for the exact accepted digest, switch
`MACOS_DOWNLOAD_CHANNEL=stable`, and repeat the API, redirect, download,
checksum, Finder, Gatekeeper, first-open, and health checks.

Rollback the download by setting `MACOS_DOWNLOAD_ENABLED=false`; do not replace
bytes at an existing immutable URL. Repair requires a new candidate and the
entire sequence above. No automatic update feed is active in this release.
