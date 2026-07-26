import { describe, expect, test } from "bun:test";
import { deriveDeterministicTestIdentity } from "../../identity/src/index.ts";
import {
  LocalEd25519Signer,
  localEd25519VerifierSet,
  type Signer,
} from "../../signer/src/index.ts";
import {
  canonicalJson,
  hashSignedPublicShotRecord,
  PUBLIC_SHOT_PROTOCOL_VERSION,
  type AppcoinLinkedRecord,
  type EvolutionRecordedRecord,
  type LifecycleTransitionedRecord,
  type PublicationEvidence,
  type Sha256Hash,
  type ShotCreatedRecord,
  type SignedPublicShotRecord,
  signPublicShotRecord,
  verifySignedPublicShotRecord,
} from "../../protocol/src/index.ts";
import {
  InMemoryRegistry,
  projectPublicShotProjection,
  RegistryError,
  validatePublicShotProjection,
} from "../src/index.ts";

const SHOT_ID = `shot_${"R".repeat(32)}` as const;
const BAD_HASH = `sha256:${"0".repeat(64)}` as Sha256Hash;
const PUBLICATION: PublicationEvidence = {
  source: {
    url: "https://code.example/public-shot",
    revision: "public-revision-1",
  },
  download: {
    url: "https://download.example/public-shot.zip",
    artifactDigest: `sha256:${"1".repeat(64)}`,
    manifestDigest: `sha256:${"2".repeat(64)}`,
  },
};

function builder(seed: string): LocalEd25519Signer {
  return LocalEd25519Signer.deterministicForTests(
    deriveDeterministicTestIdentity("BUILDER", seed),
    seed,
  );
}

function timestamp(sequence: number): string {
  return new Date(Date.UTC(2026, 6, 25, 0, sequence, 0)).toISOString();
}

async function create(
  signer: Signer,
  summary = "A public registry test Shot.",
  shotId: ShotCreatedRecord["shotId"] = SHOT_ID,
): Promise<SignedPublicShotRecord> {
  const record: ShotCreatedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "SHOT_CREATED",
    shotId,
    sequence: 0,
    previousRecordHash: null,
    recordedAt: timestamp(0),
    authority: signer.verificationMethod,
    body: {
      name: "Registry Shot",
      summary,
      platform: "IOS",
      builder: signer.verificationMethod.identity,
      lifecycle: "EVOLVING",
      evolution: 0,
    },
  };
  return signPublicShotRecord(record, signer);
}

async function evolve(
  signer: Signer,
  previous: SignedPublicShotRecord,
  evolution: number,
  sequence = previous.sequence + 1,
  recordedAt = timestamp(sequence),
): Promise<SignedPublicShotRecord> {
  const record: EvolutionRecordedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "EVOLUTION_RECORDED",
    shotId: previous.shotId,
    sequence,
    previousRecordHash: hashSignedPublicShotRecord(previous),
    recordedAt,
    authority: signer.verificationMethod,
    body: {
      evolution,
      title: `Evolution ${evolution}`,
      summary: `Public result for evolution ${evolution}.`,
    },
  };
  return signPublicShotRecord(record, signer);
}

async function publish(
  signer: Signer,
  previous: SignedPublicShotRecord,
): Promise<SignedPublicShotRecord> {
  const sequence = previous.sequence + 1;
  const record: LifecycleTransitionedRecord = {
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
      evidence: PUBLICATION,
    },
  };
  return signPublicShotRecord(record, signer);
}

async function appStore(
  signer: Signer,
  previous: SignedPublicShotRecord,
): Promise<SignedPublicShotRecord> {
  const sequence = previous.sequence + 1;
  const record: LifecycleTransitionedRecord = {
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
        listingUrl: "https://apps.apple.com/us/app/registry/id1234567890",
      },
    },
  };
  return signPublicShotRecord(record, signer);
}

async function linkAppcoin(
  signer: Signer,
  previous: SignedPublicShotRecord,
  receipt = "receipt-1",
): Promise<SignedPublicShotRecord> {
  const sequence = previous.sequence + 1;
  const record: AppcoinLinkedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "APPCOIN_LINKED",
    shotId: previous.shotId,
    sequence,
    previousRecordHash: hashSignedPublicShotRecord(previous),
    recordedAt: timestamp(sequence),
    authority: signer.verificationMethod,
    body: {
      link: {
        deployment: { namespace: "generic-deployment", id: "deployment-1" },
        network: { namespace: "generic-network", id: "network-1" },
        asset: { namespace: "generic-asset", id: "asset-1" },
        evidence: {
          namespace: "generic-evidence",
          id: receipt,
          url: `https://evidence.example/${receipt}`,
        },
      },
    },
  };
  return signPublicShotRecord(record, signer);
}

describe("deterministic append-only registry", () => {
  test("projects same-Shot evolutions through APP_STORE and generic links", async () => {
    const signer = builder("complete-chain");
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    const records: SignedPublicShotRecord[] = [];
    records.push(await create(signer));
    records.push(await evolve(signer, records[0]!, 1));
    records.push(await publish(signer, records[1]!));
    records.push(await evolve(signer, records[2]!, 2));
    records.push(await appStore(signer, records[3]!));
    records.push(await evolve(signer, records[4]!, 3));
    records.push(await linkAppcoin(signer, records[5]!));

    for (const record of records) {
      expect((await registry.append(record)).status).toBe("appended");
    }
    const projection = registry.getProjection(SHOT_ID);
    expect(projection).toMatchObject({
      shotId: SHOT_ID,
      lifecycle: "APP_STORE",
      evolution: 3,
      recordCount: 7,
      summary: "Public result for evolution 3.",
      createdAt: timestamp(0),
      updatedAt: timestamp(6),
    });
    expect(projection?.appcoins).toHaveLength(1);
    expect((await registry.append(records[6]!)).status).toBe("existing");

    const exported = registry.getRecords(SHOT_ID);
    const verified = await Promise.all(
      exported.map((record) =>
        verifySignedPublicShotRecord(record, localEd25519VerifierSet())
      ),
    );
    expect(canonicalJson(projectPublicShotProjection(verified))).toBe(
      canonicalJson(projection),
    );

    const rebuilt = new InMemoryRegistry(localEd25519VerifierSet());
    for (const record of exported) await rebuilt.append(record);
    expect(canonicalJson(rebuilt.getProjection(SHOT_ID))).toBe(
      canonicalJson(projection),
    );
  });

  test("rejects forks, gaps, wrong links, authority changes, and evolution gaps", async () => {
    const signer = builder("invariants");
    const otherSigner = builder("other-authority");
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    const genesis = await create(signer);
    await registry.append(genesis);
    const before = canonicalJson(registry.getProjection(SHOT_ID));

    const alternateGenesis = await create(signer, "A conflicting genesis.");
    await expect(registry.append(alternateGenesis)).rejects.toMatchObject({
      code: "sequence-conflict",
    });

    const gap = await evolve(signer, genesis, 1, 2);
    await expect(registry.append(gap)).rejects.toMatchObject({
      code: "sequence-gap",
    });

    const wrongPreviousUnsigned: EvolutionRecordedRecord = {
      protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
      kind: "EVOLUTION_RECORDED",
      shotId: SHOT_ID,
      sequence: 1,
      previousRecordHash: BAD_HASH,
      recordedAt: timestamp(1),
      authority: signer.verificationMethod,
      body: {
        evolution: 1,
        title: "Wrong link",
        summary: "This record points at the wrong predecessor.",
      },
    };
    const wrongPrevious = await signPublicShotRecord(
      wrongPreviousUnsigned,
      signer,
    );
    await expect(registry.append(wrongPrevious)).rejects.toMatchObject({
      code: "previous-hash-mismatch",
    });

    const changedAuthority = await evolve(otherSigner, genesis, 1);
    await expect(registry.append(changedAuthority)).rejects.toMatchObject({
      code: "authority-change",
    });

    const evolutionGap = await evolve(signer, genesis, 2);
    await expect(registry.append(evolutionGap)).rejects.toMatchObject({
      code: "evolution-conflict",
    });

    expect(canonicalJson(registry.getProjection(SHOT_ID))).toBe(before);
    expect(registry.getRecords(SHOT_ID)).toHaveLength(1);
  });

  test("rejects out-of-order histories instead of normalizing them", async () => {
    const signer = builder("unordered-history");
    const genesis = await create(signer);
    const evolution = await evolve(signer, genesis, 1);
    const verified = await Promise.all(
      [genesis, evolution].map((record) =>
        verifySignedPublicShotRecord(record, localEd25519VerifierSet())
      ),
    );

    expect(() =>
      projectPublicShotProjection([
        verified[1]!,
        verified[0]!,
      ])
    ).toThrow(
      expect.objectContaining({
        code: "sequence-gap",
      }),
    );
  });

  test("rejects decreasing record times and invalid projection time bounds", async () => {
    const signer = builder("record-times");
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    const genesis = await create(signer);
    await registry.append(genesis);

    const sameTime = await evolve(
      signer,
      genesis,
      1,
      1,
      timestamp(0),
    );
    await registry.append(sameTime);
    const before = registry.getProjection(SHOT_ID);
    expect(before).toMatchObject({
      createdAt: timestamp(0),
      updatedAt: timestamp(0),
    });

    const backdated = await evolve(
      signer,
      sameTime,
      2,
      2,
      timestamp(-1),
    );
    await expect(registry.append(backdated)).rejects.toMatchObject({
      code: "timestamp-conflict",
    });
    expect(registry.getRecords(SHOT_ID)).toHaveLength(2);
    expect(canonicalJson(registry.getProjection(SHOT_ID))).toBe(
      canonicalJson(before),
    );

    const reversedBounds = {
      ...before!,
      updatedAt: timestamp(-1),
    };
    expect(() => validatePublicShotProjection(reversedBounds)).toThrow(
      expect.objectContaining({ code: "timestamp-conflict" }),
    );
  });

  test("allows separate registries to seed the same Shot ID from independent genesis attestations", async () => {
    const leftSigner = builder("independent-genesis-left");
    const rightSigner = builder("independent-genesis-right");
    const leftGenesis = await create(
      leftSigner,
      "The left Builder's public genesis claim.",
    );
    const rightGenesis = await create(
      rightSigner,
      "The right Builder's public genesis claim.",
    );
    const leftRegistry = new InMemoryRegistry(localEd25519VerifierSet());
    const rightRegistry = new InMemoryRegistry(localEd25519VerifierSet());

    expect(hashSignedPublicShotRecord(leftGenesis)).not.toBe(
      hashSignedPublicShotRecord(rightGenesis),
    );
    expect(leftGenesis.authority).not.toEqual(rightGenesis.authority);
    expect((await leftRegistry.append(leftGenesis)).status).toBe("appended");
    expect((await rightRegistry.append(rightGenesis)).status).toBe("appended");

    expect(leftRegistry.getProjection(SHOT_ID)).toMatchObject({
      shotId: SHOT_ID,
      summary: "The left Builder's public genesis claim.",
      authority: leftSigner.verificationMethod,
    });
    expect(rightRegistry.getProjection(SHOT_ID)).toMatchObject({
      shotId: SHOT_ID,
      summary: "The right Builder's public genesis claim.",
      authority: rightSigner.verificationMethod,
    });
  });

  test("enforces monotonic lifecycle transitions", async () => {
    const signer = builder("lifecycle");
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    const genesis = await create(signer);
    await registry.append(genesis);

    const skippedUnsigned: LifecycleTransitionedRecord = {
      protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
      kind: "LIFECYCLE_TRANSITIONED",
      shotId: SHOT_ID,
      sequence: 1,
      previousRecordHash: hashSignedPublicShotRecord(genesis),
      recordedAt: timestamp(1),
      authority: signer.verificationMethod,
      body: {
        from: "PUBLISHED",
        to: "APP_STORE",
        evidence: {
          listingId: "1234567890",
          listingUrl:
            "https://apps.apple.com/us/app/registry/id1234567890",
        },
      },
    };
    await expect(
      registry.append(await signPublicShotRecord(skippedUnsigned, signer)),
    ).rejects.toMatchObject({ code: "lifecycle-conflict" });

    const published = await publish(signer, genesis);
    await registry.append(published);
    const inStore = await appStore(signer, published);
    await registry.append(inStore);
    const repeated = await appStore(signer, inStore);
    await expect(registry.append(repeated)).rejects.toMatchObject({
      code: "lifecycle-conflict",
    });
  });

  test("rejects a second link for the same external asset", async () => {
    const signer = builder("duplicate-asset");
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    const genesis = await create(signer);
    await registry.append(genesis);
    const first = await linkAppcoin(signer, genesis, "receipt-1");
    await registry.append(first);
    const second = await linkAppcoin(signer, first, "receipt-2");
    await expect(registry.append(second)).rejects.toBeInstanceOf(RegistryError);
    await expect(registry.append(second)).rejects.toMatchObject({
      code: "appcoin-conflict",
    });
  });

  test("returns defensive projections and deterministic projection ordering", async () => {
    const signer = builder("defensive");
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    await registry.append(await create(signer));
    const uppercaseId = `shot_I${"A".repeat(31)}` as const;
    const lowercaseId = `shot_i${"A".repeat(31)}` as const;
    await registry.append(
      await create(signer, "Uppercase ordering fixture.", uppercaseId),
    );
    await registry.append(
      await create(signer, "Lowercase ordering fixture.", lowercaseId),
    );
    const first = registry.getProjection(SHOT_ID);
    if (first === undefined) throw new Error("missing projection");
    first.summary = "caller mutation";
    expect(registry.getProjection(SHOT_ID)?.summary).toBe(
      "A public registry test Shot.",
    );
    expect(registry.listProjections().map((projection) => projection.shotId)).toEqual(
      [uppercaseId, SHOT_ID, lowercaseId],
    );
  });
});
