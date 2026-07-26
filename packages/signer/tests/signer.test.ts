import { describe, expect, test } from "bun:test";
import { deriveDeterministicTestIdentity } from "../../identity/src/index.ts";
import {
  Ed25519Verifier,
  LocalEd25519Signer,
  VerifierSet,
} from "../src/index.ts";

describe("ephemeral Ed25519 signer", () => {
  test("regenerates deterministic public test vectors", async () => {
    const identity = deriveDeterministicTestIdentity(
      "BUILDER",
      "fixture-identity",
    );
    const first = LocalEd25519Signer.deterministicForTests(
      identity,
      "public-fixture-seed",
    );
    const second = LocalEd25519Signer.deterministicForTests(
      identity,
      "public-fixture-seed",
    );
    expect(first.verificationMethod).toEqual(second.verificationMethod);
    expect(await first.sign(Uint8Array.of(1, 2, 3))).toEqual(
      await second.sign(Uint8Array.of(1, 2, 3)),
    );
    expect(
      LocalEd25519Signer.deterministicForTests(
        identity,
        "different-public-seed",
      ).verificationMethod,
    ).not.toEqual(first.verificationMethod);
  });

  test("signs arbitrary bytes with identity-bound verification", async () => {
    const signer = LocalEd25519Signer.generate(
      deriveDeterministicTestIdentity("BUILDER", "signer"),
    );
    const message = Uint8Array.of(0, 1, 2, 255);
    const signature = await signer.sign(message);
    const verifier = new Ed25519Verifier();
    expect(await verifier.verify(message, signature)).toBe(true);
    expect(await verifier.verify(Uint8Array.of(0, 1, 3, 255), signature))
      .toBe(false);
    expect(
      await verifier.verify(message, {
        ...signature,
        identity: { ...signature.identity, id: "test_other" },
      }),
    ).toBe(false);
  });

  test("fails closed for unknown suites and malformed signatures", async () => {
    const signer = LocalEd25519Signer.generate(
      deriveDeterministicTestIdentity("BUILDER", "unknown-suite"),
    );
    const signature = await signer.sign(Uint8Array.of(1));
    expect(
      await new VerifierSet([new Ed25519Verifier()]).verify(
        Uint8Array.of(1),
        { ...signature, suite: "future-suite" },
      ),
    ).toBe(false);
    expect(
      await new Ed25519Verifier().verify(
        Uint8Array.of(1),
        { ...signature, value: `${signature.value}=` },
      ),
    ).toBe(false);
  });

  test("does not expose or permit mutation of local private authority", () => {
    const signer = LocalEd25519Signer.generate(
      deriveDeterministicTestIdentity("BUILDER", "encapsulation"),
    );
    expect(Object.keys(signer)).toEqual(["verificationMethod"]);
    expect(
      (signer as unknown as Record<string, unknown>).privateKey,
    ).toBeUndefined();
    expect(Object.isFrozen(signer.verificationMethod)).toBe(true);
    expect(Object.isFrozen(signer.verificationMethod.identity)).toBe(true);
    expect(() => {
      signer.verificationMethod.identity.id = "test_mutation";
    }).toThrow();
  });
});
