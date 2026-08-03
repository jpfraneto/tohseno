import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { buildIntentPackage, parseIntentPackage, prepareReferences, safeFilename, utf8ByteLength } from "../public/modules/intent-package.js";
import { base64url, createEncryptedEnvelope, encryptIntentPackage, parseClaimToken } from "../public/modules/intent-crypto.js";
import { insertText, resolvePromptFile, transferStateLabel } from "../public/modules/draft-logic.js";
import { portableSha256 } from "../public/modules/sha256-portable.js";

const png = (number: number) => new Blob([new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, number])], { type: "image/png" });
const reference = (number: number) => ({ blob: png(number), originalFilename: `${number}.png`, mimeType: "image/png", digest: "", order: number - 1 });

describe("private intent package", () => {
  test("portable SHA-256 keeps local package integrity available without Web Crypto", () => {
    const digest = Buffer.from(portableSha256(new TextEncoder().encode("abc"))).toString("hex");
    expect(digest).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  });
  test("counts UTF-8 and constructs deterministic ordered offsets and digests", async () => {
    expect(utf8ByteLength("tree 🌲")).toBe(9);
    const input = { createdAt: "2026-08-03T00:00:00Z", prompt: "Remember tree 🌲", references: [reference(1), reference(2)] };
    const first = await buildIntentPackage(input);
    const second = await buildIntentPackage(input);
    expect(first).toEqual(second);
    const { manifest, payload } = await parseIntentPackage(first);
    expect(manifest.schema).toBe("tohseno.intent-package/1");
    expect(manifest.references.map((item: { ordinal: number }) => item.ordinal)).toEqual([0, 1]);
    expect(manifest.references[0].payload_offset).toBe(0);
    expect(manifest.references[1].payload_offset).toBe(9);
    expect(payload.byteLength).toBe(18);
    expect(manifest.prompt.sha256).toMatch(/^[a-f0-9]{64}$/);
  });

  test("rejects empty, ninth, unsupported, duplicate, and unsafe inputs", async () => {
    expect(buildIntentPackage({ createdAt: "now", prompt: " ", references: [] })).rejects.toThrow("Write an intention");
    expect(buildIntentPackage({ createdAt: "now", prompt: "x", references: Array.from({ length: 9 }, (_, i) => reference(i + 1)) })).rejects.toThrow("at most eight");
    expect(safeFilename("../bad.png")).toBe(false);
    expect(prepareReferences([{ blob: new Blob(["bad"]), originalFilename: "bad.gif", mimeType: "image/gif", digest: "", order: 0 }])).rejects.toThrow("Use PNG");
    expect(prepareReferences([reference(1), { ...reference(1), originalFilename: "copy.png" }])).rejects.toThrow("repeats");
    expect(await prepareReferences(Array.from({ length: 8 }, (_, i) => reference(i + 1)))).toHaveLength(8);
  });

  test("AES-GCM and claim tokens are browser compatible and strict", async () => {
    const fixed = await encryptIntentPackage(
      new TextEncoder().encode("TOHSENO browser AES-GCM vector"),
      new Uint8Array(32).fill(0x11),
      new Uint8Array(12).fill(0x22),
    );
    expect(Buffer.from(fixed).toString("hex")).toBe("43b84f1a8581d07f874db14b3bc39bf55ed69ecf2dc2e16b242a699c669342e05245593e2017eda2b91985b0a99f");
    const envelope = await createEncryptedEnvelope(new TextEncoder().encode("private"));
    expect(envelope.ciphertext).not.toContain(new TextEncoder().encode("private"));
    expect(envelope.key).toHaveLength(43);
    const token = `ti1.${base64url(new Uint8Array(24))}.${envelope.capabilities.claim}.${envelope.key}`;
    expect(parseClaimToken(token).key).toHaveLength(32);
    expect(() => parseClaimToken(`${token}.extra`)).toThrow("Malformed");
    expect(() => parseClaimToken(token.replace("ti1", "ti2"))).toThrow("Malformed");
  });

  test("matches the checked-in Rust/browser compatibility vector", async () => {
    const vector = JSON.parse(readFileSync(fileURLToPath(new URL("../../../../fixtures/intent-package-v1.json", import.meta.url)), "utf8"));
    const references = vector.references.map((item: { bytes_base64: string; media_type: string; display_filename: string }, order: number) => ({
      blob: new Blob([Buffer.from(item.bytes_base64, "base64")], { type: item.media_type }),
      originalFilename: item.display_filename, mimeType: item.media_type, digest: "", order,
    }));
    const bytes = await buildIntentPackage({ createdAt: vector.created_at, prompt: vector.prompt, references });
    expect(Buffer.from(bytes).toString("base64")).toBe(vector.package_base64);
  });
});

describe("draft semantics", () => {
  test("examples and files never silently destroy existing text", () => {
    expect(insertText("existing", "example").value).toBe("existing\n\nexample");
    expect(resolvePromptFile("existing", "file", "append")).toBe("existing\n\nfile");
    expect(resolvePromptFile("existing", "file", "replace")).toBe("file");
    expect(transferStateLabel("leased")).toContain("Claimed");
  });
});
