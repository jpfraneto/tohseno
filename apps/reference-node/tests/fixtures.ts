import { deriveDeterministicTestIdentity } from "../../../packages/identity/src/index.ts";
import {
  LocalEd25519Signer,
  type Signer,
} from "../../../packages/signer/src/index.ts";
import {
  hashSignedPublicShotRecord,
  PUBLIC_SHOT_PROTOCOL_VERSION,
  type AppcoinLinkedRecord,
  type EvolutionRecordedRecord,
  type LifecycleTransitionedRecord,
  type PublicationEvidence,
  type ShotCreatedRecord,
  type ShotId,
  type SignedPublicShotRecord,
  signPublicShotRecord,
} from "../../../packages/protocol/src/index.ts";

export const TEST_SHOT_ID = `shot_${"N".repeat(32)}` as ShotId;
export const TEST_PUBLICATION: PublicationEvidence = {
  source: {
    url: "https://source.example/public-shot",
    revision: "public-revision-1",
  },
  download: {
    url: "https://download.example/public-shot.zip",
    artifactDigest: `sha256:${"a".repeat(64)}`,
    manifestDigest: `sha256:${"b".repeat(64)}`,
  },
};

export function testBuilder(seed: string): LocalEd25519Signer {
  return LocalEd25519Signer.deterministicForTests(
    deriveDeterministicTestIdentity("BUILDER", seed),
    seed,
  );
}

function timestamp(sequence: number): string {
  return new Date(Date.UTC(2026, 6, 25, 0, sequence, 0)).toISOString();
}

export async function createRecord(
  signer: Signer,
  shotId: ShotId = TEST_SHOT_ID,
  summary = "A deliberately public reference-node test Shot.",
): Promise<SignedPublicShotRecord> {
  const value: ShotCreatedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "SHOT_CREATED",
    shotId,
    sequence: 0,
    previousRecordHash: null,
    recordedAt: timestamp(0),
    authority: signer.verificationMethod,
    body: {
      name: "Reference Shot",
      summary,
      platform: "IOS",
      builder: signer.verificationMethod.identity,
      lifecycle: "EVOLVING",
      evolution: 0,
    },
  };
  return signPublicShotRecord(value, signer);
}

export async function evolutionRecord(
  signer: Signer,
  previous: SignedPublicShotRecord,
  evolution = 1,
): Promise<SignedPublicShotRecord> {
  const sequence = previous.sequence + 1;
  const value: EvolutionRecordedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "EVOLUTION_RECORDED",
    shotId: previous.shotId,
    sequence,
    previousRecordHash: hashSignedPublicShotRecord(previous),
    recordedAt: timestamp(sequence),
    authority: signer.verificationMethod,
    body: {
      evolution,
      title: `Evolution ${evolution}`,
      summary: `Public evolution ${evolution}.`,
    },
  };
  return signPublicShotRecord(value, signer);
}

export async function publishedRecord(
  signer: Signer,
  previous: SignedPublicShotRecord,
): Promise<SignedPublicShotRecord> {
  const sequence = previous.sequence + 1;
  const value: LifecycleTransitionedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "LIFECYCLE_TRANSITIONED",
    shotId: previous.shotId,
    sequence,
    previousRecordHash: hashSignedPublicShotRecord(previous),
    recordedAt: timestamp(sequence),
    authority: signer.verificationMethod,
    body: {
      from: "EVOLVING",
      to: "PUBLISHED",
      evidence: TEST_PUBLICATION,
    },
  };
  return signPublicShotRecord(value, signer);
}

export async function appStoreRecord(
  signer: Signer,
  previous: SignedPublicShotRecord,
): Promise<SignedPublicShotRecord> {
  const sequence = previous.sequence + 1;
  const value: LifecycleTransitionedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "LIFECYCLE_TRANSITIONED",
    shotId: previous.shotId,
    sequence,
    previousRecordHash: hashSignedPublicShotRecord(previous),
    recordedAt: timestamp(sequence),
    authority: signer.verificationMethod,
    body: {
      from: "PUBLISHED",
      to: "APP_STORE",
      evidence: {
        listingId: "1234567890",
        listingUrl:
          "https://apps.apple.com/us/app/reference-shot/id1234567890",
      },
    },
  };
  return signPublicShotRecord(value, signer);
}

export async function appcoinRecord(
  signer: Signer,
  previous: SignedPublicShotRecord,
): Promise<SignedPublicShotRecord> {
  const sequence = previous.sequence + 1;
  const value: AppcoinLinkedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "APPCOIN_LINKED",
    shotId: previous.shotId,
    sequence,
    previousRecordHash: hashSignedPublicShotRecord(previous),
    recordedAt: timestamp(sequence),
    authority: signer.verificationMethod,
    body: {
      link: {
        deployment: { namespace: "deployment", id: "public-deployment-1" },
        network: { namespace: "network", id: "public-network-1" },
        asset: { namespace: "asset", id: "public-asset-1" },
        evidence: {
          namespace: "receipt",
          id: "public-receipt-1",
          url: "https://evidence.example/public-receipt-1",
        },
      },
    },
  };
  return signPublicShotRecord(value, signer);
}

export async function completeRecordChain(
  signer: Signer = testBuilder("complete-chain"),
): Promise<SignedPublicShotRecord[]> {
  const records: SignedPublicShotRecord[] = [];
  records.push(await createRecord(signer));
  records.push(await evolutionRecord(signer, records[0]!));
  records.push(await publishedRecord(signer, records[1]!));
  records.push(await appStoreRecord(signer, records[2]!));
  records.push(await appcoinRecord(signer, records[3]!));
  return records;
}
