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
xcodebuild -project Writing.xcodeproj -scheme Writing \
  -destination 'platform=iOS Simulator,name=iPhone 16 Pro' test
```

## What's inside

```text
App/                     SwiftUI sources
  AppConfig.swift        THE configuration seam: feature flags + key slots
  Identity/              BIP39 seed phrase, keypair, keychain — the no-auth spine
  Sessions/              atomic local persistence with crash recovery
  Modules/               paywall (off), share card (on), notifications (off),
                         SessionLink (reserved)
  Views/                 writing surface, session log, settings
Tests/                   the invariant tests that keep the spine honest
site/index.html          the app's landing page — one static file, ships with the app
continuity.manifest.json machine-readable record of what this app does
fastlane/Fastfile        the prepared TestFlight lane
scripts/setup.ts         one-time credential flow (bun run setup)
project.yml              XcodeGen source of truth for Writing.xcodeproj
```

## The spine

- **Identity is a seed phrase.** Silently generated on first launch, stored in
  the keychain, revealed or restored only from Settings. The derived key's
  fingerprint is the user ID. No accounts, no logins, no email — for you or
  your users.
- **Data is files.** Each kept session is a plain `.txt` plus a small JSON
  sidecar, written atomically. A killed process never loses committed text.
- **Modules are flags.** `AppConfig.swift` is the single seam. Flipping a flag
  is the only integration step; every module compiles cleanly when off.
- **Ejectable from birth.** Everything here builds and runs without any
  TOHSENO credential.

## To your phone

```sh
bun run setup     # app name, bundle ID, Team ID, optional ASC + RevenueCat keys
fastlane beta     # prepared TestFlight upload — you run this, agents only print it
```

Setup writes `app.config.json` and `Config/Local.xcconfig` (both gitignored;
key *paths* and public identifiers only — secret values never enter git).

## Changing the project

Edit `project.yml`, then `xcodegen generate`. If you don't have XcodeGen and
aren't adding targets, editing the project in Xcode directly is fine too.
