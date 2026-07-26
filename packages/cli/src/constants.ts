export const CLI_VERSION = "0.5.0" as const;
export const FACTORY_RELEASE_SCHEMA_VERSION = 1 as const;
export const SHOT_SCHEMA_VERSION = 1 as const;
export const CONFIG_SCHEMA_VERSION = 1 as const;
export const ONBOARDING_VERSION = 1 as const;
export const IOS_TEMPLATE_VERSION = "ios-kernel-v1" as const;
export const MANIFEST_SCHEMA_VERSION = "1.0.0" as const;
export const AGENT_INSTRUCTION = "Read the local AGENTS.md and begin." as const;
export const MAX_FACTORY_RELEASE_FILES = 4_096;
export const MAX_FACTORY_RELEASE_FILE_BYTES = 64 * 1_048_576;
export const MAX_FACTORY_RELEASE_BYTES = 256 * 1_048_576;

export const RELEASE_SOURCE_FILES = [
  "LICENSE",
  "packages/manifest/app.ts",
  "packages/manifest/app.manifest.schema.json",
  "packages/manifest/cli.ts",
  "packages/cli/factory/AGENTS.md",
  "packages/cli/factory/CLAUDE.md",
  "packages/cli/factory/OPERATIONS.md",
  "packages/cli/factory/shot-machine.ts",
  "packages/cli/factory/runtime/ios.ts",
  "packages/cli/factory/runtime/shared.ts",
  "packages/cli/factory/shot-verify.ts",
  "packages/cli/package.json",
] as const;

export const REQUIRED_RELEASE_FILES = [
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
  "factory/identity/src/index.ts",
  "factory/signer/src/index.ts",
  "factory/protocol/src/index.ts",
  "factory/protocol/src/types.ts",
  "factory/protocol/src/canonical.ts",
  "factory/protocol/src/validation.ts",
  "factory/registry/src/index.ts",
  "factory/node-client/src/index.ts",
  "factory/node-client/src/http.ts",
  "manifest/app.ts",
  "manifest/app.manifest.schema.json",
  "manifest/cli.ts",
  "shot/AGENTS.md",
  "shot/CLAUDE.md",
  "shot/OPERATIONS.md",
  "shot/machine.ts",
  "shot/runtime/ios.ts",
  "shot/runtime/shared.ts",
  "shot/verify.ts",
  "legal/LICENSE",
  "factory/cli/package.json",
  "factory/cli/src/release.ts",
  "factory/cli/src/protocol-state.ts",
] as const;
