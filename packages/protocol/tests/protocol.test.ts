import { describe, expect, test } from "bun:test";
import { deriveDeterministicTestIdentity } from "../../identity/src/index.ts";
import {
  LocalEd25519Signer,
  localEd25519VerifierSet,
  type Signer,
} from "../../signer/src/index.ts";
import {
  canonicalBytes,
  canonicalJson,
  createShotId,
  deriveDeterministicTestShotId,
  hashSignedPublicShotRecord,
  isShotId,
  PUBLIC_SHOT_PROTOCOL_VERSION,
  type AppcoinLinkedRecord,
  type AppStoreEvidence,
  type EvolutionRecordedRecord,
  type LifecycleTransitionedRecord,
  type PublicationEvidence,
  type Sha256Hash,
  type ShotCreatedRecord,
  type SignedPublicShotRecord,
  signPublicShotRecord,
  validateCanonicalTimestamp,
  validatePublicShotRecord,
  validateSignedPublicShotRecord,
  verifySignedPublicShotRecord,
} from "../src/index.ts";

const SHOT_ID = `shot_${"A".repeat(32)}` as const;
const DIGEST_A = `sha256:${"a".repeat(64)}` as Sha256Hash;
const DIGEST_B = `sha256:${"b".repeat(64)}` as Sha256Hash;

const PUBLICATION: PublicationEvidence = {
  source: {
    url: "https://code.example/shots/one",
    revision: "0123456789abcdef",
  },
  download: {
    url: "https://downloads.example/shots/one.zip",
    artifactDigest: DIGEST_A,
    manifestDigest: DIGEST_B,
  },
};

const APP_STORE: AppStoreEvidence = {
  listingId: "1234567890",
  listingUrl: "https://apps.apple.com/us/app/example/id1234567890",
};

function testSigner(seed: string): LocalEd25519Signer {
  return LocalEd25519Signer.deterministicForTests(
    deriveDeterministicTestIdentity("BUILDER", seed),
    seed,
  );
}

function genesis(signer: Signer): ShotCreatedRecord {
  return {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "SHOT_CREATED",
    shotId: SHOT_ID,
    sequence: 0,
    previousRecordHash: null,
    recordedAt: "2026-07-25T00:00:00.000Z",
    authority: signer.verificationMethod,
    body: {
      name: "One Shot",
      summary: "A deliberately public summary.",
      platform: "IOS",
      builder: signer.verificationMethod.identity,
      lifecycle: "EVOLVING",
      evolution: 0,
    },
  };
}

describe("canonical public records", () => {
  test("sorts object keys while preserving array and exact Unicode input", () => {
    expect(canonicalJson({ z: [2, 1], a: "value" })).toBe(
      "{\"a\":\"value\",\"z\":[2,1]}",
    );
    expect(canonicalBytes({ text: "\u00e9" })).not.toEqual(
      canonicalBytes({ text: "e\u0301" }),
    );
    expect(() => canonicalJson({ text: "\ud800" })).toThrow("lone surrogate");
    expect(() => canonicalJson([, 1])).toThrow("sparse");
    const accessorArray: unknown[] = [];
    Object.defineProperty(accessorArray, "0", {
      enumerable: true,
      get: () => 1,
    });
    expect(() => canonicalJson(accessorArray)).toThrow("data property");
    const extraPropertyArray: unknown[] = [];
    Object.defineProperty(extraPropertyArray, "4294967295", {
      enumerable: true,
      value: "must not be silently omitted",
    });
    expect(() => canonicalJson(extraPropertyArray)).toThrow(
      "non-JSON array properties",
    );
  });

  test("gives insertion-order-independent hashes", async () => {
    const signer = testSigner("canonical-hash");
    const signed = await signPublicShotRecord(genesis(signer), signer);
    const reordered = {
      signature: signed.signature,
      body: signed.body,
      authority: signed.authority,
      recordedAt: signed.recordedAt,
      previousRecordHash: signed.previousRecordHash,
      sequence: signed.sequence,
      shotId: signed.shotId,
      kind: signed.kind,
      protocolVersion: signed.protocolVersion,
    } as SignedPublicShotRecord;
    expect(canonicalJson(reordered)).toBe(canonicalJson(signed));
    expect(hashSignedPublicShotRecord(reordered)).toBe(
      hashSignedPublicShotRecord(signed),
    );
  });
});

describe("closed signed Shot records", () => {
  test("keeps a public, independently verifiable signed fixture", async () => {
    const fixture = await Bun.file(
      `${import.meta.dir}/../fixtures/signed-shot-created.json`,
    ).json();
    const verified = await verifySignedPublicShotRecord(
      fixture,
      localEd25519VerifierSet(),
    );
    expect(verified).toMatchObject({
      protocolVersion: 1,
      kind: "SHOT_CREATED",
    });
    expect(isShotId(verified.shotId)).toBe(true);
  });

  test("signs, verifies, and rejects a mutation", async () => {
    const signer = testSigner("mutation");
    const signed = await signPublicShotRecord(genesis(signer), signer);
    const verified = await verifySignedPublicShotRecord(
      signed,
      localEd25519VerifierSet(),
    );
    expect(canonicalJson(verified)).toBe(canonicalJson(signed));

    const changed = structuredClone(signed);
    if (changed.kind !== "SHOT_CREATED") throw new Error("unreachable");
    changed.body.summary = "A changed public summary.";
    await expect(
      verifySignedPublicShotRecord(changed, localEd25519VerifierSet()),
    ).rejects.toMatchObject({ code: "invalid-signature" });
  });

  test("rejects private-shaped and unknown fields at every closed level", async () => {
    const signer = testSigner("closed");
    const signed = await signPublicShotRecord(genesis(signer), signer);
    expect(() =>
      validateSignedPublicShotRecord({
        ...signed,
        privatePrompt: "must never be accepted",
      })
    ).toThrow("is not allowed");
    expect(() =>
      validateSignedPublicShotRecord({
        ...signed,
        body: { ...signed.body, sourceBytes: "not public protocol data" },
      })
    ).toThrow("is not allowed");
  });

  test("requires exact timestamps and independent random Shot IDs", () => {
    expect(() =>
      validateCanonicalTimestamp("2026-02-30T00:00:00.000Z")
    ).toThrow("exact RFC 3339");
    expect(() =>
      validateCanonicalTimestamp("2026-07-25T00:00:00Z")
    ).toThrow("exact RFC 3339");
    const first = createShotId();
    const second = createShotId();
    expect(isShotId(first)).toBe(true);
    expect(first).not.toBe(second);
    expect(deriveDeterministicTestShotId("fixture")).toBe(
      deriveDeterministicTestShotId("fixture"),
    );
    expect(deriveDeterministicTestShotId("fixture")).not.toBe(
      deriveDeterministicTestShotId("other-fixture"),
    );
  });

  test("requires both source and TOHSENO download evidence for PUBLISHED", () => {
    const signer = testSigner("published-evidence");
    const record: LifecycleTransitionedRecord = {
      protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
      kind: "LIFECYCLE_TRANSITIONED",
      shotId: SHOT_ID,
      sequence: 1,
      previousRecordHash: DIGEST_A,
      recordedAt: "2026-07-25T00:01:00.000Z",
      authority: signer.verificationMethod,
      body: {
        from: "EVOLVING",
        to: "PUBLISHED",
        evidence: PUBLICATION,
      },
    };
    expect(validatePublicShotRecord(record)).toEqual(record);
    expect(() =>
      validatePublicShotRecord({
        ...record,
        body: {
          from: "EVOLVING",
          to: "PUBLISHED",
          evidence: { source: PUBLICATION.source },
        },
      })
    ).toThrow("download");
  });

  test("accepts only matching Apple listing evidence", () => {
    const signer = testSigner("app-store-evidence");
    const record: LifecycleTransitionedRecord = {
      protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
      kind: "LIFECYCLE_TRANSITIONED",
      shotId: SHOT_ID,
      sequence: 2,
      previousRecordHash: DIGEST_A,
      recordedAt: "2026-07-25T00:02:00.000Z",
      authority: signer.verificationMethod,
      body: {
        from: "PUBLISHED",
        to: "APP_STORE",
        evidence: APP_STORE,
      },
    };
    expect(validatePublicShotRecord(record)).toEqual(record);
    expect(() =>
      validatePublicShotRecord({
        ...record,
        body: {
          ...record.body,
          evidence: { ...APP_STORE, listingId: "123456789" },
        },
      })
    ).toThrow("matching");
  });

  test("keeps evolutions on the same Shot and appcoin evidence generic", () => {
    const signer = testSigner("generic-link");
    const evolution: EvolutionRecordedRecord = {
      protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
      kind: "EVOLUTION_RECORDED",
      shotId: SHOT_ID,
      sequence: 1,
      previousRecordHash: DIGEST_A,
      recordedAt: "2026-07-25T00:01:00.000Z",
      authority: signer.verificationMethod,
      body: {
        evolution: 1,
        title: "A second pass",
        summary: "The same Shot after one public evolution.",
      },
    };
    const appcoin: AppcoinLinkedRecord = {
      protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
      kind: "APPCOIN_LINKED",
      shotId: evolution.shotId,
      sequence: 2,
      previousRecordHash: DIGEST_B,
      recordedAt: "2026-07-25T00:02:00.000Z",
      authority: signer.verificationMethod,
      body: {
        link: {
          deployment: { namespace: "external-system", id: "deployment-7" },
          network: { namespace: "network-family", id: "network-42" },
          asset: { namespace: "asset-standard", id: "asset-public-id" },
          evidence: {
            namespace: "external-receipt",
            id: "receipt-9",
            url: "https://evidence.example/receipt-9",
          },
        },
      },
    };
    expect(validatePublicShotRecord(evolution).shotId).toBe(SHOT_ID);
    expect(() =>
      validatePublicShotRecord({
        ...evolution,
        body: {
          ...evolution.body,
          publication: PUBLICATION,
        },
      })
    ).toThrow("is not allowed");
    expect(validatePublicShotRecord(appcoin)).toEqual(appcoin);
  });
});
