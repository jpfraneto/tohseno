import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { CliError } from "./errors.ts";

function isFactorySourceRoot(path: string): boolean {
  return existsSync(join(path, "templates", "continuity-app", "continuity.manifest.json")) &&
    existsSync(join(path, "packages", "manifest", "cli.ts")) &&
    existsSync(join(path, "skills", "continuity-app", "SKILL.md")) &&
    existsSync(join(path, "packages", "cli", "factory", "shot-verify.ts"));
}

export function locateFactorySourceRoot(
  environment: Record<string, string | undefined> = process.env,
  start = import.meta.dir,
): string {
  const override = environment.TOHSENO_SOURCE_ROOT;
  if (override !== undefined) {
    const candidate = resolve(override);
    if (!isFactorySourceRoot(candidate)) {
      throw new CliError(`TOHSENO_SOURCE_ROOT is not a compatible TOHSENO checkout: ${candidate}`);
    }
    return candidate;
  }

  let candidate = resolve(start);
  while (true) {
    if (isFactorySourceRoot(candidate)) return candidate;
    const parent = dirname(candidate);
    if (parent === candidate) break;
    candidate = parent;
  }

  const embedded = resolve(import.meta.dir, "..", "factory-source");
  if (isFactorySourceRoot(embedded)) return embedded;
  throw new CliError(
    "cannot locate TOHSENO factory assets; use this CLI from a checkout or set TOHSENO_SOURCE_ROOT",
  );
}
