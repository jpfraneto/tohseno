import { loadConfig } from "../config.ts";
import { createRegistryBlobStore } from "../src/blob-store.ts";
import { migrateRegistryBlobs, r2BucketFingerprint } from "../src/registry-r2-migration.ts";

const argument = process.argv[2];
if (process.argv.length !== 3 || (argument !== "--dry-run" && argument !== "--apply")) {
  console.error("usage: bun run registry:r2:migrate --dry-run|--apply");
  process.exit(2);
}

const config = loadConfig();
if (config.registry.blobStore !== "r2" || !config.registry.r2 || !config.registry.root) {
  console.error("REGISTRY_BLOB_STORE=r2 and complete Registry/R2 configuration are required");
  process.exit(2);
}
const sourceCommit = await gitCommit();
const observedAt = new Date().toISOString();
const audit = await migrateRegistryBlobs({
  root: config.registry.root,
  blobStore: createRegistryBlobStore(config.registry),
  mode: argument === "--apply" ? "apply" : "dry-run",
  sourceCommit,
  observedAt,
  bucketFingerprint: r2BucketFingerprint(config.registry.r2.accountId, config.registry.r2.bucket),
});
console.log(JSON.stringify(audit, null, 2));
process.exit(audit.passed ? 0 : 1);

async function gitCommit(): Promise<string> {
  const process = Bun.spawn(["git", "rev-parse", "HEAD"], {
    cwd: new URL("../../../..", import.meta.url).pathname,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [status, stdout] = await Promise.all([process.exited, new Response(process.stdout).text()]);
  const value = stdout.trim();
  if (status !== 0 || !/^[0-9a-f]{40}$/.test(value)) throw new Error("source commit could not be resolved");
  return value;
}
