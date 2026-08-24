import { createHash } from "node:crypto";
import { createWriteStream } from "node:fs";
import { open, stat } from "node:fs/promises";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import { validatedHttpsURL } from "./manifest.js";

const REDIRECT_STATUSES = new Set([301, 302, 303, 307, 308]);
const MAX_REDIRECTS = 3;

async function fetchAllowlisted(url, allowedOrigins) {
  let current = validatedHttpsURL(url, allowedOrigins);
  for (let redirects = 0; redirects <= MAX_REDIRECTS; redirects += 1) {
    const response = await fetch(current, {
      redirect: "manual",
      headers: { "user-agent": "tohseno-npm/1.0.0" },
    });
    if (!REDIRECT_STATUSES.has(response.status)) return response;
    if (redirects === MAX_REDIRECTS) throw new Error("release download has too many redirects");
    const location = response.headers.get("location");
    if (!location) throw new Error("release download redirect is missing its destination");
    current = validatedHttpsURL(new URL(location, current).href, allowedOrigins);
    if (response.body) await response.body.cancel();
  }
  throw new Error("release download has too many redirects");
}

export async function fetchBounded(url, maximumBytes, allowedOrigins) {
  const response = await fetchAllowlisted(url, allowedOrigins);
  if (!response.ok) throw new Error(`release download failed with HTTP ${response.status}`);
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maximumBytes) throw new Error("release response is oversized");
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength > maximumBytes) throw new Error("release response is oversized");
  return bytes;
}

export async function downloadArtifact(url, destination, expectedSize, expectedDigest) {
  const response = await fetchAllowlisted(url);
  if (!response.ok || !response.body) throw new Error(`native release download failed with HTTP ${response.status}`);
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared !== expectedSize) throw new Error("native release declared byte size differs");
  const hash = createHash("sha256");
  let observed = 0;
  const transform = new TransformStream({
    transform(chunk, controller) {
      observed += chunk.byteLength;
      if (observed > expectedSize) throw new Error("native release exceeds its authorized byte size");
      hash.update(chunk);
      controller.enqueue(chunk);
    },
  });
  await pipeline(Readable.fromWeb(response.body.pipeThrough(transform)), createWriteStream(destination, { flags: "wx", mode: 0o600 }));
  if (observed !== expectedSize) throw new Error("native release byte size differs");
  if (hash.digest("hex") !== expectedDigest) throw new Error("native release SHA-256 differs");
  const descriptor = await open(destination, "r");
  try { await descriptor.sync(); } finally { await descriptor.close(); }
  if ((await stat(destination)).size !== expectedSize) throw new Error("native release changed after download");
}

export function verifyArtifactBytes(bytes, expectedSize, expectedDigest) {
  if (bytes.byteLength !== expectedSize) throw new Error("native release byte size differs");
  const observed = createHash("sha256").update(bytes).digest("hex");
  if (observed !== expectedDigest) throw new Error("native release SHA-256 differs");
}
