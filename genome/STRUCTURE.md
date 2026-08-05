# Structure

- Put one `<app-name>.xcodeproj/project.pbxproj` and all source and resources directly under `src/`.
- Name the single iOS application target and shared scheme exactly `<app-name>`.
- The current Apple factory targets iOS 17 or newer, automatic signing, and a
  directly inspectable Xcode project. These are distribution and current
  factory boundaries, not product-shaping laws.
- Set `IPHONEOS_DEPLOYMENT_TARGET = 17.0`, `CODE_SIGN_STYLE = Automatic`, and `GENERATE_INFOPLIST_FILE = YES`.
- Set `PRODUCT_BUNDLE_IDENTIFIER` from the identity in `TASK.md`.
- Set `CURRENT_PROJECT_VERSION` to the literal token `__TOHSENO_SHOT__`; TOHSENO replaces it before building.
- Set `MARKETING_VERSION = 1.0`, `SWIFT_VERSION = 5.0`, and target iPhone.
- Include `CFBundleDisplayName`, a launch screen declaration, and required usage-description keys for every protected API used.
- The accepted Birth Plan is the intent-level capability declaration: it says
  why each material exists. The engine scans executable Swift and structured
  build metadata, reconciles those observations with the plan, and writes the
  final mechanical `TOHSENO/capabilities.json` used by the Fascia.
- For network use, provide the human facts source analysis cannot invent:
  stable remote origins or `bonjour:<service-type>`, purpose, and nonempty data
  categories. Include matching local-network usage and Bonjour declarations.
  Undeclared observed behavior and stale contradictory declarations fail
  closed with exact structural evidence.
- Native Apple frameworks are the default material. The current factory's
  rejection of uninspected package, binary, or third-party runtime dependencies
  is an explicit `factory_capability_gap`, not a universal ban on what an
  intention may require.
- Add all five `TohsenoFascia/*.swift` reference files to the application target
  and prepare `InstallationIdentity.shared` during first launch.
- Include the engine-produced `TOHSENO/embedded-provenance.json` as a bundled
  resource without editing it.
- Keep the project discoverable by `xcodebuild -project <app-name>.xcodeproj -scheme <app-name>`.
- Keep the engine-written `AGENTS.md` at the project root. `MEMORY.md` and
  `WORLD.md` are optional high-signal source artifacts; never bundle them or
  create them as a ritual before the product is complete.
