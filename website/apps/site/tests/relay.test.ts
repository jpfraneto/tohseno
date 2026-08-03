import { afterEach, describe, expect, test } from "bun:test";
import { createHash, randomBytes } from "node:crypto";
import { mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createApplication } from "../server.ts";
import { loadConfig } from "../config.ts";
import { FilesystemRelayStorage } from "../src/relay-storage.ts";

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
const capability = () => randomBytes(32).toString("base64url");
const verifier = (value: string) => createHash("sha256").update(value).digest("hex");
const digest = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");

async function fixture() {
  const root = mkdtempSync(join(tmpdir(), "tohseno-relay-test-")); roots.push(root);
  const logs: Record<string, unknown>[] = [];
  const application = await createApplication({
    config: loadConfig({ NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000", INTENT_RELAY_ENABLED: "true", CLAIM_INSTALLER_READY: "true", INTENT_RELAY_ROOT: root }),
    log: (record) => logs.push(record), logError: (record) => logs.push(record),
  });
  const upload = capability(); const status = capability(); const claim = capability();
  return { root, logs, application, upload, status, claim };
}

const browser = (path: string, init: RequestInit = {}) => new Request(`http://localhost:3000${path}`, {
  ...init,
  headers: { Origin: "http://localhost:3000", "Sec-Fetch-Site": "same-origin", ...(init.headers || {}) },
});
const auth = (value: string) => ({ Authorization: `Bearer ${value}` });

describe("encrypted relay", () => {
  test("uploads, retries, leases, deletes synchronously, and leaves an authenticated tombstone", async () => {
    const fx = await fixture(); const ciphertext = new TextEncoder().encode("opaque ciphertext only");
    const createdResponse = await fx.application.fetch(browser("/api/intent-relay/records", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({
        schema: "tohseno.intent-relay-create/1", ciphertext_bytes: ciphertext.byteLength, chunk_count: 1,
        ciphertext_sha256: digest(ciphertext), nonce: "AAAAAAAAAAAAAAAA", associated_data: "tohseno.intent-envelope/1",
        upload_verifier: verifier(fx.upload), status_verifier: verifier(fx.status), claim_verifier: verifier(fx.claim),
      }),
    }));
    expect(createdResponse.status).toBe(201); expect(createdResponse.headers.get("Cache-Control")).toBe("no-store");
    const { relay_id: id } = await createdResponse.json();
    const upload = () => fx.application.fetch(browser(`/api/intent-relay/records/${id}/chunks/000000`, { method: "PUT", headers: { "Content-Type": "application/octet-stream", "X-Chunk-SHA256": digest(ciphertext), ...auth(fx.upload) }, body: ciphertext }));
    expect((await upload()).status).toBe(200);
    expect(await (await upload()).json()).toMatchObject({ duplicate: true });
    expect((await fx.application.fetch(browser(`/api/intent-relay/records/${id}/finalize`, { method: "POST", headers: { "Content-Type": "application/json", ...auth(fx.upload) }, body: "{}" }))).status).toBe(200);
    const leaseResponse = await fx.application.fetch(new Request(`http://localhost:3000/api/intent-relay/records/${id}/claim`, { method: "POST", headers: { "Content-Type": "application/json", ...auth(fx.claim) }, body: "{}" }));
    const lease = await leaseResponse.json(); expect(lease.state).toBeUndefined(); expect(lease.chunk_count).toBe(1);
    const downloaded = await fx.application.fetch(new Request(`http://localhost:3000/api/intent-relay/records/${id}/claim/chunks/000000`, { headers: auth(lease.lease_capability) }));
    expect(new Uint8Array(await downloaded.arrayBuffer())).toEqual(ciphertext);
    const complete = () => fx.application.fetch(new Request(`http://localhost:3000/api/intent-relay/records/${id}/complete`, { method: "POST", headers: { "Content-Type": "application/json", ...auth(lease.lease_capability) }, body: "{}" }));
    expect((await complete()).status).toBe(200); expect((await complete()).status).toBe(200);
    expect(readdirSync(join(fx.root, id))).toEqual(["tombstone.json"]);
    expect((await (await fx.application.fetch(new Request(`http://localhost:3000/api/intent-relay/records/${id}/status`, { headers: auth(fx.status) }))).json()).state).toBe("completed");
    const disk = readFileSync(join(fx.root, id, "tombstone.json"), "utf8");
    expect(disk).not.toContain("opaque ciphertext only"); expect(disk).not.toContain(fx.status); expect(disk).not.toContain(fx.claim);
    expect(JSON.stringify(fx.logs)).not.toContain(fx.upload); expect(JSON.stringify(fx.logs)).not.toContain(id);
  });

  test("fails closed across origin, content type, authorization, ordering, and body limits", async () => {
    const fx = await fixture();
    const body = JSON.stringify({ schema: "tohseno.intent-relay-create/1", ciphertext_bytes: 17, chunk_count: 1, ciphertext_sha256: "a".repeat(64), nonce: "AAAAAAAAAAAAAAAA", associated_data: "tohseno.intent-envelope/1", upload_verifier: verifier(fx.upload), status_verifier: verifier(fx.status), claim_verifier: verifier(fx.claim) });
    const crossOrigin = await fx.application.fetch(browser("/api/intent-relay/records", { method: "POST", headers: { Origin: "https://evil.example", "Content-Type": "application/json" }, body }));
    expect(crossOrigin.status).toBe(403); expect(crossOrigin.headers.get("Cache-Control")).toBe("no-store");
    expect((await fx.application.fetch(new Request("http://localhost:3000/api/intent-relay/records", { method: "POST", headers: { "Content-Type": "application/json" }, body }))).status).toBe(403);
    expect((await fx.application.fetch(browser("/api/intent-relay/records", { method: "POST", headers: { "Content-Type": "text/plain" }, body }))).status).toBe(415);
    const created = await fx.application.fetch(browser("/api/intent-relay/records", { method: "POST", headers: { "Content-Type": "application/json" }, body }));
    expect(created.status).toBe(201);
    const { relay_id: id } = await created.json();
    expect((await fx.application.fetch(browser(`/api/intent-relay/records/${id}/status`, { headers: auth(capability()) }))).status).toBe(401);
    expect((await fx.application.fetch(browser("/api/intent-relay/records", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ padding: "x".repeat(20 * 1024) }) }))).status).toBe(413);
    expect((await fx.application.fetch(new Request("http://localhost:3000/api/intent-relay/records/../../etc/status", { headers: auth(fx.status) }))).status).toBe(404);
  });

  test("rejects conflicting retries, missing chunks, and final digest mismatches", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno-relay-integrity-test-")); roots.push(root);
    const storage = new FilesystemRelayStorage(root, { maxRecords: 10, maxBytes: 1024 }); await storage.initialize();
    const upload = capability(); const status = capability(); const claim = capability();
    const first = new Uint8Array([1, 2]); const second = new Uint8Array([3, 4]); const both = new Uint8Array([1, 2, 3, 4]);
    const input = { ciphertextBytes: 4, chunkCount: 2, ciphertextSha256: digest(both), nonce: "AAAAAAAAAAAAAAAA", associatedData: "tohseno.intent-envelope/1", uploadVerifier: verifier(upload), statusVerifier: verifier(status), claimVerifier: verifier(claim) };
    const record = await storage.create(input);
    await storage.uploadChunk(record.id, upload, 0, first, digest(first));
    expect(storage.uploadChunk(record.id, upload, 0, second, digest(second))).rejects.toThrow("conflicting");
    expect(storage.finalize(record.id, upload)).rejects.toThrow("incomplete");
    await storage.uploadChunk(record.id, upload, 1, second, digest(second));
    await storage.finalize(record.id, upload);

    const wrong = await storage.create({ ...input, ciphertextSha256: "f".repeat(64) });
    await storage.uploadChunk(wrong.id, upload, 0, first, digest(first));
    await storage.uploadChunk(wrong.id, upload, 1, second, digest(second));
    expect(storage.finalize(wrong.id, upload)).rejects.toThrow("digest");

    const interrupted = await storage.create({ ...input, ciphertextBytes: 4, chunkCount: 2 });
    writeFileSync(join(root, interrupted.id, "chunks/000000"), first, { mode: 0o600 });
    expect(await storage.uploadChunk(interrupted.id, upload, 0, first, digest(first))).toMatchObject({ duplicate: true });
    await storage.uploadChunk(interrupted.id, upload, 1, second, digest(second));
    await storage.finalize(interrupted.id, upload);
  });

  test("capability endpoint is honest while production activation is gated", async () => {
    const disabled = await createApplication({ config: loadConfig({ NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000" }) });
    expect(await (await disabled.fetch(new Request("http://localhost:3000/api/intent-relay"))).json()).toMatchObject({ available: false });
    expect(() => loadConfig({ NODE_ENV: "production", PORT: "3000", BASE_URL: "https://tohseno.com", INTENT_RELAY_ENABLED: "true", INTENT_RELAY_ROOT: "/relay" })).toThrow("CLAIM_INSTALLER_READY");
    expect(() => loadConfig({ NODE_ENV: "production", PORT: "3000", BASE_URL: "https://tohseno.com", INTENT_RELAY_ENABLED: "true", CLAIM_INSTALLER_READY: "true", INTENT_RELAY_ROOT: "relative" })).toThrow("absolute path");
  });

  test("storage recovers an expired lease, expires uploads, cancels, and enforces capacity", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno-relay-storage-test-")); roots.push(root);
    const storage = new FilesystemRelayStorage(root, { maxRecords: 2, maxBytes: 1024 }); await storage.initialize();
    const now = Date.now();
    const upload = capability(); const status = capability(); const claim = capability();
    const bytes = new Uint8Array([1, 2, 3, 4]);
    const input = { ciphertextBytes: 4, chunkCount: 1, ciphertextSha256: digest(bytes), nonce: "AAAAAAAAAAAAAAAA", associatedData: "tohseno.intent-envelope/1", uploadVerifier: verifier(upload), statusVerifier: verifier(status), claimVerifier: verifier(claim) };
    const first = await storage.create(input, now);
    await storage.uploadChunk(first.id, upload, 0, bytes, digest(bytes), now + 1);
    await storage.finalize(first.id, upload, now + 2);
    const restarted = new FilesystemRelayStorage(root, { maxRecords: 2, maxBytes: 1024 }); await restarted.initialize();
    const lease = await restarted.lease(first.id, claim, now + 3) as { lease_capability: string };
    expect(restarted.lease(first.id, claim, now + 4)).rejects.toThrow("not available");
    expect((await restarted.status(first.id, status, now + 3 + 15 * 60 * 1000 + 1)).state).toBe("ready");
    const secondLease = await restarted.lease(first.id, claim, now + 3 + 15 * 60 * 1000 + 2) as { lease_capability: string };
    await restarted.release(first.id, secondLease.lease_capability, now + 3 + 15 * 60 * 1000 + 3);
    const second = await storage.create(input, now + 100); await storage.cancel(second.id, upload, now + 101);
    expect((await storage.status(second.id, status, now + 102)).state).toBe("cancelled");
    const incomplete = await storage.create(input, now + 200);
    await storage.cleanup(20, now + 200 + 60 * 60 * 1000 + 1);
    expect((await storage.status(incomplete.id, status, now + 200 + 60 * 60 * 1000 + 2)).state).toBe("expired");
    expect(lease.lease_capability).not.toBe(secondLease.lease_capability);

    const capacityRoot = mkdtempSync(join(tmpdir(), "tohseno-relay-capacity-test-")); roots.push(capacityRoot);
    const capacity = new FilesystemRelayStorage(capacityRoot, { maxRecords: 1, maxBytes: 4 }); await capacity.initialize();
    await capacity.create(input, now);
    expect(capacity.create(input, now + 1)).rejects.toThrow("capacity");
  });

  test("rejects a symlinked relay root and removes expired tombstones with bounded cleanup", async () => {
    const parent = mkdtempSync(join(tmpdir(), "tohseno-relay-symlink-test-")); roots.push(parent);
    const real = join(parent, "real"); const link = join(parent, "link");
    const initial = new FilesystemRelayStorage(real, { maxRecords: 2, maxBytes: 1024 }); await initial.initialize();
    symlinkSync(real, link);
    await expect(new FilesystemRelayStorage(link, { maxRecords: 2, maxBytes: 1024 }).initialize()).rejects.toThrow("symbolic link");

    const upload = capability(); const status = capability(); const claim = capability(); const now = Date.now();
    const bytes = new Uint8Array([1]);
    const record = await initial.create({ ciphertextBytes: 1, chunkCount: 1, ciphertextSha256: digest(bytes), nonce: "AAAAAAAAAAAAAAAA", associatedData: "tohseno.intent-envelope/1", uploadVerifier: verifier(upload), statusVerifier: verifier(status), claimVerifier: verifier(claim) }, now);
    await initial.cancel(record.id, upload, now + 1);
    expect(await initial.cleanup(1, now + 8 * 24 * 60 * 60 * 1000)).toBe(1);
    expect(() => readdirSync(join(real, record.id))).toThrow();
  });
});
