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
