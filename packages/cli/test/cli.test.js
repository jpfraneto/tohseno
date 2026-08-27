import test from "node:test";
import assert from "node:assert/strict";
import { parseCommand, redact } from "../src/cli.js";
import { compareVersions } from "../src/semver.js";
import { nodeArchitecture, validateManifest, validatedHttpsURL } from "../src/manifest.js";
import { validateArchivePaths } from "../src/archive.js";
import { verifyArtifactBytes } from "../src/download.js";
import { ensureInstallerMarker, verifyAppleSignature } from "../src/installer.js";
import { isGlobalNpmInstall, startFreshGlobalInstall } from "../src/postinstall.js";
import { startProduct } from "../src/start.js";
import { createHash } from "node:crypto";
import { access, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

function manifest() {
  return {
    schema: "tohseno.native-release-manifest/1",
    native_release_version: "1.0.2",
    minimum_npm_cli_version: "1.0.2",
    layout_version: "tohseno-user-release/2",
    artifacts: [
      {
        architecture: "arm64",
        target: "aarch64-apple-darwin",
        url: "https://github.com/jpfraneto/tohseno/releases/download/v1.0.2/tohseno-release-aarch64-apple-darwin.tar.gz",
        byte_size: 123,
        sha256: "ab".repeat(32),
        signing: { kind: "release-package", team_id: null, designated_requirement: null },
      },
      {
        architecture: "x64",
        target: "x86_64-apple-darwin",
        url: "https://github.com/jpfraneto/tohseno/releases/download/v1.0.2/tohseno-release-x86_64-apple-darwin.tar.gz",
        byte_size: 456,
        sha256: "cd".repeat(32),
        signing: { kind: "release-package", team_id: null, designated_requirement: null },
      },
    ],
  };
}

test("command parsing keeps native commands opaque", () => {
  assert.deepEqual(parseCommand([]), { kind: "start", args: [] });
  assert.deepEqual(parseCommand(["install"]), { kind: "install", args: [] });
  assert.deepEqual(parseCommand(["create", "my-app"]), { kind: "delegate", args: ["create", "my-app"] });
});

test("stable semantic versions compare without prerelease ambiguity", () => {
  assert.equal(compareVersions("1.0.0", "1.0.0"), 0);
  assert.equal(compareVersions("1.0.1", "1.0.0"), 1);
  assert.throws(() => compareVersions("1.0.0-beta", "1.0.0"));
});

test("manifest selects the exact architecture and enforces minimum CLI", () => {
  assert.equal(validateManifest(manifest(), "1.0.2", "arm64").artifact.target, "aarch64-apple-darwin");
  const newer = manifest();
  newer.minimum_npm_cli_version = "1.1.0";
  assert.throws(() => validateManifest(newer, "1.0.2", "arm64"), /too old/);
  assert.throws(() => nodeArchitecture("mips"), /Apple silicon and Intel/);
});

test("npm 1.0.2 refuses a stale native release manifest", () => {
  const stale = manifest();
  stale.native_release_version = "1.0.0";
  assert.throws(
    () => validateManifest(stale, "1.0.2", "arm64"),
    /requires native TOHSENO 1\.0\.2/,
  );
});

test("manifest rejects duplicate architectures, sizes, digests, and extra fields", () => {
  const duplicate = manifest();
  duplicate.artifacts[1].architecture = "arm64";
  duplicate.artifacts[1].target = "aarch64-apple-darwin";
  assert.throws(() => validateManifest(duplicate, "1.0.2", "arm64"), /duplicate/);
  const size = manifest();
  size.artifacts[0].byte_size = 0;
  assert.throws(() => validateManifest(size, "1.0.2", "arm64"), /byte size/);
  const digest = manifest();
  digest.artifacts[0].sha256 = "AB".repeat(32);
  assert.throws(() => validateManifest(digest, "1.0.2", "arm64"), /SHA-256/);
  const extra = manifest();
  extra.token = "secret";
  assert.throws(() => validateManifest(extra, "1.0.2", "arm64"), /unexpected/);
});

test("URL allowlist rejects HTTP, credentials, ports, and unapproved hosts", () => {
  assert.equal(validatedHttpsURL("https://github.com/jpfraneto/tohseno").hostname, "github.com");
  for (const value of [
    "http://github.com/jpfraneto/tohseno",
    "https://user:pass@github.com/jpfraneto/tohseno",
    "https://github.com:444/jpfraneto/tohseno",
    "https://evil.example/tohseno",
  ]) assert.throws(() => validatedHttpsURL(value), /allowlist/);
});

test("archive paths cannot traverse or escape", () => {
  assert.deepEqual(validateArchivePaths("release/\nrelease/bin/\nrelease/bin/tohseno\n").length, 3);
  for (const listing of ["../escape\n", "/absolute\n", "release/../../escape\n", "release\\escape\n"])
    assert.throws(() => validateArchivePaths(listing), /unsafe/);
});

test("diagnostic text redacts secret-shaped query values and provider keys", () => {
  const output = redact("https://tohseno.com/c?nonce=private-value sk_live_abcdef");
  assert.equal(output, "https://tohseno.com/c?nonce=[redacted] [redacted]");
});

test("artifact bytes require exact size and SHA-256", () => {
  const bytes = Buffer.from("authorized native release");
  const digest = createHash("sha256").update(bytes).digest("hex");
  assert.doesNotThrow(() => verifyArtifactBytes(bytes, bytes.length, digest));
  assert.throws(() => verifyArtifactBytes(bytes, bytes.length + 1, digest), /byte size/);
  assert.throws(() => verifyArtifactBytes(bytes, bytes.length, "00".repeat(32)), /SHA-256/);
});

test("Apple signature verification passes the manifest requirement inline", () => {
  const requirement = 'identifier tohseno and certificate leaf[subject.OU] = "84V63LKV45"';
  const calls = [];
  const spawn = (executable, args) => {
    calls.push({ executable, args });
    return calls.length === 1
      ? { status: 0, stdout: "", stderr: "" }
      : { status: 0, stdout: "", stderr: "TeamIdentifier=84V63LKV45\n" };
  };

  verifyAppleSignature("/verified/release/bin/tohseno", {
    kind: "apple-developer-id",
    team_id: "84V63LKV45",
    designated_requirement: requirement,
  }, spawn);

  assert.deepEqual(calls[0], {
    executable: "/usr/bin/codesign",
    args: [
      "--verify",
      "--deep",
      "--strict",
      "--test-requirement",
      `=${requirement}`,
      "/verified/release/bin/tohseno",
    ],
  });
});

test("only a fresh global Mac install starts first run automatically", async () => {
  assert.equal(isGlobalNpmInstall("darwin", { npm_config_global: "true" }), true);
  assert.equal(isGlobalNpmInstall("darwin", {}), false);
  assert.equal(isGlobalNpmInstall("linux", { npm_config_global: "true" }), false);

  const calls = [];
  const status = await startFreshGlobalInstall({
    platform: "darwin",
    environment: { npm_config_global: "true" },
    findInstalledNative: async () => null,
    spawn: (executable, args, options) => {
      calls.push({ executable, args, options });
      return { status: 0 };
    },
    executable: "/node",
    entrypoint: "/package/bin/tohseno.js",
  });
  assert.equal(status, 0);
  assert.deepEqual(calls, [{
    executable: "/node",
    args: ["/package/bin/tohseno.js"],
    options: { env: { npm_config_global: "true" }, stdio: "inherit" },
  }]);

  await startFreshGlobalInstall({
    platform: "darwin",
    environment: { npm_config_global: "true" },
    findInstalledNative: async () => ({ version: "1.0.2" }),
    repairInstallerMarker: async () => {},
    spawn: () => { throw new Error("must not start twice"); },
  });
});

test("npm installations write the marker expected by native uninstall", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "tohseno-marker-test-"));
  try {
    await writeFile(path.join(root, ".installer-managed"), "tohseno-stable-install-v2\n");
    await ensureInstallerMarker(root);
    assert.equal(
      await readFile(path.join(root, ".tohseno-install-root"), "utf8"),
      "tohseno-stable-install-v2\n",
    );
    await assert.rejects(access(path.join(root, ".installer-managed")));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("product start installs the service once and then opens Studio", () => {
  const commands = [];
  const messages = [];
  const status = startProduct((args) => {
    commands.push(args);
    return 0;
  }, (message) => messages.push(message));

  assert.equal(status, 0);
  assert.deepEqual(commands, [["service", "install"], ["studio"]]);
  assert.deepEqual(messages, ["Starting TOHSENO…", "Opening TOHSENO…"]);

  const failed = [];
  assert.equal(startProduct((args) => {
    failed.push(args);
    return 7;
  }, () => {}), 7);
  assert.deepEqual(failed, [["service", "install"]]);
});

test("redirects follow only an exact allowlisted HTTPS chain", async () => {
  const original = globalThis.fetch;
  try {
    const { fetchBounded } = await import("../src/download.js");
    let calls = 0;
    globalThis.fetch = async () => {
      calls += 1;
      return calls === 1
        ? new Response(null, { status: 302, headers: { location: "https://fixture.example/release" } })
        : new Response("release", { status: 200 });
    };
    const bytes = await fetchBounded(
      "https://fixture.example/start",
      100,
      new Set(["https://fixture.example"]),
    );
    assert.equal(new TextDecoder().decode(bytes), "release");
    assert.equal(calls, 2);

    globalThis.fetch = async () => new Response(null, {
      status: 302,
      headers: { location: "https://evil.example/release" },
    });
    await assert.rejects(
      fetchBounded("https://fixture.example/manifest", 100, new Set(["https://fixture.example"])),
      /allowlist/,
    );
  } finally {
    globalThis.fetch = original;
  }
});
