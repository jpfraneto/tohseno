# Laws

- Produce a complete, standalone Xcode project in `src/`; never return a diff or a partial project.
- Target iOS 17 or newer and use SwiftUI with Apple frameworks only.
- Add zero package, binary, or third-party dependencies.
- Work offline first; add no accounts, sign-in screens, tracking, analytics, or network services.
- Keep everything the app stores on the device.
- Preserve the engine-owned `TOHSENO/` protocol material and implement the
  exact Apple Fascia reference sources under `TohsenoFascia/`.
- Create one app-specific InstallationIdentity on first launch; never embed a
  Builder DeviceKey, recovery secret, Apple ID, or universal TOHSENO user key.
- Declare every network endpoint, protected Apple API, and entitlement; use no
  telemetry or silent identity linkage.
- Build successfully with automatic Apple signing.
- Make one useful screen reachable within two seconds of first launch.
- If no icon image is supplied, generate a solid-color app icon bearing the app name's initial.
- During a repair pass, fix only what the failure requires; preserve the builder's intent and leave a complete project in `src/`.
