import { afterEach, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createApplication } from "../server.ts";
import { loadConfig } from "../config.ts";
import { buildIntentPackage } from "../public/modules/intent-package.js";
import { base64url, claimToken, createEncryptedEnvelope } from "../public/modules/intent-crypto.js";

const repository = fileURLToPath(new URL("../../../../", import.meta.url));
const temporaryRoots: string[] = [];
afterEach(() => { for (const root of temporaryRoots.splice(0)) rmSync(root, { recursive: true, force: true }); });
const sha = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex");
async function unusedPort(): Promise<number> {
  const probe = Bun.serve({ port: 0, fetch: () => new Response("probe") });
  const port = probe.port;
  await probe.stop(true);
  if (port === undefined) throw new Error("Bun did not assign an ephemeral port");
  return port;
}

test("browser package reaches durable local pending state and Studio without a paid harness", async () => {
  const root = mkdtempSync(join(tmpdir(), "tohseno-local-handoff-")); temporaryRoots.push(root);
  const relayRoot = join(root, "relay"); const dataRoot = join(root, "data");
  const port = await unusedPort(); const origin = `http://127.0.0.1:${port}`;
  const app = await createApplication({ config: loadConfig({
    NODE_ENV: "test", PORT: String(port), BASE_URL: origin, INTENT_RELAY_ENABLED: "true",
    CLAIM_INSTALLER_READY: "true", INTENT_RELAY_ROOT: relayRoot,
  }) });
  const server = Bun.serve({ port, fetch: app.fetch });
  let studio: ReturnType<typeof Bun.spawn> | null = null;
  try {
    const first = new Blob([new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1])], { type: "image/png" });
    const second = new Blob([new Uint8Array([0xff, 0xd8, 0xff, 2])], { type: "image/jpeg" });
    const prompt = "An offline field notebook for two harmless test trees 🌲";
    const packageBytes = await buildIntentPackage({ createdAt: "2026-08-03T00:00:00Z", prompt, references: [
      { blob: first, originalFilename: "tree-one.png", mimeType: "image/png", digest: "", order: 0 },
      { blob: second, originalFilename: "tree-two.jpeg", mimeType: "image/jpeg", digest: "", order: 1 },
    ] });
    const envelope = await createEncryptedEnvelope(packageBytes);
    const chunks = Math.ceil(envelope.ciphertext.byteLength / (1024 * 1024));
    const create = await fetch(`${origin}/api/intent-relay/records`, { method: "POST", headers: { Origin: origin, "Sec-Fetch-Site": "same-origin", "Content-Type": "application/json" }, body: JSON.stringify({
      schema: "tohseno.intent-relay-create/1", ciphertext_bytes: envelope.ciphertext.byteLength, chunk_count: chunks,
      ciphertext_sha256: envelope.ciphertextSha256, nonce: envelope.nonce, associated_data: envelope.associatedData,
      upload_verifier: envelope.verifiers.upload, status_verifier: envelope.verifiers.status, claim_verifier: envelope.verifiers.claim,
    }) });
    expect(create.status).toBe(201); const created = await create.json();
    for (let index = 0; index < chunks; index += 1) {
      const bytes = envelope.ciphertext.slice(index * 1024 * 1024, (index + 1) * 1024 * 1024);
      const uploaded = await fetch(`${origin}/api/intent-relay/records/${created.relay_id}/chunks/${String(index).padStart(6, "0")}`, { method: "PUT", headers: { Origin: origin, "Sec-Fetch-Site": "same-origin", "Content-Type": "application/octet-stream", "X-Chunk-SHA256": sha(bytes), Authorization: `Bearer ${envelope.capabilities.upload}` }, body: bytes });
      expect(uploaded.status).toBe(200);
    }
    expect((await fetch(`${origin}/api/intent-relay/records/${created.relay_id}/finalize`, { method: "POST", headers: { Origin: origin, "Sec-Fetch-Site": "same-origin", "Content-Type": "application/json", Authorization: `Bearer ${envelope.capabilities.upload}` }, body: "{}" })).status).toBe(200);
    const token = claimToken(created.relay_id, envelope.capabilities.claim, envelope.key);
    const wrongToken = claimToken(created.relay_id, envelope.capabilities.claim, base64url(new Uint8Array(32)));
    const rejected = Bun.spawn(["cargo", "run", "--quiet", "-p", "tohseno", "--", "intent", "claim", "--stdin", "--no-open"], {
      cwd: repository, env: { ...process.env, TOHSENO_DATA_ROOT: dataRoot, TOHSENO_INTENT_RELAY_ORIGIN: origin }, stdin: "pipe", stdout: "pipe", stderr: "pipe",
    });
    rejected.stdin.write(`${wrongToken}\n`); rejected.stdin.end();
    const [rejectedExit, rejectedOut, rejectedError] = await Promise.all([rejected.exited, new Response(rejected.stdout).text(), new Response(rejected.stderr).text()]);
    expect(rejectedExit).not.toBe(0);
    expect(`${rejectedOut}${rejectedError}`).not.toContain(wrongToken);
    expect(`${rejectedOut}${rejectedError}`).not.toContain(prompt);
    const recovered = await fetch(`${origin}/api/intent-relay/records/${created.relay_id}/status`, { headers: { Authorization: `Bearer ${envelope.capabilities.status}` } });
    expect((await recovered.json()).state).toBe("ready");

    const claimed = Bun.spawn(["cargo", "run", "--quiet", "-p", "tohseno", "--", "intent", "claim", "--stdin", "--no-open"], {
      cwd: repository, env: { ...process.env, TOHSENO_DATA_ROOT: dataRoot, TOHSENO_INTENT_RELAY_ORIGIN: origin }, stdin: "pipe", stdout: "pipe", stderr: "pipe",
    });
    claimed.stdin.write(`${token}\n`); claimed.stdin.end();
    const [exit, stdout, stderr] = await Promise.all([claimed.exited, new Response(claimed.stdout).text(), new Response(claimed.stderr).text()]);
    expect(exit).toBe(0); expect(`${stdout}${stderr}`).not.toContain(token);
    const records = readdirSync(join(dataRoot, "pending-intentions/records")); expect(records).toHaveLength(1);
    const pendingId = records[0]; const record = JSON.parse(readFileSync(join(dataRoot, "pending-intentions/records", pendingId, "record.json"), "utf8"));
    expect(record.prompt).toBe(prompt); expect(record.references.map((item: { display_filename: string }) => item.display_filename)).toEqual(["tree-one.png", "tree-two.jpeg"]);
    expect(readdirSync(join(relayRoot, created.relay_id))).toEqual(["tombstone.json"]);

    const portableRoot = join(root, "portable-data"); const portablePath = join(root, "fallback.tohseno-intent");
    writeFileSync(portablePath, packageBytes);
    const portable = Bun.spawn([join(repository, "target/debug/tohseno"), "intent", "open", portablePath, "--no-open"], {
      cwd: repository, env: { ...process.env, TOHSENO_DATA_ROOT: portableRoot }, stdout: "pipe", stderr: "pipe",
    });
    const [portableExit, portableOut, portableError] = await Promise.all([portable.exited, new Response(portable.stdout).text(), new Response(portable.stderr).text()]);
    expect(portableExit).toBe(0); expect(`${portableOut}${portableError}`).not.toContain(prompt); expect(`${portableOut}${portableError}`).not.toContain("tree-one.png");
    expect(readdirSync(join(portableRoot, "pending-intentions/records"))).toHaveLength(1);
    const studioPort = await unusedPort();
    studio = Bun.spawn([join(repository, "target/debug/tohseno"), "studio", "--port", String(studioPort), "--pending", pendingId], {
      cwd: repository, env: {
        ...process.env, TOHSENO_DATA_ROOT: dataRoot, TOHSENO_HOME: join(root, "shots"),
        TOHSENO_STUDIO_NO_OPEN: "1", TOHSENO_IDENTITY_BACKEND: "software-test",
        TOHSENO_TEST_NONLAUNCHING_HARNESS: "1",
      }, stdout: "pipe", stderr: "pipe",
    });
    let response: Response | null = null;
    for (let attempt = 0; attempt < 60; attempt += 1) {
      try { response = await fetch(`http://127.0.0.1:${studioPort}/api/pending-intentions/${pendingId}`); if (response.ok) break; } catch { /* starting */ }
      await Bun.sleep(100);
    }
    expect(response?.status).toBe(200); const view = await response!.json();
    expect(view.prompt).toBe(prompt); expect(view.references).toHaveLength(2); expect(view.safe_on_this_mac).toBe(true);
    const onboarding = await (await fetch(`http://127.0.0.1:${studioPort}/api/onboarding`)).json();
    expect(typeof onboarding.ready_for_first_shot).toBe("boolean");
    const studioHeaders = { Origin: `http://127.0.0.1:${studioPort}`, "Content-Type": "application/json", "X-TOHSENO-STUDIO": "1" };
    const plan = await fetch(`http://127.0.0.1:${studioPort}/api/plan`, { method: "POST", headers: studioHeaders, body: JSON.stringify({ app_name: view.suggested_app_name, pending_intention_id: pendingId }) });
    expect(plan.status).toBe(200);
    const ambiguous = await fetch(`http://127.0.0.1:${studioPort}/shots`, { method: "POST", headers: studioHeaders, body: JSON.stringify({
      mode: "create", app_name: view.suggested_app_name, pending_intention_id: pendingId, prompt: "different", accept_genome: true,
      selected_feedback_actions: [], harness: "tohseno-test-nonlaunching", model: "fixture", route: "no-inference",
    }) });
    expect(ambiguous.status).toBe(400);
    expect(readdirSync(join(dataRoot, "pending-intentions/records"))).toContain(pendingId);
    const stopped = await fetch(`http://127.0.0.1:${studioPort}/shots`, { method: "POST", headers: studioHeaders, body: JSON.stringify({
      mode: "create", app_name: view.suggested_app_name, pending_intention_id: pendingId, accept_genome: true,
      selected_feedback_actions: [], harness: "unsupported", model: "default", route: "none",
    }) });
    expect(stopped.status).toBe(422);
    expect(readdirSync(join(dataRoot, "pending-intentions/records"))).toContain(pendingId);
    const prepared = await fetch(`http://127.0.0.1:${studioPort}/shots`, { method: "POST", headers: studioHeaders, body: JSON.stringify({
      mode: "create", app_name: view.suggested_app_name, pending_intention_id: pendingId, accept_genome: true,
      selected_feedback_actions: [], harness: "tohseno-test-nonlaunching", model: "fixture", route: "no-inference",
    }) });
    const preparedBody = await prepared.text();
    expect(`${prepared.status}: ${preparedBody}`).toStartWith("201:");
    expect(() => readdirSync(join(dataRoot, "pending-intentions/records", pendingId))).toThrow();
    const receipt = JSON.parse(readFileSync(join(dataRoot, "pending-intentions/receipts", `${sha(packageBytes)}.json`), "utf8"));
    expect(receipt.state).toBe("consumed");
  } finally {
    studio?.kill(); await server.stop(true);
  }
}, 120_000);
