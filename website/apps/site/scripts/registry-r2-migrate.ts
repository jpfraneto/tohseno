import { loadConfig } from "../config.ts";
import { createRegistryBlobStore } from "../src/blob-store.ts";
import { migrateRegistryBlobs, r2BucketFingerprint } from "../src/registry-r2-migration.ts";

const argument = process.argv[2];
const sourceCommitArgument = process.argv[3];
if ((process.argv.length !== 3 && process.argv.length !== 4)
    || (argument !== "--dry-run" && argument !== "--apply")
    || (sourceCommitArgument !== undefined
      && !/^--source-commit=[0-9a-f]{40}$/.test(sourceCommitArgument))) {
  console.error(
    "usage: bun run registry:r2:migrate --dry-run|--apply [--source-commit=<40-lowercase-hex>]",
  );
  process.exit(2);
}

const config = loadConfig();
if (config.registry.blobStore !== "r2" || !config.registry.r2 || !config.registry.root) {
  console.error("REGISTRY_BLOB_STORE=r2 and complete Registry/R2 configuration are required");
  process.exit(2);
}
const sourceCommit = await resolveSourceCommit(
  sourceCommitArgument?.slice("--source-commit=".length),
);
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

async function resolveSourceCommit(explicit?: string): Promise<string> {
  if (explicit) return explicit;
  const railway = process.env.RAILWAY_GIT_COMMIT_SHA;
  if (railway !== undefined) {
    if (!/^[0-9a-f]{40}$/.test(railway)) throw new Error("Railway source commit is malformed");
    return railway;
  }
  const git = Bun.spawn(["git", "rev-parse", "HEAD"], {
    cwd: new URL("../../../..", import.meta.url).pathname,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [status, stdout] = await Promise.all([git.exited, new Response(git.stdout).text()]);
  const value = stdout.trim();
  if (status !== 0 || !/^[0-9a-f]{40}$/.test(value)) {
    throw new Error("source commit could not be resolved; pass --source-commit=<40-lowercase-hex>");
  }
  return value;
}
