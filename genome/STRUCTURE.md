# Structure

- Put one `<app-name>.xcodeproj/project.pbxproj` and all source and resources directly under `src/`.
- Name the single iOS application target and shared scheme exactly `<app-name>`.
- Set `IPHONEOS_DEPLOYMENT_TARGET = 17.0`, `CODE_SIGN_STYLE = Automatic`, and `GENERATE_INFOPLIST_FILE = YES`.
- Set `PRODUCT_BUNDLE_IDENTIFIER` from the identity in `TASK.md`.
- Set `CURRENT_PROJECT_VERSION` to the literal token `__TOHSENO_SHOT__`; TOHSENO replaces it before building.
- Set `MARKETING_VERSION = 1.0`, `SWIFT_VERSION = 5.0`, and target iPhone.
- Include `CFBundleDisplayName`, a launch screen declaration, and required usage-description keys for every protected API used.
- Add all five `TohsenoFascia/*.swift` reference files to the application target
  and prepare `InstallationIdentity.shared` during first launch.
- Include the engine-produced `TOHSENO/embedded-provenance.json` as a bundled
  resource without editing it.
- Keep the project discoverable by `xcodebuild -project <app-name>.xcodeproj -scheme <app-name>`.
- Keep the engine-written `AGENTS.md` at the project root, and maintain
  `MEMORY.md` beside it as Memory requires (source-only; never bundled).
