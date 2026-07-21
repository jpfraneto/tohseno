/**
 * Public TypeScript representation of continuity.manifest.json version 0.3.0.
 *
 * The JSON Schema is the language-neutral source of truth. These types make the
 * same boundary convenient to consume from strict TypeScript. The bounded field
 * space is a reliability mechanism: a feature that cannot be expressed here is
 * unsupported, and an agent says so instead of improvising.
 */

export const CONTINUITY_MANIFEST_SCHEMA_VERSION = "0.3.0" as const;

export type ContinuityManifestSchemaVersion =
  typeof CONTINUITY_MANIFEST_SCHEMA_VERSION;

export type CompletionCondition =
  | {
      kind: "active-duration";
      thresholdMs: number;
    }
  | {
      kind: "count";
      threshold: number;
      unit: string;
    }
  | {
      kind: "explicit";
      predicate: string;
    };

export type InterruptionCondition =
  | {
      kind: "inactivity";
      afterMs: number;
    }
  | {
      kind: "lifecycle-exit";
    }
  | {
      kind: "explicit";
      predicate: string;
    };

export interface ApplicationIdentity {
  id: string;
  name: string;
  coreAction: string;
}

export interface ActionInput {
  kind: string;
  mediaTypes: string[];
  constraints?: string[];
}

export interface ActionRuntime {
  input: ActionInput;
  completion: CompletionCondition;
  interruption: {
    condition: InterruptionCondition;
    partialAction: "preserve" | "discard-with-explicit-consent";
    resumption: "resume" | "start-new";
  };
  checkpoint:
    | "on-progress"
    | {
        intervalMs: number;
      };
}

export interface RuntimeFeedback {
  onProgress?: string;
  onCompletion: string;
  onInterruption: string;
}

export interface ArtifactPolicy {
  id: string;
  mediaType: string;
  codec: string;
  export: "owner-selected" | "private-by-default" | "none";
}

export interface AccumulatedContinuity {
  description: string;
  projections: string[];
}

export interface ReflectionPolicy {
  mode: "local-deterministic" | "local-ai" | "remote-service";
  trigger: "after-event-opt-in" | "after-event-automatic";
  eventEligibility: "completed-only" | "completed-or-interrupted";
  consent: "per-event" | "standing-explicit";
  inputDisclosure: "none" | "derived-features" | "private-artifact";
  policyId: string;
  fallback: "continue-without-reflection";
}

export interface PrivacyBoundary {
  localStorage: "platform-private" | "application-encrypted";
  publicByDefault: boolean;
  externalDisclosure: string[];
  telemetry: "none" | "operational-metadata-only" | "minimal-pseudonymous";
}

export interface IdentityPolicy {
  mode: "none" | "seed-phrase" | "local-contextual";
  creation: "first-action" | "first-committed-event" | "first-launch";
  wordlist?: "bip39-english";
  suite?: string;
  crossAppLinking: "never" | "explicit-consent-only";
}

export interface RecoveryPolicy {
  offer: "after-first-value" | "settings-only" | "never";
  identity: "none" | "manual" | "automatic-encrypted-backup" | "opt-in-encrypted-backup";
  content: "none" | "manual-export" | "opt-in-encrypted-backup";
}

export interface SynchronizationPolicy {
  mode: "opt-in-encrypted";
  conflictPolicy: string;
}

/** The flag-gated indie-stack modules. Flipping a flag is the integration step. */
export interface ModulePolicies {
  paywall: {
    enabled: boolean;
    provider: "revenuecat";
    publicKeySlot?: string;
  };
  shareCard: {
    enabled: boolean;
  };
  notifications: {
    enabled: boolean;
  };
  /** Reserved future primitive: QR browser pairing. Enabling it is unsupported in 0.3.0. */
  sessionLink: {
    enabled: false;
    status: "reserved";
  };
  /**
   * Reserved future primitive: a minimal server that mints short-lived
   * third-party credentials and never receives user content. Enabling it is
   * unsupported in 0.3.0; prototypes declare operations.developmentSecrets.
   */
  tokenMint: {
    enabled: false;
    status: "reserved";
  };
}

/**
 * How much of the core action works with no network. `full` is the default the
 * base app ships with; anything else must say what still works offline and
 * carry a non-empty external disclosure inventory.
 */
export type OfflineCoreAction = "full" | "degraded-readonly" | "network-required";

/** Reliability invariants an implementation must enforce rather than merely describe. */
export interface RuntimeProperties {
  offlineCoreAction: OfflineCoreAction;
  /** What a person can still do with no network. Required unless offlineCoreAction is full. */
  offlineSurface?: string;
  noAccountBeforeValue: true;
  localFirstRecord: true;
  crashSafePersistence: true;
  stableEventIdentity: true;
}

export interface ManifestRuntime {
  properties: RuntimeProperties;
  action: ActionRuntime;
  feedback: RuntimeFeedback;
  continuity: {
    accumulated: AccumulatedContinuity;
    artifacts: ArtifactPolicy[];
  };
  reflection?: ReflectionPolicy;
  privacy: PrivacyBoundary;
  identity: IdentityPolicy;
  recovery: RecoveryPolicy;
  synchronization?: SynchronizationPolicy;
  modules: ModulePolicies;
}

/** Guidance constrains coding-agent judgment (look, tone, notes); a runtime must not interpret it. */
export interface CodingAgentGuidance {
  visualDirection: string[];
  tonalDirection: string[];
  implementationNotes?: string[];
}

export type DeploymentTarget =
  | "native-ios"
  | "native-android"
  | "web"
  | "server";

/**
 * Why a server exists. token-mint-only mints short-lived third-party
 * credentials and never receives user content; sync-relay relays
 * application-encrypted blobs it cannot read; app-backend receives user data
 * and must appear in the external disclosure inventory.
 */
export type ServerRole = "token-mint-only" | "sync-relay" | "app-backend";

/** A prototype-only secret living in gitignored local configuration — a slot name and its purpose, never a value. */
export interface DevelopmentSecret {
  slot: string;
  purpose: string;
}

export type ApprovalBoundary =
  | "paid-infrastructure"
  | "dns-change"
  | "store-submission"
  | "production-credential-rotation";

/** Operator/deployment metadata is explicit and cannot weaken runtime guarantees. */
export interface OperatorMetadata {
  deploymentTargets: DeploymentTarget[];
  requiresServer: boolean;
  /** Required when requiresServer is true; omitted when false. */
  serverRole?: ServerRole;
  developmentSecrets?: DevelopmentSecret[];
  ejectionRequired: true;
  approvalRequiredFor: ApprovalBoundary[];
  operatorNotes?: string[];
}

export interface ContinuityManifest {
  schemaVersion: ContinuityManifestSchemaVersion;
  application: ApplicationIdentity;
  runtime: ManifestRuntime;
  guidance: CodingAgentGuidance;
  operations: OperatorMetadata;
}

export type ManifestIssueSeverity = "error" | "warning";

export interface ManifestValidationIssue {
  path: string;
  code: string;
  message: string;
  severity: ManifestIssueSeverity;
}

export interface ManifestValidationResult {
  valid: boolean;
  issues: ManifestValidationIssue[];
  errors: ManifestValidationIssue[];
  warnings: ManifestValidationIssue[];
}
