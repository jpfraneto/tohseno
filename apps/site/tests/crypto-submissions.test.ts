import { describe, expect, test } from "bun:test";
import { PRODUCT } from "../config.ts";
import {
  decryptString,
  encryptString,
  generateCapabilityToken,
  hashCapabilityToken,
  isCapabilityTokenShape,
} from "../src/crypto.ts";
import type { SubmissionRow } from "../src/submissions.ts";
import {
  SubmissionValidationError,
  validateMarkdown,
} from "../src/submissions.ts";
import {
  createSiteHarness,
  submitThroughHttp,
  syntheticMarkdown,
  testConfig,
} from "./helpers.ts";

describe("authenticated encryption", () => {
  test("round-trips Unicode through a versioned AES-256-GCM envelope", async () => {
    const key = testConfig().dataKeyBase64;
    const plaintext = "One observation: araucaria, café, 雨, and 🌱.";
    const serialized = await encryptString(plaintext, key);
    const envelope = JSON.parse(serialized) as Record<string, unknown>;

    expect(envelope.version).toBe(1);
    expect(envelope.algorithm).toBe("AES-256-GCM");
    expect(envelope.key).toBe("primary");
    expect(Buffer.from(String(envelope.nonce), "base64")).toHaveLength(12);
    expect(serialized).not.toContain(plaintext);
    expect(await decryptString(serialized, key)).toBe(plaintext);
  });

  test("detects ciphertext tampering", async () => {
    const key = testConfig().dataKeyBase64;
    const serialized = await encryptString("tamper-evident private source", key);
    const envelope = JSON.parse(serialized) as {
      version: number;
      algorithm: string;
      key: string;
      nonce: string;
      ciphertext: string;
    };
    const ciphertext = Buffer.from(envelope.ciphertext, "base64");
    ciphertext[0] = (ciphertext[0] ?? 0) ^ 0xff;
    envelope.ciphertext = ciphertext.toString("base64");

    await expect(decryptString(JSON.stringify(envelope), key)).rejects.toThrow();
  });
});

describe("capability primitives", () => {
  test("generates at least 256 random bits and hashes tokens with domain separation", async () => {
    const first = generateCapabilityToken();
    const second = generateCapabilityToken();

    expect(first).not.toBe(second);
    expect(isCapabilityTokenShape(first)).toBe(true);
    expect(Buffer.from(first, "base64url")).toHaveLength(32);

    const firstHash = await hashCapabilityToken(first);
    expect(firstHash).toMatch(/^[a-f0-9]{64}$/);
    expect(firstHash).toBe(await hashCapabilityToken(first));
    expect(firstHash).not.toBe(await hashCapabilityToken(second));
    expect(firstHash).not.toContain(first);
  });
});

describe("deterministic Markdown preflight", () => {
  test("accepts useful text at the byte limit and rejects content above it", () => {
    const exact = `# action\n${"a".repeat(PRODUCT.maxMarkdownBytes - 9)}`;
    expect(new TextEncoder().encode(exact)).toHaveLength(PRODUCT.maxMarkdownBytes);
    expect(() => validateMarkdown(exact)).not.toThrow();

    expect(() => validateMarkdown(`${exact}a`)).toThrow(SubmissionValidationError);
    try {
      validateMarkdown(`${exact}a`);
    } catch (error) {
      expect(error).toBeInstanceOf(SubmissionValidationError);
      expect((error as SubmissionValidationError).field).toBe("markdown");
    }
  });

  test("counts UTF-8 bytes rather than JavaScript characters", () => {
    const multibyte = `# action\n${"é".repeat(Math.floor(PRODUCT.maxMarkdownBytes / 2))}`;
    expect(multibyte.length).toBeLessThan(PRODUCT.maxMarkdownBytes);
    expect(new TextEncoder().encode(multibyte).byteLength).toBeGreaterThan(PRODUCT.maxMarkdownBytes);
    expect(() => validateMarkdown(multibyte)).toThrow("at most");
  });

  test("rejects too-short and binary-like input", () => {
    expect(() => validateMarkdown("tiny note")).toThrow("useful characters");
    expect(() => validateMarkdown(`${syntheticMarkdown()}\0binary`)).toThrow("binary");
    expect(() => validateMarkdown(`${syntheticMarkdown()}${"\u0001".repeat(24)}`)).toThrow("binary control");
    expect(() => validateMarkdown(`${syntheticMarkdown()}\ud800`)).toThrow("valid Unicode");
  });
});

describe("private submission persistence", () => {
  test("persists encrypted intake without plaintext Markdown, email, or raw capability", async () => {
    const harness = await createSiteHarness();
    try {
      const marker = crypto.randomUUID();
      const markdown = `# PRIVATE-${marker}\n\nRecord one synthetic observation and return without exposing this marker.`;
      const email = `private-${marker}@example.test`;
      const created = await submitThroughHttp(harness, "self-hosted", { markdown, email });
      const row = harness.application.database.query<SubmissionRow, [string]>(
        "SELECT * FROM submissions WHERE id = ?",
      ).get(created.submissionId);

      expect(row).not.toBeNull();
      expect(row?.status).toBe("READY_FOR_PAYMENT");
      expect(row?.encrypted_markdown).not.toContain(markdown);
      expect(row?.encrypted_contact).not.toContain(email);
      expect(row?.capability_token_hash).toBe(await hashCapabilityToken(created.token));
      expect(row?.capability_token_hash).not.toContain(created.token);

      const events = harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(created.submissionId);
      expect(events?.count).toBe(2);

      const persisted = harness.persistedBytes();
      expect(persisted.includes(Buffer.from(markdown))).toBe(false);
      expect(persisted.includes(Buffer.from(email))).toBe(false);
      expect(persisted.includes(Buffer.from(created.token))).toBe(false);
    } finally {
      await harness.close();
    }
  });
});
