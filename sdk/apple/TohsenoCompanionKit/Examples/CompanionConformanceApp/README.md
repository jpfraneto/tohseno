# Companion conformance fixture

This is a deliberately small SwiftUI fixture, not the branded TOHSENO app. Its
checked-in Xcode project targets iOS 17 and resolves the adjacent CompanionKit
directory as a local Swift Package dependency.

The fixture demonstrates identity creation/restoration, scanning an exact QR
payload with the real camera, receiving a workspace snapshot, queuing feedback,
marketing, evolution and creation commands, foreground/push reconciliation,
and the visible revoked state. `CompanionPushTokenProvider` is the narrow push
adapter, so protocol behavior can be exercised without APNs credentials.

Debug verification builds may set the generated Info.plist key
`TOHSENOVerificationRelayOrigin` to one temporary HTTPS origin. The QR still
contains only the signed allowlisted relay identifier; it never carries an
arbitrary relay URL. Release builds ignore this verification-only key.

Pairing starts the bounded foreground SSE synchronization loop. The explicit
`START LIVE SYNC` and `STOP LIVE SYNC` controls exercise reconnection and clean
cancellation independently, while authenticated workspace and execution events
update the visible Shot list without polling.

The checked-in Xcode project uses the adjacent CompanionKit directory as a
local Swift Package dependency. Build it without signing with:

```sh
xcodebuild \
  -project CompanionConformanceApp.xcodeproj \
  -scheme CompanionConformanceApp \
  -sdk iphonesimulator \
  -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO build
```
