import { LIMITS } from "./intent-limits.js";
import { sha256Hex } from "./intent-package.js";

async function relayFetch(path, { capability, ...init } = {}) {
  const headers = new Headers(init.headers);
  if (capability) headers.set("Authorization", `Bearer ${capability}`);
  const response = await fetch(path, { ...init, headers, cache: "no-store", credentials: "omit" });
  if (!response.ok) {
    let message = `Relay request failed (${response.status}).`;
    try { message = (await response.json()).error || message; } catch { /* bounded generic fallback */ }
    throw new Error(message);
  }
  return response;
}

export async function capabilities() { return (await relayFetch("/api/intent-relay")).json(); }

export async function uploadEnvelope(envelope, onProgress, onCreated, existingRecord = null) {
  const chunks = Math.ceil(envelope.ciphertext.byteLength / LIMITS.chunkBytes);
  let created = existingRecord;
  if (!created) {
    const create = await relayFetch("/api/intent-relay/records", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({
        schema: "tohseno.intent-relay-create/1", ciphertext_bytes: envelope.ciphertext.byteLength,
        chunk_count: chunks, ciphertext_sha256: envelope.ciphertextSha256, nonce: envelope.nonce,
        associated_data: envelope.associatedData, upload_verifier: envelope.verifiers.upload,
        status_verifier: envelope.verifiers.status, claim_verifier: envelope.verifiers.claim,
      }),
    });
    created = await create.json();
    await onCreated?.(created);
  }
  for (let index = 0; index < chunks; index += 1) {
    const bytes = envelope.ciphertext.slice(index * LIMITS.chunkBytes, Math.min((index + 1) * LIMITS.chunkBytes, envelope.ciphertext.byteLength));
    await relayFetch(`/api/intent-relay/records/${created.relay_id}/chunks/${String(index).padStart(6, "0")}`, {
      method: "PUT", capability: envelope.capabilities.upload,
      headers: { "Content-Type": "application/octet-stream", "X-Chunk-SHA256": await sha256Hex(bytes) }, body: bytes,
    });
    onProgress?.(index + 1, chunks);
  }
  await relayFetch(`/api/intent-relay/records/${created.relay_id}/finalize`, {
    method: "POST", capability: envelope.capabilities.upload,
    headers: { "Content-Type": "application/json" }, body: "{}",
  });
  return created;
}

export async function relayStatus(relayId, statusCapability) {
  return (await relayFetch(`/api/intent-relay/records/${relayId}/status`, { capability: statusCapability })).json();
}

export async function cancelTransfer(relayId, uploadCapability) {
  await relayFetch(`/api/intent-relay/records/${relayId}/cancel`, {
    method: "POST", capability: uploadCapability, headers: { "Content-Type": "application/json" }, body: "{}",
  });
}
