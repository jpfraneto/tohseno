import { createHash } from "node:crypto";
import { createWriteStream } from "node:fs";
import { open, stat } from "node:fs/promises";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import { validatedHttpsURL } from "./manifest.js";

export async function fetchBounded(url, maximumBytes, allowedOrigins) {
  const checked = validatedHttpsURL(url, allowedOrigins);
  const response = await fetch(checked, { redirect: "manual", headers: { "user-agent": "tohseno-npm/0.1.0" } });
  if (response.status >= 300 && response.status < 400) throw new Error("release download redirect was refused");
  if (!response.ok) throw new Error(`release download failed with HTTP ${response.status}`);
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maximumBytes) throw new Error("release response is oversized");
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength > maximumBytes) throw new Error("release response is oversized");
  return bytes;
}

export async function downloadArtifact(url, destination, expectedSize, expectedDigest) {
  const checked = validatedHttpsURL(url);
  const response = await fetch(checked, { redirect: "manual", headers: { "user-agent": "tohseno-npm/0.1.0" } });
  if (response.status >= 300 && response.status < 400) throw new Error("native release redirect was refused");
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
