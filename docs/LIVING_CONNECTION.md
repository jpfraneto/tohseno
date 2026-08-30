# The living connection

This is the implementation and local acceptance guide for ADR 0033. It is
descriptive; `protocol/` remains authoritative over public encodings.

## Product boundary

Tohseno connects one person's native iPhone app to its source project on their
Mac, the intent/history explaining it, one configured coding harness, the
Companion request surface, and the Apple build/install return path.

It does not require an adopted project to become a protocol Shot, rewrite its
repository, publish it, commit it, push it, deploy it, collect Apple
credentials, install on strangers' phones, or expose raw prompts/logs on the
normal phone path. The generated Shot factory, CLI, local Registry, and browser
Studio remain secondary compatibility/support surfaces.

## Adopt a project

In `Tohseno.app`, choose **Adopt Existing App** and select one exact
`.xcodeproj` or `.xcworkspace`. Tohseno:

1. Lists schemes and probes app-target build settings with `xcodebuild`.
2. Asks for a scheme only when inference remains ambiguous.
3. Records display/product name, bundle ID, container, source root, scheme,
   iOS deployment target, non-secret signing-team setting, Git revision/dirty
   paths, and bounded `AGENTS.md`, `CLAUDE.md`, `MASTER_PROMPT.md`, README, and
   existing Tohseno instruction metadata.
4. Creates a random stable `project_<uuid>` identity that is not a protocol
   digest and is not derived only from the folder path.
5. Leaves the repository structure and working tree untouched.
6. Performs a real unsigned Simulator build and reports its real result.
7. If exactly one development iPhone is reachable, observes whether the exact
   bundle is already installed.

Choosing the same canonical container, scheme, and bundle again preserves the
existing project ID. If the folder later moves, the record retains its former
path and reports source unavailable; automatic relinking is not implemented
yet. Do not delete the record and pretend its history moved.

Private records live under:

```text
~/.tohseno/service/living-projects-v1/
  store.json
  projects/project_<uuid>.json
  projects/project_<uuid>/evolutions/evolution_<uuid>/
  commands/<command-id>.json
```

The store is schema-versioned, bounded, permissioned `0700/0600`, rejects
symlink paths, writes replacements atomically, and fails closed on an unknown
store version.

## Pairing and transport

Mac Settings lists active/revoked Companion devices and can rename, revoke, or
create a two-minute one-use QR invitation. The Companion scans that invitation,
proves possession of its signing/agreement keys, and receives a scoped grant.
Pairing completes only after the Mac publishes an authenticated encrypted
workspace snapshot.

Phone identity and agreement keys live in iOS Keychain. Mac workspace identity
lives in Keychain. Phone state/outbox files are encrypted in protected
Application Support; Mac device/mailbox records are private service files. Raw
secrets are redacted from debug output and never appear in UI or repository
metadata. Revocation updates local authorization first and revokes both opaque
relay mailboxes; future signed requests from that phone are rejected.

The current production transport is the existing HTTPS content-blind relay,
not Bonjour. Reusing it preserves working durable delivery and already allows
a phone away from the Mac to queue a request. The relay sees routing metadata
and ciphertext but has neither content keys nor project authority. A future
same-LAN transport can sit behind the same signed-command/outbox boundary.

## One request

Companion shows adopted apps, their Mac/status, latest history, and **Evolve
App**. A request can include edited text, native speech transcription, and up
to eight PNG/JPEG images. The signed payload binds:

- stable project ID and current private source-state token;
- request text and attachment blob references;
- originating paired device and timestamp;
- optional follow-up evolution ID.

The SDK persists the signed command and encrypted attachment/outbox bytes
before returning from the send action. Relaunch/foreground reconciliation is
idempotent. The Mac authenticates the device, grant, signature, replay state,
exact project/base, and attachments, then persists the evolution/command index
before returning an accepted receipt.

## Harness, build, and install

The first concrete adapter is the already configured harness selected by the
factory (Codex when configured). It runs in the adopted source root with a
private execution packet containing the exact request, references,
instructions and digests, current source/Git observation, build container and
scheme, prior relevant state, and safety constraints. It is told to inspect
first, preserve unrelated work, avoid destructive Git, run relevant tests, and
never commit, push, publish, or deploy implicitly.

The record captures pre-existing dirty paths separately from the post-run Git
observation. Rollback is `false` because a general automatic reset cannot be
safe in a dirty owner repository.

After a successful harness exit, Tohseno runs a real signed `iphoneos`
`xcodebuild`, finds the `.app`, and verifies it with `codesign`. It then uses
`xcrun devicectl device install app` only if exactly one ready physical iPhone
is resolved. `Installed` requires a second `devicectl device info apps
--bundle-id …` query to find that exact bundle. A successful build alone is
never Installed.

Source/compiler/test, signing, timeout, locked/unavailable device, Trust,
Developer Mode, multiple-device ambiguity, and install/verification failures
have distinct recorded categories. A verified build waiting for the phone is
retained and retried without rerunning the harness.

If the adopted target is the Companion itself, the command, build, and
installation state are already durable before `devicectl` replaces the app.
The live channel may close during replacement; Companion resumes encrypted
reconciliation after relaunch instead of treating that disconnect as success.

## Apple manual actions

Tohseno cannot honestly perform these Apple-controlled actions:

- Accept Xcode's license/install components and add the Apple Account in
  Xcode's own Settings when no Personal Team is available.
- Unlock the iPhone, tap **Trust This Computer**, and enter its passcode.
- Enable **Settings → Privacy & Security → Developer Mode** and allow restart.
- Reconnect the intended phone if it is not reachable. If multiple iPhones are
  reachable in this slice, disconnect the others so selection is unambiguous.

Apple credentials are never entered into Tohseno.

## Run locally

Build/test the Mac app and launch its Swift package executable:

```sh
swift test --package-path macos/Tohseno
swift run --package-path macos/Tohseno TohsenoMacApp
```

Build/test the Companion library and build its iOS app target:

```sh
swift test --package-path sdk/apple/TohsenoCompanionKit
swift test --package-path companion/apple/TohsenoCompanion
xcodebuild \
  -project companion/apple/TohsenoCompanion/App/TohsenoCompanion.xcodeproj \
  -scheme TohsenoCompanion \
  -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO build
```

For a physical Companion install, open that project in Xcode, select the
owner's Personal Team and iPhone, and Run once if the native onboarding has not
already installed it. Use Xcode's own account/signing surfaces.

## End-to-end acceptance

1. Start `Tohseno.app`; ensure the configured harness is installed and signed
   in.
2. Complete Xcode, Trust, Developer Mode, Companion install, and QR pairing.
3. Confirm pairing reaches the authenticated connected state in both apps.
4. Adopt a small buildable iOS project and choose its app scheme only if asked.
5. Confirm its real Simulator adoption build result and that it appears in
   Companion.
6. In the app's source, note visible text `X`. From Companion request:
   `Change the text on the main screen from X to Y.`
7. Close Companion if desired. Confirm Mac history advances Received → Working
   → Building and records changed files/build result.
8. Keep exactly the intended iPhone connected/unlocked. Confirm Installing is
   shown only during devicectl work.
9. Confirm Installed/Completed appears only after exact bundle inventory
   verification, then open the changed app and verify `Y`.
10. Relaunch both apps and confirm the project and evolution history remain.

Without a physical phone, unit/integration tests cover command canonicalization,
cryptographic admission/revocation, durable outbox replay, routing,
state-machine transitions, harness command construction, Xcode/install error
classification, storage round trips, and UI models. Simulator builds validate
Apple project compilation; they do not prove signing or physical installation.

## Current limitations and next remote milestone

- Adopted-project icon extraction falls back safely to the Tohseno mark.
- The repository has no neutral consumer sample app; the Companion is a real
  product target with a self-update disconnect, so onboarding does not pretend
  it is a risk-free demo.
- Moved-source relinking and safe automatic rollback are not implemented.
- Only one reachable physical iPhone is auto-selected for adopted-app
  delivery; multi-device association UI is future work.
- iOS background execution is not indefinite. A queued request survives, but
  delivery may wait for Companion foreground/push wake.
- A request can already cross the encrypted relay while away from the Mac, but
  Apple installation normally waits until the phone is again reachable to that
  Mac through CoreDevice.

The smallest next milestone is a physical acceptance of **request while away,
build on an awake home Mac, retain Ready to install, then auto-install when the
same phone returns to the Mac's reachable network**. It needs wake/reachability
evidence and an explicit durable Companion-to-CoreDevice association; it does
not need a new cloud execution architecture.
