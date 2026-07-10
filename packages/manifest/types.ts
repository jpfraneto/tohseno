/**
 * Public TypeScript representation of continuity.manifest.json version 0.1.0.
 *
 * The JSON Schema is the language-neutral source of truth. These types make the
 * same boundary convenient to consume from strict TypeScript without turning
 * the manifest into a UI builder or deployment configuration language.
 */

export const CONTINUITY_MANIFEST_SCHEMA_VERSION = "0.1.0" as const;

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
  targetHuman: string;
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
  publicByDefault: false;
  controlPlaneReceives: Array<
    | "none"
    | "release-version"
    | "deployment-health"
    | "migration-health"
    | "opaque-application-id"
    | "pseudonymous-request-authorization"
    | "minimal-payment-and-order-state"
  >;
  externalDisclosure: string[];
  telemetry: "none" | "operational-metadata-only" | "minimal-pseudonymous";
}

export interface PracticeIdentityPolicy {
  mode: "none" | "local-contextual";
  creation: "first-action" | "first-committed-event" | "first-launch";
  suite?: string;
  crossAppLinking: "never" | "explicit-consent-only";
}

export interface RecoveryPolicy {
  offer: "after-first-value" | "settings-only" | "never";
  identity: "none" | "manual" | "opt-in-encrypted-backup";
  content: "none" | "manual-export" | "opt-in-encrypted-backup";
}

export interface ProofPolicy {
  mode: "practice-key-attestation" | "server-witnessed";
  statement: string;
  disclosure: "minimal";
  artifactDisclosure: "none" | "digest";
  export: "owner-selected";
}

export interface SynchronizationPolicy {
  mode: "opt-in-encrypted";
  conflictPolicy: string;
}

export interface PaymentPolicy {
  mode: "optional-entitlement";
  mayGate: string[];
  mustNotGate: string[];
}

/** Properties an implementation must enforce rather than merely describe. */
export interface RuntimeProperties {
  offlineCoreAction: true;
  actionBeforeAccount: true;
  localFirstRecord: true;
  continuityWithoutAi: true;
  stableEventIdentity: true;
  immutableSealedArtifacts: true;
}

export interface ManifestRuntime {
  properties: RuntimeProperties;
  action: ActionRuntime;
  feedback: RuntimeFeedback;
  returnInvitation: string;
  continuity: {
    accumulated: AccumulatedContinuity;
    artifacts: ArtifactPolicy[];
  };
  reflection?: ReflectionPolicy;
  privacy: PrivacyBoundary;
  identity: PracticeIdentityPolicy;
  recovery: RecoveryPolicy;
  proofs?: ProofPolicy[];
  synchronization?: SynchronizationPolicy;
  payments?: PaymentPolicy;
}

/** Guidance constrains coding-agent judgment; a runtime must not interpret it. */
export interface CodingAgentGuidance {
  forbiddenPatterns: string[];
  visualDirection: string[];
  tonalDirection: string[];
  implementationNotes?: string[];
}

export type OperatingMode =
  | "self-hosted"
  | "client-owned"
  | "anky-operated";

export type DeploymentTarget =
  | "native-ios"
  | "native-android"
  | "web"
  | "server";

export type ApprovalBoundary =
  | "paid-infrastructure"
  | "dns-change"
  | "store-submission"
  | "production-credential-rotation";

/** Operator/deployment metadata is explicit and does not alter ritual semantics. */
export interface OperatorMetadata {
  operatingMode: OperatingMode;
  deploymentTargets: DeploymentTarget[];
  requiresServer: boolean;
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
