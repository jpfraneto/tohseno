# Local native feedback loop

For owner development, run changed clients locally after relevant edits. Public
DMG publication is for distribution and acceptance of that exact artifact, not
a prerequisite for inspecting a UI change. Keep the website pins unchanged.

## Mac UI

```sh
bash scripts/dev-macos.sh
```

This incrementally builds the host-architecture debug UI, copies the installed
Mac bundle into a new directory under `dist/local-macos`, replaces only its UI
executable, signs with the matching installed Developer ID, verifies the app,
and opens it. The window title says Local with source revision/time and a dirty
marker for tracked UI changes. This is a live client of the owner's factory,
not a simulated screenshot. Buttons perform their normal real actions.

The script requires the installed app's factory manifest to equal the active
factory manifest so opening the preview cannot silently select an older bundled
runtime. It preserves the exact bundled factory and helpers. Rust/service edits
therefore need a separately rebuilt runtime; this command does not preview them.

Existing windows are left open because their unsent composers are in memory.
Save drafts and close the old preview when switching. Never kill all Tohseno
processes or restart the factory just to refresh a SwiftUI view. Keep previews
until no longer useful; they are local signed builds, not notarized releases.

## Physical Companion

Resolve the currently intended device using `xcrun devicectl list devices` and
`device info details`. Match its CoreDevice identifier against the saved
Companion setup digest using the existing `TOHSENO-CABLE-DEVICE-V1` domain.
Use the observed hardware UDID for Xcode and the matching CoreDevice identifier
for installation; never infer a target from the first available device.

```sh
xcodebuild -quiet \
  -project companion/apple/TohsenoCompanion/App/TohsenoCompanion.xcodeproj \
  -scheme TohsenoCompanion -configuration Debug \
  -destination 'platform=iOS,id=OBSERVED_HARDWARE_UDID' \
  -derivedDataPath dist/local-companion build
xcrun devicectl device install app --device CONFIRMED_COREDEVICE_ID \
  dist/local-companion/Build/Products/Debug-iphoneos/TohsenoCompanion.app
xcrun devicectl device info apps --device CONFIRMED_COREDEVICE_ID \
  --bundle-id com.tohseno.companion
xcrun devicectl device process launch --device CONFIRMED_COREDEVICE_ID \
  com.tohseno.companion
```

Replace placeholders only with observed values. Preserve the existing bundle
identifier, Apple team and Keychain access identity. Update in place; do not
uninstall or reset pairing. Verify the built signature/entitlements before
installing. Apple account, signing permission, lock or Trust prompts remain
owner actions. After launch, observe Companion's actual pairing and workshop
state; an install command alone does not prove those survived or work.

Reuse DerivedData for subsequent changes. Build and relaunch the affected client
after a coherent edit, rather than on every keystroke. There is no automatic
filesystem watcher or SwiftUI hot reload in this workflow.

## Phone-to-Mac work and continuous availability

On the owner's Mac, `~/Library/LaunchAgents/com.tohseno.workshop-awake.plist`
runs `/usr/bin/caffeinate -s` through the logged-in user's launchd session. This
is local operating configuration, not a new factory or public release. Connect
the Mac to power, leave the lid open and the user logged in, and retain internet
access. The display may sleep. Battery operation, closing the lid, logout and
shutdown are not continuous workshop availability.

Inspect or remove that local wake assertion with:

```sh
launchctl print gui/$(id -u)/com.tohseno.workshop-awake
pmset -g assertions
# To disable it for this login session:
launchctl bootout gui/$(id -u)/com.tohseno.workshop-awake
# Remove the plist too if it should not return at the next login.
```

In Companion, create a Shot with the central button or open an app and use
Evolve App. Keep Companion open until Workshop activity reports that the Mac
accepted the request. The request is saved locally before submission; closing
before acknowledgement preserves it but foreground reconciliation may be
required to deliver it. Once accepted, the Mac continues independently. Expand
the request to see its intention, timestamped Mac reports and exact command ID.
Relay connectivity is not proof that the Mac is awake. Last Mac report is dated.

The phone log projects existing signed execution events. Detailed harness/Xcode
output stays on the Mac. An intended iPhone can submit remotely over the relay;
actual installation still requires that exact device to be reachable through
Apple's supported USB/Xcode Wi-Fi path and to satisfy its unlock/Trust rules.

Real acceptance is a phone-origin request, acknowledgement from this Mac,
observed harness/source changes and a build result, followed by exact intended
phone delivery when reachable. Do not replace that check with a CLI-origin
request, a Simulator, or a rendered request-history fixture.

A Rust service change needs a complete local factory bundle, not a bare
`target/debug/tohseno service run`: Apple identity discovery expects its bundled
helper beside the executable. For a local-only test, retain the installed
FactoryRelease resources and identity helper, replace both the bundled factory
CLI and native session helper with the compiled CLI, record the exact source
commit and base manifest digest, regenerate the file manifest after signing,
and sign/verify the containing app with the existing Developer ID. Open that
app so the existing native installer selects the complete release and restores
the normal LaunchAgent. This is a local build, not a notarized distribution or
public installer activation. Do not replace or fabricate the private command
journal when recovering a failed request.
