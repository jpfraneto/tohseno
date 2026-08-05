# Laws

- Produce a complete, standalone Xcode project in `src/`; never return a diff or a partial project.
- Target iOS 17 or newer and use SwiftUI with Apple frameworks only.
- Add zero package, binary, or third-party dependencies.
- Work offline first. Do not invent an account, network service, or protected
  capability that the intention does not need, and do not weaken an explicit
  product requirement merely to keep the app offline-only.
- Let each Shot use the native Apple frameworks and capabilities its intention
  requires. Implement the real interaction instead of substituting a mock,
  manual export, or explanatory screen for a permitted native capability.
- Keep private state on the device by default. Transmit or synchronize only the
  data categories the intention requires, to explicitly declared endpoints,
  for a stated purpose visible in the app's source declaration.
- Preserve the engine-owned `TOHSENO/` protocol material and implement the
  exact Apple Fascia reference sources under `TohsenoFascia/`.
- Create one app-specific InstallationIdentity on first launch; never embed a
  Builder DeviceKey, recovery secret, Apple ID, or universal TOHSENO user key.
- Declare every network endpoint or local discovery service, its purpose, and
  transmitted data categories. Declare every protected Apple capability and
  entitlement with its purpose, and include every required usage description.
- Authentication may be used when the intention needs it, but it must not be
  confused with TOHSENO ownership or silently link installations. Add no
  tracking, advertising identifiers, analytics, or telemetry.
- Build successfully with automatic Apple signing.
- Make one useful screen reachable within two seconds of first launch without a
  mandatory account wall or onboarding carousel.
- If no icon image is supplied, generate a solid-color app icon bearing the app name's initial.
- During a repair pass, fix only what the failure requires; preserve the builder's intent and leave a complete project in `src/`.
