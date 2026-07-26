import { describe, expect, test } from "bun:test";
import {
  deriveDeterministicTestIdentity,
  identityReferencesEqual,
  validateIdentityReference,
  validateVerificationMethod,
} from "../src/index.ts";

describe("protocol identities", () => {
  test("keeps Builder and Continuity roles distinct", () => {
    const builder = deriveDeterministicTestIdentity("BUILDER", "same");
    const continuity = deriveDeterministicTestIdentity("CONTINUITY", "same");
    expect(builder.id).not.toBe(continuity.id);
    expect(identityReferencesEqual(builder, continuity)).toBe(false);
  });

  test("derives stable local fixture references without deriving keys", () => {
    expect(deriveDeterministicTestIdentity("BUILDER", "fixture")).toEqual(
      deriveDeterministicTestIdentity("BUILDER", "fixture"),
    );
  });

  test("rejects unknown and malformed fields", () => {
    expect(() =>
      validateIdentityReference({
        role: "BUILDER",
        method: "local",
        id: "builder",
        prompt: "private",
      })
    ).toThrow("is not allowed");
    expect(() =>
      validateVerificationMethod({
        identity: { role: "BUILDER", method: "local", id: "builder" },
        suite: "suite",
        keyId: "key",
        publicKey: "with padding=",
      })
    ).toThrow("invalid format");
    expect(() =>
      validateVerificationMethod({
        identity: { role: "BUILDER", method: "local", id: "builder" },
        suite: "suite",
        keyId: "key",
        publicKey: "AAAAA",
      })
    ).toThrow("invalid format");
  });
});
