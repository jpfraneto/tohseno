import { spawnSync } from "node:child_process";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { installedNative } from "./native.js";
import { ensureInstallerMarker } from "./installer.js";

export function isGlobalNpmInstall(platform = process.platform, environment = process.env) {
  return platform === "darwin" && environment.npm_config_global === "true";
}

export async function startFreshGlobalInstall({
  platform = process.platform,
  environment = process.env,
  findInstalledNative = installedNative,
  repairInstallerMarker = ensureInstallerMarker,
  spawn = spawnSync,
  executable = process.execPath,
  entrypoint = fileURLToPath(new URL("../bin/tohseno.js", import.meta.url)),
} = {}) {
  if (!isGlobalNpmInstall(platform, environment)) return 0;
  if (await findInstalledNative()) {
    await repairInstallerMarker();
    return 0;
  }

  const result = spawn(executable, [entrypoint], {
    env: environment,
    stdio: "inherit",
  });
  if (result.error) throw new Error("TOHSENO first run could not start");
  return result.status ?? 1;
}

const invokedDirectly = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (invokedDirectly) {
  startFreshGlobalInstall()
    .then((status) => { process.exitCode = status; })
    .catch((error) => {
      console.error(`tohseno: ${error.message}`);
      process.exitCode = 1;
    });
}
