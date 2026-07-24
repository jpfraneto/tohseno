export const CLI_VERSION = "0.4.0" as const;
export const FACTORY_RELEASE_SCHEMA_VERSION = 1 as const;
export const SHOT_SCHEMA_VERSION = 1 as const;
export const GENERIC_SHOT_SCHEMA_VERSION = 2 as const;
export const CONFIG_SCHEMA_VERSION = 1 as const;
export const ONBOARDING_VERSION = 1 as const;
export const IOS_TEMPLATE_VERSION = "continuity-app-ios-v3" as const;
export const MANIFEST_SCHEMA_VERSION = "0.4.0" as const;
export const AGENT_INSTRUCTION = "Read the local AGENTS.md and begin." as const;
export const MAX_FACTORY_RELEASE_FILES = 4_096;
export const MAX_FACTORY_RELEASE_FILE_BYTES = 64 * 1_048_576;
export const MAX_FACTORY_RELEASE_BYTES = 256 * 1_048_576;

export const RELEASE_SOURCE_FILES = [
  "LICENSE",
  "skills/continuity-app/SKILL.md",
  "packages/manifest/app.ts",
  "packages/manifest/app.manifest.schema.json",
  "packages/manifest/cli.ts",
  "packages/manifest/validate.ts",
  "packages/manifest/types.ts",
  "packages/manifest/continuity.manifest.schema.json",
  "packages/cli/factory/AGENTS.md",
  "packages/cli/factory/CLAUDE.md",
  "packages/cli/factory/OPERATIONS.md",
  "packages/cli/factory/shot-machine.ts",
  "packages/cli/factory/runtime/dev.ts",
  "packages/cli/factory/runtime/ios.ts",
  "packages/cli/factory/runtime/production.ts",
  "packages/cli/factory/runtime/shared.ts",
  "packages/cli/factory/runtime/token.ts",
  "packages/cli/factory/shot-verify.ts",
  "packages/cli/package.json",
] as const;

export const REQUIRED_IOS_BASE_FILES = [
  ".gitignore",
  "App/AppConfig.swift",
  "App/Identity/BIP39.swift",
  "App/Resources/bip39-english.txt",
  "App/WritingApp.swift",
  "Config/App.xcconfig",
  "Config/Debug.xcconfig",
  "Config/Local.xcconfig.example",
  "Config/Production.xcconfig",
  "Config/Release.xcconfig",
  "Backend/database.ts",
  "Backend/server.ts",
  "operations/production.json",
  "scripts/validate-production-endpoint.sh",
  "Tests/BIP39Tests.swift",
  "Writing.xcodeproj/project.pbxproj",
  "Writing.xcodeproj/xcshareddata/xcschemes/Writing.xcscheme",
  "continuity.manifest.json",
  "fastlane/Fastfile",
  "package.json",
  "project.yml",
  "scripts/setup.ts",
  "site/index.html",
] as const;

export const REQUIRED_RELEASE_FILES = [
  ...REQUIRED_IOS_BASE_FILES.map((path) => `platforms/ios/base/${path}`),
  "agent/continuity-app/SKILL.md",
  "catalog/kernels/ios-kernel/kernel.json",
  "catalog/kernels/ios-kernel/overlay/App/ShotApp.swift",
  "catalog/kernels/ios-kernel/overlay/Shot.xcodeproj/project.pbxproj",
  "catalog/templates/blank/template.json",
  "catalog/templates/daily-game/template.json",
  "catalog/skills/daily-challenge/skill.json",
  "catalog/skills/local-progress/skill.json",
  "catalog/skills/rank-progression/skill.json",
  "catalog/skills/share-card/skill.json",
  "factory/skills/index.ts",
  "factory/skills/types.ts",
  "manifest/app.ts",
  "manifest/app.manifest.schema.json",
  "manifest/cli.ts",
  "manifest/validate.ts",
  "manifest/types.ts",
  "manifest/continuity.manifest.schema.json",
  "shot/AGENTS.md",
  "shot/CLAUDE.md",
  "shot/OPERATIONS.md",
  "shot/machine.ts",
  "shot/runtime/dev.ts",
  "shot/runtime/ios.ts",
  "shot/runtime/production.ts",
  "shot/runtime/shared.ts",
  "shot/runtime/token.ts",
  "shot/verify.ts",
  "legal/LICENSE",
  "factory/cli/package.json",
  "factory/cli/src/release.ts",
] as const;
