import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

test("npm pack installs into an isolated prefix without network or native mutation", async () => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), "tohseno-npm-pack-"));
  try {
    const packed = spawnSync("npm", ["pack", "--json", "--pack-destination", temporary], {
      cwd: packageRoot, encoding: "utf8", env: { ...process.env, npm_config_ignore_scripts: "true" },
    });
    assert.equal(packed.status, 0, packed.stderr);
    const [{ filename, files }] = JSON.parse(packed.stdout);
    const names = files.map((entry) => entry.path);
    assert(names.includes("bin/tohseno.js"));
    assert(names.includes("src/postinstall.js"));
    assert(names.includes("src/start.js"));
    assert(!names.some((name) => name.includes(".env") || name.startsWith("test/") || name.includes("node_modules")));
    const prefix = path.join(temporary, "prefix");
    const installed = spawnSync("npm", ["install", "--global", "--prefix", prefix, "--offline", path.join(temporary, filename)], {
      encoding: "utf8", env: { ...process.env, npm_config_ignore_scripts: "true" },
    });
    assert.equal(installed.status, 0, installed.stderr);
    const executable = path.join(prefix, "bin", "tohseno");
    assert.match(await readFile(executable, "utf8"), /node/);
    for (const args of [["--version"], ["--help"]]) {
      const run = spawnSync(executable, args, { encoding: "utf8" });
      assert.equal(run.status, 0, run.stderr);
      assert.match(run.stdout, /TOHSENO|tohseno/);
    }
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});

test("a fresh global install visibly enters first run", { skip: process.platform !== "darwin" }, async () => {
  const temporary = await realpath(await mkdtemp(path.join(os.tmpdir(), "tohseno-npm-first-run-")));
  try {
    const packageDirectory = path.join(temporary, "package");
    await mkdir(packageDirectory);
    const packed = spawnSync("npm", ["pack", "--json", "--pack-destination", packageDirectory], {
      cwd: packageRoot,
      encoding: "utf8",
      env: { ...process.env, npm_config_ignore_scripts: "true" },
    });
    assert.equal(packed.status, 0, packed.stderr);
    const [{ filename }] = JSON.parse(packed.stdout);

    const release = path.join(temporary, "fixture-release");
    const firstRunLog = path.join(temporary, "first-run.log");
    await mkdir(path.join(release, "bin"), { recursive: true });
    const native = `#!/bin/sh
set -eu
if [ "\${1:-}" = "--version" ]; then
  printf '%s\\n' 'tohseno 1.2.0'
  exit 0
fi
if [ -n "\${TOHSENO_FIRST_RUN_LOG:-}" ]; then
  printf '%s\\n' "$*" >> "$TOHSENO_FIRST_RUN_LOG"
fi
printf 'native tohseno:'
printf ' %s' "$@"
printf '\\n'
`;
    const target = process.arch === "arm64" ? "aarch64-apple-darwin" : "x86_64-apple-darwin";
    const releaseRecord = `${JSON.stringify({
      schema: "tohseno.release/1",
      version: "1.2.0",
      target,
      channel: "stable",
      prerelease: false,
      dirty: false,
    })}\n`;
    await writeFile(path.join(release, "bin", "tohseno"), native, { mode: 0o755 });
    await chmod(path.join(release, "bin", "tohseno"), 0o755);
    await writeFile(path.join(release, "RELEASE.json"), releaseRecord);
    const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
    await writeFile(path.join(release, "CHECKSUMS.sha256"), [
      `${digest(releaseRecord)}  RELEASE.json`,
      `${digest(native)}  bin/tohseno`,
      "",
    ].join("\n"));
    const archive = path.join(temporary, "fixture-release.tar.gz");
    const archived = spawnSync("/usr/bin/tar", ["-czf", archive, "-C", temporary, "fixture-release"], {
      encoding: "utf8",
    });
    assert.equal(archived.status, 0, archived.stderr);
    const archiveBytes = await readFile(archive);
    const artifactUrl = "https://github.com/jpfraneto/tohseno/releases/download/v1.2.0/fixture.tar.gz";
    const manifest = `${JSON.stringify({
      schema: "tohseno.native-release-manifest/1",
      native_release_version: "1.2.0",
      minimum_npm_cli_version: "1.2.0",
      layout_version: "tohseno-user-release/2",
      artifacts: [
        { architecture: "arm64", target: "aarch64-apple-darwin" },
        { architecture: "x64", target: "x86_64-apple-darwin" },
      ].map((artifact) => ({
        ...artifact,
        url: artifactUrl,
        byte_size: archiveBytes.length,
        sha256: digest(archiveBytes),
        signing: { kind: "release-package", team_id: null, designated_requirement: null },
      })),
    })}\n`;
    const hook = path.join(temporary, "fixture-fetch.mjs");
    await writeFile(hook, `import { readFileSync } from "node:fs";
const originalFetch = globalThis.fetch;
globalThis.fetch = async (input, options) => {
  const url = String(input);
  if (url === "https://tohseno.com/releases/native-v1.json") {
    const body = Buffer.from(${JSON.stringify(Buffer.from(manifest).toString("base64"))}, "base64");
    return new Response(body, { status: 200, headers: { "content-length": String(body.length) } });
  }
  if (url === ${JSON.stringify(artifactUrl)}) {
    const body = readFileSync(${JSON.stringify(archive)});
    return new Response(body, { status: 200, headers: { "content-length": String(body.length) } });
  }
  return originalFetch(input, options);
};
`);

    const home = path.join(temporary, "home");
    const prefix = path.join(temporary, "prefix");
    await mkdir(home);
    const installed = spawnSync("npm", [
      "install",
      "--global",
      "--prefix",
      prefix,
      "--offline",
      path.join(packageDirectory, filename),
    ], {
      encoding: "utf8",
      env: {
        ...process.env,
        HOME: home,
        NODE_OPTIONS: `--import=${hook}`,
        npm_config_ignore_scripts: "false",
        TOHSENO_FIRST_RUN_LOG: firstRunLog,
      },
    });
    assert.equal(installed.status, 0, installed.stderr);
    assert.match(installed.stdout, /added 1 package/);
    assert.doesNotMatch(installed.stdout, /Installing TOHSENO|Starting TOHSENO|Opening TOHSENO/);
    assert.equal(await readFile(firstRunLog, "utf8"), "service install\nstudio\n");
    const executable = path.join(prefix, "bin", "tohseno");
    const firstApp = spawnSync(executable, [
      "create",
      "--prompt",
      "A one-screen counter that remembers its value.",
      "--wait",
    ], {
      encoding: "utf8",
      env: {
        ...process.env,
        HOME: home,
        TOHSENO_FIRST_RUN_LOG: firstRunLog,
      },
    });
    assert.equal(firstApp.status, 0, firstApp.stderr);
    assert.equal(
      await readFile(firstRunLog, "utf8"),
      "service install\nstudio\ncreate --prompt A one-screen counter that remembers its value. --wait\n",
    );
    assert.equal(
      await readFile(path.join(home, ".tohseno", ".tohseno-install-root"), "utf8"),
      "tohseno-stable-install-v2\n",
    );
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
