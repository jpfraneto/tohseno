# Your continuity app

This workspace already contains a working iOS app: a lightweight writing app
that demonstrates the whole spine — open, write, it's yours, nobody asked for
your email. Your coding agent mutates this app toward your prompt; it never
starts from an empty directory.

## Run it now

```sh
open Writing.xcodeproj
```

Press ⌘R with any iPhone simulator selected. Zero API keys, zero
configuration. For a physical device, select your signing team in
Signing & Capabilities (or run setup below).

Run the invariant tests with ⌘U, or:

```sh
UDID=$(xcrun simctl list devices available | grep -E '^[[:space:]]+iPhone' | grep -oE '[0-9A-F-]{36}' | head -1)
if [ -z "$UDID" ]; then
  echo "No available iPhone simulator; install one in Xcode → Settings → Platforms." >&2
  exit 1
fi
xcodebuild -project Writing.xcodeproj -scheme Writing \
  -destination "platform=iOS Simulator,id=$UDID" test
```

## What's inside

```text
App/                     SwiftUI sources
  AppConfig.swift        THE configuration seam: feature flags + key slots
  Identity/              BIP39 seed phrase, keypair, keychain — the no-auth spine
  Sessions/              atomic local persistence with crash recovery
  Modules/               paywall (off), share card (on), notifications (off),
                         SessionLink + TokenMint (reserved)
  Views/                 writing surface, session log, settings
Tests/                   the invariant tests that keep the spine honest
site/index.html          the app's landing page — one static file, ships with the app
continuity.manifest.json machine-readable record of what this app does
fastlane/Fastfile        the prepared TestFlight lane
scripts/setup.ts         one-time credential flow (bun run setup)
project.yml              XcodeGen source of truth for Writing.xcodeproj
```

## The spine

- **Identity is a seed phrase.** Silently generated on first launch — unless
  one already exists in iCloud Keychain, in which case it is adopted silently.
  The keychain item is synchronizable: identity is backed up automatically
  through iCloud Keychain, end-to-end encrypted, and survives reinstalls and
  new devices. Reveal/restore in Settings is the manual fallback. The derived
  key's fingerprint is the user ID. No accounts, no logins, no email — for
  you or your users. Identity backup is not content backup: sessions do not
  sync and stay on the device.
- **Data is files.** Each kept session is a plain `.txt` plus a small JSON
  sidecar, written atomically. A killed process never loses committed text.
- **Modules are flags.** `AppConfig.swift` is the single seam. Flipping a flag
  is the only integration step; every module compiles cleanly when off.
- **Ejectable from birth.** Everything here builds and runs without any
  TOHSENO credential.

## To your phone

```sh
bun run setup     # press enter to accept manifest/previous answers where possible
fastlane beta     # prepared TestFlight upload — owner runs; agents only print it
```

Setup writes `app.config.json` and `Config/Local.xcconfig` (both gitignored;
setup itself writes key *paths* and public identifiers, never secret values).
It preserves comments and assignments it does not own on every rerun.

Setup explains why each answer matters and what skipping costs. It defaults to
the previous run, then `application.name` / `application.id` in the manifest,
and auto-detects the Apple Team ID for confirmation. Skipping Apple signing is
fine for the simulator; skipping App Store Connect means no TestFlight until
you rerun setup.

For an App Store Connect key, create and download it once at: App Store Connect
→ Users and Access → Integrations → App Store Connect API → Team Keys →
"+" → role App Manager → download once. Before writing either config file,
setup makes a read-only `GET /v1/apps?limit=1` request to validate the key,
issuer, and access.

With explicit owner approval, an agent may use non-interactive setup:

```sh
bun run setup --from-manifest --team auto
bun run setup --from-manifest --team TEAMID1234 \
  --asc-key /absolute/path/key.p8 \
  --asc-key-id KEYID12345 --asc-issuer-id 00000000-0000-0000-0000-000000000000
```

The key ID is inferred from `AuthKey_<ID>.p8` when possible; the issuer can
come from an earlier local setup. Add `--revenuecat-key <public-key>` only for
an enabled paywall. Environment equivalents are
`TOHSENO_FROM_MANIFEST=1`, `TOHSENO_APPLE_TEAM_ID`, `TOHSENO_ASC_KEY_PATH`,
`TOHSENO_ASC_KEY_ID`, `TOHSENO_ASC_ISSUER_ID`, and
`TOHSENO_REVENUECAT_PUBLIC_KEY`. Credentials arrive only by path or environment
and remain outside git.

### Prototype provider credentials

If the app cannot obtain short-lived provider credentials yet, the one blessed
development exception is `DEV_SECRET` in gitignored `Config/Local.xcconfig`;
[`Config/Local.xcconfig.example`](Config/Local.xcconfig.example) shows the
shape. Declare that seam in `operations.developmentSecrets` with the canonical
manifest slot `dev-secret`. It is for an owner-controlled development device only, never a
distributed build. The committed build settings expose it only to Debug
iphoneos builds and force it empty for simulators and Release archives. Read it
through `AppConfig.developmentSecret`, do not add another Info.plist injection.

Before TestFlight or any other distribution, remove the local value and replace
it with short-lived credentials from the reserved TokenMint service pattern.
The mint receives no user content; an app that needs it declares that server
as `requiresServer: "credential-minting-only"` and names any metered provider
in its manifest.

## Changing the project

`Writing.xcodeproj` is generated, not file-system-synced. Run
`xcodegen generate` after editing `project.yml` or adding, removing, or moving
any Swift file, then build. (`brew install xcodegen` if it is unavailable.)
