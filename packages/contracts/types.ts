/**
 * TypeScript mirrors of the language-neutral JSON contracts in ./schemas.
 * These contracts are experimental 0.1 characterization boundaries, not a
 * generic production identity, proof, storage, or synchronization package.
 */

export const CONTINUITY_CONTRACT_SCHEMA_VERSION = "0.1.0" as const;

export interface ContractVersion {
  id: string;
  version: string;
}

export interface ArtifactReference {
  artifactId: string;
  relation: "primary" | "attachment" | "derived";
}

export interface ContinuityEvent {
  schemaVersion: typeof CONTINUITY_CONTRACT_SCHEMA_VERSION;
  eventId: string;
  applicationId: string;
  practiceContextId: string;
  actionPolicy: ContractVersion;
  lifecycle: {
    startedAt: string;
    endedAt: string;
    sealedAt: string;
  };
  completion: {
    state: "completed" | "interrupted";
    conditionId: string;
    reason: string;
  };
  artifactRefs: ArtifactReference[];
  createdAt: string;
}

export interface Sha256Digest {
  algorithm: "sha-256";
  value: string;
}

export type ArtifactContent =
  | {
      kind: "embedded";
      encoding: "base64";
      bytes: string;
      byteLength: number;
    }
  | {
      kind: "reference";
      uri: string;
      byteLength: number;
    };

export interface ContinuityArtifact {
  schemaVersion: typeof CONTINUITY_CONTRACT_SCHEMA_VERSION;
  artifactId: string;
  eventId: string;
  mediaType: string;
  codec: string;
  content: ArtifactContent;
  digest: Sha256Digest;
  createdAt: string;
  sealedAt: string;
  seal: {
    sealedBy: string;
    immutable: true;
  };
}

export interface ContinuityReflection {
  schemaVersion: typeof CONTINUITY_CONTRACT_SCHEMA_VERSION;
  reflectionId: string;
  eventId: string;
  artifactId?: string;
  provider: {
    kind: "local" | "remote";
    id: string;
    policyVersion: string;
    model?: string;
  };
  consent: {
    basis: "not-required-local" | "per-event-opt-in" | "standing-explicit";
    recordedAt: string;
    disclosure: string[];
  };
  generatedAt: string;
  output: unknown;
  deletion: {
    independentlyDeletable: true;
  };
}

export interface ContinuityProof {
  schemaVersion: typeof CONTINUITY_CONTRACT_SCHEMA_VERSION;
  proofId: string;
  eventId: string;
  proofVersion: "1";
  statement: {
    type: "practice-key-attestation" | "server-witness";
    text: string;
    claims: Record<string, string | number | boolean>;
  };
  disclosure: {
    fields: string[];
    artifactContentIncluded: false;
  };
  signer: {
    suite: string;
    keyId: string;
    publicKey: string;
  };
  verification: {
    algorithm: string;
    signature: string;
    material: string;
  };
  generatedAt: string;
}

export interface SignedRequestEnvelopeV1 {
  protocolVersion: "1";
  method: string;
  path: string;
  bodyHash: Sha256Digest;
  timestamp: string;
  nonce: string;
  signer: {
    suite: string;
    keyId: string;
    publicKey: string;
  };
  signature: {
    encoding: "base64url";
    value: string;
  };
}

export type ContractKind =
  | "ContinuityEvent"
  | "ContinuityArtifact"
  | "ContinuityReflection"
  | "ContinuityProof"
  | "SignedRequestEnvelopeV1";

export interface ContractValidationIssue {
  path: string;
  code: string;
  message: string;
}

export interface ContractValidationResult {
  valid: boolean;
  issues: ContractValidationIssue[];
}
