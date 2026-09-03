import test from "node:test";
import assert from "node:assert/strict";
import { access, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
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
    assert(!names.includes("src/postinstall.js"));
    assert(names.includes("src/start.js"));
    assert(!names.some((name) => name.includes(".env") || name.startsWith("test/") || name.includes("node_modules")));
    const metadata = JSON.parse(await readFile(path.join(packageRoot, "package.json"), "utf8"));
    assert.equal(metadata.scripts.postinstall, undefined);
    const prefix = path.join(temporary, "prefix");
    const installed = spawnSync("npm", ["install", "--global", "--prefix", prefix, "--offline", path.join(temporary, filename)], {
      encoding: "utf8", env: { ...process.env, npm_config_ignore_scripts: "true" },
    });
    assert.equal(installed.status, 0, installed.stderr);
    const executable = path.join(prefix, "bin", "tohseno");
    assert.match(await readFile(executable, "utf8"), /node/);
    for (const args of [[], ["--version"], ["--help"]]) {
      const run = spawnSync(executable, args, { encoding: "utf8" });
      assert.equal(run.status, 0, run.stderr);
      assert.match(run.stdout, /TOHSENO|tohseno/);
      if (args[0] === "--version") assert.equal(run.stdout, "tohseno 1.2.1\n");
    }
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});

test("a fresh global install installs only the CLI launcher", async () => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), "tohseno-npm-cli-only-"));
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
        npm_config_ignore_scripts: "false",
      },
    });
    assert.equal(installed.status, 0, installed.stderr);
    assert.match(installed.stdout, /added 1 package/);
    await assert.rejects(access(path.join(home, ".tohseno")));

    const executable = path.join(prefix, "bin", "tohseno");
    const guide = spawnSync(executable, [], {
      encoding: "utf8",
      env: { ...process.env, HOME: home },
    });
    assert.equal(guide.status, 0, guide.stderr);
    assert.match(guide.stdout, /tohseno init\n  tohseno deploy/);
    await assert.rejects(access(path.join(home, ".tohseno")));
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
