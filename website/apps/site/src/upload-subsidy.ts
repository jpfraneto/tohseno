import { mkdir, open, readFile } from "node:fs/promises";
import { join } from "node:path";
import { HttpError } from "./security.ts";

/** One durable gas reservation per Builder, shared by all retries of one upload. */
export async function reserveUploadSubsidy(
  registryRoot: string,
  builderAddress: string,
  publicationID: string,
  hasPreviousUpload: boolean,
): Promise<void> {
  if (!/^0x[0-9a-f]{40}$/.test(builderAddress) || !/^[0-9a-f]{32}$/.test(publicationID)) {
    throw new Error("Invalid verified upload subsidy identity");
  }
  const directory = join(registryRoot, "upload-subsidies");
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const path = join(directory, `${builderAddress}.json`);
  // Historic catalog entries count: a rollout must not grant existing Builders
  // another free upload. Persist that fact even if catalog entries later move.
  const reservation = hasPreviousUpload ? "already-used" : publicationID;
  try {
    const file = await open(path, "wx", 0o600);
    try {
      await file.writeFile(JSON.stringify({ schema: "menlo.upload-subsidy/1", publicationID: reservation }));
      await file.sync();
    } finally { await file.close(); }
    const parent = await open(directory, "r");
    try { await parent.sync(); } finally { await parent.close(); }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
  }
  let stored: { schema?: unknown; publicationID?: unknown };
  try { stored = JSON.parse(await readFile(path, "utf8")); }
  catch {
    // A concurrent write or interrupted write never opens another subsidy.
    throw new HttpError(503, "Upload subsidy reservation is not readable; retry this same upload.");
  }
  if (hasPreviousUpload || stored.schema !== "menlo.upload-subsidy/1" || stored.publicationID !== publicationID) {
    throw new HttpError(402, "Your one sponsored upload has been used. Further uploads require ETH funding; funding setup is not available yet.");
  }
}
