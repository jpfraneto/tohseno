import type {
  IdentityReference,
  VerificationMethod,
} from "../../identity/src/index.ts";
import type { SignatureEnvelope } from "../../signer/src/index.ts";

export const PUBLIC_SHOT_PROTOCOL_VERSION = 1 as const;
export const PUBLIC_SHOT_LIFECYCLES = [
  "EVOLVING",
  "PUBLISHED",
  "APP_STORE",
] as const;
export const PUBLIC_SHOT_RECORD_KINDS = [
  "SHOT_CREATED",
  "EVOLUTION_RECORDED",
  "LIFECYCLE_TRANSITIONED",
  "APPCOIN_LINKED",
] as const;

export type PublicShotLifecycle = (typeof PUBLIC_SHOT_LIFECYCLES)[number];
export type PublicShotRecordKind = (typeof PUBLIC_SHOT_RECORD_KINDS)[number];
export type ShotId = `shot_${string}`;
export type Sha256Hash = `sha256:${string}`;
export type CanonicalTimestamp = string;

export interface PublicSourcePointer {
  url: string;
  revision: string;
}

export interface TohsenoDownloadEvidence {
  url: string;
  artifactDigest: Sha256Hash;
  manifestDigest: Sha256Hash;
}

export interface PublicationEvidence {
  source: PublicSourcePointer;
  download: TohsenoDownloadEvidence;
}

export interface AppStoreEvidence {
  listingId: string;
  listingUrl: string;
}

export interface ExternalIdentifier {
  namespace: string;
  id: string;
}

export interface AppcoinEvidenceIdentifier extends ExternalIdentifier {
  url?: string;
}

export interface AppcoinLink {
  deployment: ExternalIdentifier;
  network: ExternalIdentifier;
  asset: ExternalIdentifier;
  evidence: AppcoinEvidenceIdentifier;
}

export interface ShotCreatedBody {
  name: string;
  summary: string;
  platform: string;
  builder: IdentityReference;
  continuity?: IdentityReference;
  lifecycle: "EVOLVING";
  evolution: 0;
}

export interface EvolutionRecordedBody {
  evolution: number;
  title: string;
  summary: string;
}

export type LifecycleTransitionedBody =
  | {
    from: "EVOLVING";
    to: "PUBLISHED";
    evidence: PublicationEvidence;
  }
  | {
    from: "PUBLISHED";
    to: "APP_STORE";
    evidence: AppStoreEvidence;
  };

export interface AppcoinLinkedBody {
  link: AppcoinLink;
}

export interface PublicShotRecordBase {
  protocolVersion: typeof PUBLIC_SHOT_PROTOCOL_VERSION;
  shotId: ShotId;
  sequence: number;
  previousRecordHash: Sha256Hash | null;
  recordedAt: CanonicalTimestamp;
  authority: VerificationMethod;
}

export interface ShotCreatedRecord extends PublicShotRecordBase {
  kind: "SHOT_CREATED";
  body: ShotCreatedBody;
}

export interface EvolutionRecordedRecord extends PublicShotRecordBase {
  kind: "EVOLUTION_RECORDED";
  body: EvolutionRecordedBody;
}

export interface LifecycleTransitionedRecord extends PublicShotRecordBase {
  kind: "LIFECYCLE_TRANSITIONED";
  body: LifecycleTransitionedBody;
}

export interface AppcoinLinkedRecord extends PublicShotRecordBase {
  kind: "APPCOIN_LINKED";
  body: AppcoinLinkedBody;
}

export type PublicShotRecord =
  | ShotCreatedRecord
  | EvolutionRecordedRecord
  | LifecycleTransitionedRecord
  | AppcoinLinkedRecord;

export type SignedPublicShotRecord = PublicShotRecord & {
  signature: SignatureEnvelope;
};

declare const verifiedRecordBrand: unique symbol;
export type VerifiedSignedPublicShotRecord = SignedPublicShotRecord & {
  readonly [verifiedRecordBrand]: true;
};
