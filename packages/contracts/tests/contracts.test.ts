import { describe, expect, test } from "bun:test";
import {
  canonicalSignedRequestBytes,
  canonicalSignedRequestText,
  decodeBase64,
  encodeBase64,
  sha256Hex,
  verifyRequestBinding,
} from "../canonical";
import type {
  ContractKind,
  SignedRequestEnvelopeV1,
} from "../types";
import {
  asSignedRequestEnvelopeV1,
  validateContract,
} from "../validate";

const fixtureNames = [
  "records.json",
  "unicode.json",
  "empty-content.json",
  "timing.json",
  "canonical-bytes.json",
  "mutation.json",
  "route-signing.json",
  "request-freshness.json",
] as const;

interface ActualRequestFixture {
  method: string;
  path: string;
  bodyBase64: string;
}

interface ContractFixtureCase {
  name: string;
  contract: ContractKind;
  expectedValid: boolean;
  expectedIssueCode?: string;
  value: unknown;
  exactText?: string;
  exactBytesBase64?: string;
  expectedByteLength?: number;
  expectedSha256?: string;
  mutatedSha256?: string;
  expectedBinding?: boolean;
  expectedMismatches?: Array<"method" | "path" | "bodyHash">;
  actualRequest?: ActualRequestFixture;
  expectedCanonicalText?: string;
  expectedCanonicalBytesBase64?: string;
  expectedCanonicalSha256?: string;
  evaluatedAt?: string;
  maxClockSkewMs?: number;
  expectedFresh?: boolean;
  replaySequence?: number;
  expectedReplay?: boolean;
}

interface ContractFixtureFile {
  fixtureVersion: "1";
  purpose: string;
  cases: ContractFixtureCase[];
}

async function fixture(name: string): Promise<ContractFixtureFile> {
  return Bun.file(new URL(`../fixtures/${name}`, import.meta.url)).json() as Promise<ContractFixtureFile>;
}

async function allCases(): Promise<ContractFixtureCase[]> {
  const files = await Promise.all(fixtureNames.map((name) => fixture(name)));
  return files.flatMap((file) => file.cases);
}

function unsignedEnvelope(
  envelope: SignedRequestEnvelopeV1,
): Omit<SignedRequestEnvelopeV1, "signature"> {
  return {
    protocolVersion: envelope.protocolVersion,
    method: envelope.method,
    path: envelope.path,
    bodyHash: envelope.bodyHash,
    timestamp: envelope.timestamp,
    nonce: envelope.nonce,
    signer: envelope.signer,
  };
}

describe("continuity contract harness", () => {
  test("every language-neutral JSON Schema parses and identifies draft 2020-12", async () => {
    const schemas = [
      "continuity-event.schema.json",
      "continuity-artifact.schema.json",
      "continuity-reflection.schema.json",
      "continuity-proof.schema.json",
      "signed-request-envelope-v1.schema.json",
    ] as const;
    const ids = new Set<string>();
    for (const name of schemas) {
      const value = await Bun.file(
        new URL(`../schemas/${name}`, import.meta.url),
      ).json() as Record<string, unknown>;
      expect(value.$schema).toBe(
        "https://json-schema.org/draft/2020-12/schema",
      );
      expect(typeof value.$id).toBe("string");
      ids.add(value.$id as string);
      expect(value.additionalProperties).toBe(false);
    }
    expect(ids.size).toBe(schemas.length);
  });

  test("all declared fixture outcomes match contract validation", async () => {
    for (const entry of await allCases()) {
      const result = await validateContract(entry.contract, entry.value);
      expect(result.valid, `${entry.name}: ${JSON.stringify(result.issues)}`).toBe(
        entry.expectedValid,
      );
      if (entry.expectedIssueCode !== undefined) {
        expect(
          result.issues.map((candidate) => candidate.code),
          entry.name,
        ).toContain(entry.expectedIssueCode);
      }
    }
  });

  test("Unicode fixtures preserve exact bytes without normalization", async () => {
    const unicode = await fixture("unicode.json");
    const [nfc, nfd] = unicode.cases;
    expect(nfc).toBeDefined();
    expect(nfd).toBeDefined();
    if (nfc === undefined || nfd === undefined) return;

    for (const entry of [nfc, nfd]) {
      expect(entry.exactText).toBeDefined();
      expect(entry.exactBytesBase64).toBeDefined();
      if (
        entry.exactText === undefined ||
        entry.exactBytesBase64 === undefined ||
        entry.expectedByteLength === undefined ||
        entry.expectedSha256 === undefined
      ) continue;
      const bytes = new TextEncoder().encode(entry.exactText);
      expect(encodeBase64(bytes)).toBe(entry.exactBytesBase64);
      expect(bytes.byteLength).toBe(entry.expectedByteLength);
      expect(await sha256Hex(bytes)).toBe(entry.expectedSha256);
    }
    expect(nfc.expectedSha256).not.toBe(nfd.expectedSha256);
    expect(nfc.exactText?.normalize("NFC")).toBe(nfd.exactText?.normalize("NFC"));
  });

  test("empty content has the standard SHA-256 digest and remains distinct from completion", async () => {
    const empty = (await fixture("empty-content.json")).cases[0];
    expect(empty).toBeDefined();
    if (empty === undefined) return;
    const bytes = decodeBase64(empty.exactBytesBase64 ?? "not-base64");
    expect(bytes.byteLength).toBe(0);
    expect(await sha256Hex(bytes)).toBe(
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
    expect(JSON.stringify(empty.value)).not.toContain('"completion"');
  });

  test("canonical artifact bytes are not reconstructed from parsed JSON", async () => {
    const entry = (await fixture("canonical-bytes.json")).cases[0];
    expect(entry).toBeDefined();
    if (
      entry === undefined ||
      entry.exactText === undefined ||
      entry.exactBytesBase64 === undefined ||
      entry.expectedByteLength === undefined ||
      entry.expectedSha256 === undefined
    ) return;
    const exactBytes = decodeBase64(entry.exactBytesBase64);
    expect(new TextDecoder().decode(exactBytes)).toBe(entry.exactText);
    expect(exactBytes.byteLength).toBe(entry.expectedByteLength);
    expect(await sha256Hex(exactBytes)).toBe(entry.expectedSha256);

    const reparsed = JSON.stringify(JSON.parse(entry.exactText) as unknown, null, 2);
    expect(encodeBase64(new TextEncoder().encode(reparsed))).not.toBe(
      entry.exactBytesBase64,
    );
  });

  test("a one-glyph artifact mutation is detected without changing event identity", async () => {
    const mutation = await fixture("mutation.json");
    const original = mutation.cases.find((entry) =>
      entry.name.startsWith("original"),
    );
    const changed = mutation.cases.find((entry) =>
      entry.name.startsWith("one-glyph"),
    );
    expect(original).toBeDefined();
    expect(changed).toBeDefined();
    if (original === undefined || changed === undefined) return;
    const originalObject = original.value as {
      eventId: string;
      digest: { value: string };
    };
    const changedObject = changed.value as {
      eventId: string;
      digest: { value: string };
    };
    expect(changedObject.eventId).toBe(originalObject.eventId);
    expect(changedObject.digest.value).toBe(originalObject.digest.value);
    expect((await validateContract("ContinuityArtifact", changed.value)).valid).toBe(
      false,
    );
    expect(changed.mutatedSha256).not.toBe(originalObject.digest.value);
  });

  test("route-signing fixtures bind actual method, path, and body", async () => {
    const routeCases = (await fixture("route-signing.json")).cases;
    for (const entry of routeCases) {
      if (entry.expectedBinding === undefined) continue;
      expect(entry.actualRequest).toBeDefined();
      if (entry.actualRequest === undefined) continue;
      const envelope = asSignedRequestEnvelopeV1(entry.value);
      const binding = await verifyRequestBinding(
        envelope,
        entry.actualRequest.method,
        entry.actualRequest.path,
        decodeBase64(entry.actualRequest.bodyBase64),
      );
      expect(binding.valid, entry.name).toBe(entry.expectedBinding);
      expect(binding.mismatches, entry.name).toEqual(
        entry.expectedMismatches ?? [],
      );
    }
  });

  test("signed-request canonical bytes match the golden fixture exactly", async () => {
    const entry = (await fixture("route-signing.json")).cases[0];
    expect(entry).toBeDefined();
    if (
      entry === undefined ||
      entry.expectedCanonicalText === undefined ||
      entry.expectedCanonicalBytesBase64 === undefined ||
      entry.expectedCanonicalSha256 === undefined
    ) return;
    const envelope = asSignedRequestEnvelopeV1(entry.value);
    const unsigned = unsignedEnvelope(envelope);
    const bytes = canonicalSignedRequestBytes(unsigned);
    expect(canonicalSignedRequestText(unsigned)).toBe(entry.expectedCanonicalText);
    expect(encodeBase64(bytes)).toBe(entry.expectedCanonicalBytesBase64);
    expect(await sha256Hex(bytes)).toBe(entry.expectedCanonicalSha256);
  });

  test("signed-request canonical bytes bind the represented public key", async () => {
    const entry = (await fixture("route-signing.json")).cases[0];
    expect(entry).toBeDefined();
    if (entry === undefined) return;
    const envelope = asSignedRequestEnvelopeV1(entry.value);
    const original = canonicalSignedRequestBytes(unsignedEnvelope(envelope));
    const substituted = canonicalSignedRequestBytes({
      ...unsignedEnvelope(envelope),
      signer: { ...envelope.signer, publicKey: "different-suite-key-material" },
    });
    expect(encodeBase64(substituted)).not.toBe(encodeBase64(original));
    expect(await sha256Hex(substituted)).not.toBe(await sha256Hex(original));
  });

  test("request-policy fixtures characterize expiration and nonce replay", async () => {
    const cases = (await fixture("request-freshness.json")).cases
      .toSorted((left, right) => (left.replaySequence ?? 0) - (right.replaySequence ?? 0));
    const usedNonces = new Set<string>();
    for (const entry of cases) {
      const envelope = asSignedRequestEnvelopeV1(entry.value);
      expect(entry.evaluatedAt).toBeDefined();
      expect(entry.maxClockSkewMs).toBeDefined();
      if (entry.evaluatedAt === undefined || entry.maxClockSkewMs === undefined) continue;
      const delta = Math.abs(Date.parse(entry.evaluatedAt) - Date.parse(envelope.timestamp));
      expect(delta <= entry.maxClockSkewMs, entry.name).toBe(entry.expectedFresh ?? false);
      const replayKey = `${envelope.signer.suite}\0${envelope.signer.keyId}\0${envelope.nonce}`;
      expect(usedNonces.has(replayKey), entry.name).toBe(entry.expectedReplay ?? false);
      usedNonces.add(replayKey);
    }
  });

  test("stable event IDs are not artifact digests", async () => {
    const records = await fixture("records.json");
    const eventCase = records.cases.find(
      (entry) => entry.contract === "ContinuityEvent" && entry.expectedValid,
    );
    const artifactCase = records.cases.find(
      (entry) => entry.contract === "ContinuityArtifact" && entry.expectedValid,
    );
    expect(eventCase).toBeDefined();
    expect(artifactCase).toBeDefined();
    if (eventCase === undefined || artifactCase === undefined) return;
    const event = eventCase.value as { eventId: string };
    const artifact = artifactCase.value as {
      eventId: string;
      digest: { value: string };
    };
    expect(event.eventId).toBe(artifact.eventId);
    expect(event.eventId).not.toBe(artifact.digest.value);
    expect(event.eventId.startsWith("evt_")).toBe(true);
  });

  test("the executable validator enforces proof disclosure uniqueness", async () => {
    const records = await fixture("records.json");
    const proofCase = records.cases.find(
      (entry) => entry.contract === "ContinuityProof" && entry.expectedValid,
    );
    expect(proofCase).toBeDefined();
    if (proofCase === undefined) return;
    const duplicate = structuredClone(proofCase.value) as {
      disclosure: { fields: string[] };
    };
    const first = duplicate.disclosure.fields[0];
    expect(first).toBeDefined();
    if (first === undefined) return;
    duplicate.disclosure.fields.push(first);
    const result = await validateContract("ContinuityProof", duplicate);
    expect(result.valid).toBe(false);
    expect(result.issues.map((issue) => issue.code)).toContain("array.unique");
  });
});
