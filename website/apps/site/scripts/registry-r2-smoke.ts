import { loadConfig } from "../config.ts";
import { createRegistryBlobStore } from "../src/blob-store.ts";

const values = new Map<string, string>();
for (let index = 2; index < process.argv.length; index += 2) {
  const name = process.argv[index];
  const value = process.argv[index + 1];
  if (!name || !value || !name.startsWith("--") || values.has(name)) usage();
  values.set(name, value);
}
if (values.get("--confirm") !== "owner-attended-live-r2"
    || !/^0x[0-9a-f]{64}$/.test(values.get("--digest") ?? "")
    || !/^[1-9][0-9]*$/.test(values.get("--byte-length") ?? "")) usage();

const digest = values.get("--digest") as `0x${string}`;
const byteLength = Number(values.get("--byte-length"));
if (!Number.isSafeInteger(byteLength) || byteLength < 1) usage();
const config = loadConfig();
if (config.registry.blobStore !== "r2" || !config.registry.r2) {
  throw new Error("live smoke requires the complete fail-closed R2 Registry configuration");
}

const store = createRegistryBlobStore(config.registry);
await store.initialize();
const prefixLength = Math.min(byteLength, 1024 * 1024);
const full = await store.read({ digest, byteLength });
const fullObserved = await hashStream(full.stream, prefixLength);
if (fullObserved.byteLength !== byteLength || fullObserved.digest !== digest) {
  throw new Error("live R2 full-object read differs from its expected SHA-256 or length");
}
const range = await store.read({ digest, byteLength }, { start: 0, end: prefixLength - 1 });
const rangeObserved = await hashStream(range.stream, prefixLength);
if (rangeObserved.byteLength !== prefixLength
    || rangeObserved.digest !== fullObserved.prefixDigest) {
  throw new Error("live R2 first range differs from the same bytes in the full object");
}

console.log(JSON.stringify({
  schema: "tohseno.registry-r2-live-smoke/1",
  digest,
  byteLength,
  prefixLength,
  fullSHA256: fullObserved.digest,
  prefixSHA256: rangeObserved.digest,
  verified: true,
}, null, 2));

async function hashStream(stream: ReadableStream<Uint8Array>, prefixLength: number) {
  const hasher = new Bun.CryptoHasher("sha256");
  const prefixHasher = new Bun.CryptoHasher("sha256");
  const reader = stream.getReader();
  let byteLength = 0;
  let prefixBytes = 0;
  try {
    while (true) {
      const value = await reader.read();
      if (value.done) break;
      const bytes = value.value;
      byteLength += bytes.byteLength;
      hasher.update(bytes);
      if (prefixBytes < prefixLength) {
        const take = Math.min(bytes.byteLength, prefixLength - prefixBytes);
        prefixHasher.update(bytes.subarray(0, take));
        prefixBytes += take;
      }
    }
  } finally {
    reader.releaseLock();
  }
  return {
    digest: `0x${hasher.digest("hex")}` as `0x${string}`,
    prefixDigest: `0x${prefixHasher.digest("hex")}` as `0x${string}`,
    byteLength,
  };
}

function usage(): never {
  console.error(
    "usage: bun run registry:r2:smoke --confirm owner-attended-live-r2 --digest 0x<sha256> --byte-length <bytes>",
  );
  process.exit(2);
}
