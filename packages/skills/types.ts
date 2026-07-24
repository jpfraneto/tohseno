export const APP_SKILL_SCHEMA_VERSION = 1 as const;
export const APP_TEMPLATE_SCHEMA_VERSION = 1 as const;
export const APP_SKILLS_LOCK_SCHEMA_VERSION = 1 as const;

export type AppSkillMaturity = "experimental" | "stable";
export type AppSkillCategory =
  | "experience"
  | "data"
  | "progression"
  | "sharing";

export interface AcceptanceFileCheck {
  id: string;
  type: "file";
  path: string;
}

export interface AppSkillDescriptor {
  schemaVersion: typeof APP_SKILL_SCHEMA_VERSION;
  id: string;
  version: string;
  title: string;
  summary: string;
  category: AppSkillCategory;
  maturity: AppSkillMaturity;
  platforms: ["ios"];
  requires: string[];
  conflicts: string[];
  contributions: {
    overlay: string;
    manifestFragments: string[];
    entitlements: string[];
    privacy: string[];
    xcodeSourcePaths: string[];
    replaces?: string[];
  };
  acceptanceChecks: AcceptanceFileCheck[];
  agentInstructions: string[];
}

export interface AppTemplateDescriptor {
  schemaVersion: typeof APP_TEMPLATE_SCHEMA_VERSION;
  id: string;
  version: string;
  title: string;
  summary: string;
  platforms: ["ios"];
  kernel: string;
  skills: string[];
  overlay?: string;
  replaces?: string[];
  agentInstructions: string[];
  definitionOfDone: string[];
}

export interface KernelDescriptor {
  schemaVersion: 1;
  id: string;
  version: string;
  title: string;
  platforms: ["ios"];
}

export interface CatalogSkill {
  directory: string;
  descriptorPath: string;
  descriptor: AppSkillDescriptor;
  digest: string;
}

export interface CatalogTemplate {
  directory: string;
  descriptorPath: string;
  descriptor: AppTemplateDescriptor;
  digest: string;
}

export interface CatalogKernel {
  directory: string;
  descriptorPath: string;
  descriptor: KernelDescriptor;
  digest: string;
}

export interface AppCatalog {
  root: string;
  skills: ReadonlyMap<string, CatalogSkill>;
  templates: ReadonlyMap<string, CatalogTemplate>;
  kernels: ReadonlyMap<string, CatalogKernel>;
}

export interface DeclaredComposition {
  schemaVersion: 1;
  template: string;
  skills: string[];
}

export interface ResolvedComposition {
  template: CatalogTemplate;
  kernel: CatalogKernel;
  skills: CatalogSkill[];
}

export interface AppSkillsLock {
  schemaVersion: typeof APP_SKILLS_LOCK_SCHEMA_VERSION;
  factoryReleaseId: string;
  kernel: {
    id: string;
    version: string;
    digest: string;
  };
  template: {
    id: string;
    version: string;
    digest: string;
  };
  skills: Array<{
    id: string;
    version: string;
    digest: string;
  }>;
  resolvedOrder: string[];
  files: AppliedFile[];
}

export interface AppliedFile {
  path: string;
  owner: string;
  sha256: string;
}

export interface AppliedComposition {
  lock: AppSkillsLock;
  files: AppliedFile[];
}
