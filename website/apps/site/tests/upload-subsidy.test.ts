import { afterEach, expect, test } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { reserveUploadSubsidy } from "../src/upload-subsidy.ts";
const roots: string[] = [];
const builder = `0x${"1".repeat(40)}`;
const first = "a".repeat(32);
const second = "b".repeat(32);
async function root() { const value = await mkdtemp(join(tmpdir(), "menlo-subsidy-")); roots.push(value); return value; }
afterEach(async () => { await Promise.all(roots.splice(0).map((path) => rm(path, { recursive: true, force: true }))); });
test("one upload survives retries but a second upload requires funding", async () => {
  const path = await root();
  await reserveUploadSubsidy(path, builder, first, false);
  await reserveUploadSubsidy(path, builder, first, false);
  await expect(reserveUploadSubsidy(path, builder, second, false)).rejects.toThrow("one sponsored upload");
  await reserveUploadSubsidy(path, `0x${"2".repeat(40)}`, second, false);
});
test("concurrent reservations cannot spend two subsidies", async () => {
  const path = await root();
  const attempts = await Promise.allSettled([first, second].map((id) => reserveUploadSubsidy(path, builder, id, false)));
  expect(attempts.filter((result) => result.status === "fulfilled")).toHaveLength(1);
  const winner = attempts[0]?.status === "fulfilled" ? first : second;
  await reserveUploadSubsidy(path, builder, winner, false);
  await expect(reserveUploadSubsidy(path, builder, winner === first ? second : first, false)).rejects.toThrow();
});
test("historical uploads consume the subsidy across subsequent reads", async () => {
  const path = await root();
  await expect(reserveUploadSubsidy(path, builder, first, true)).rejects.toThrow("one sponsored upload");
  await expect(reserveUploadSubsidy(path, builder, first, false)).rejects.toThrow("one sponsored upload");
});
test("corrupt state and path injection cannot open another subsidy", async () => {
  const path = await root();
  await reserveUploadSubsidy(path, builder, first, false);
  await writeFile(join(path, "upload-subsidies", `${builder}.json`), "{");
  await expect(reserveUploadSubsidy(path, builder, second, false)).rejects.toThrow("not readable");
  await expect(reserveUploadSubsidy(path, "../elsewhere", first, false)).rejects.toThrow("identity");
});
