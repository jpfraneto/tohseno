import test from "node:test";
import assert from "node:assert/strict";
import { parseCommand, redact } from "../src/cli.js";
import { compareVersions } from "../src/semver.js";
import { nodeArchitecture, validateManifest, validatedHttpsURL } from "../src/manifest.js";
import { validateArchivePaths } from "../src/archive.js";
import { verifyArtifactBytes } from "../src/download.js";
import { createHash } from "node:crypto";

function manifest() {
  return {
    schema: "tohseno.native-release-manifest/1",
    native_release_version: "1.0.0",
    minimum_npm_cli_version: "1.0.0",
    layout_version: "tohseno-user-release/2",
    artifacts: [
      {
        architecture: "arm64",
        target: "aarch64-apple-darwin",
        url: "https://github.com/jpfraneto/tohseno/releases/download/v1.0.0/tohseno-release-aarch64-apple-darwin.tar.gz",
        byte_size: 123,
        sha256: "ab".repeat(32),
        signing: { kind: "release-package", team_id: null, designated_requirement: null },
      },
      {
        architecture: "x64",
        target: "x86_64-apple-darwin",
        url: "https://github.com/jpfraneto/tohseno/releases/download/v1.0.0/tohseno-release-x86_64-apple-darwin.tar.gz",
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
  assert.equal(validateManifest(manifest(), "1.0.0", "arm64").artifact.target, "aarch64-apple-darwin");
  const newer = manifest();
  newer.minimum_npm_cli_version = "1.1.0";
  assert.throws(() => validateManifest(newer, "1.0.0", "arm64"), /too old/);
  assert.throws(() => nodeArchitecture("mips"), /Apple silicon and Intel/);
});

test("manifest rejects duplicate architectures, sizes, digests, and extra fields", () => {
  const duplicate = manifest();
  duplicate.artifacts[1].architecture = "arm64";
  duplicate.artifacts[1].target = "aarch64-apple-darwin";
  assert.throws(() => validateManifest(duplicate, "1.0.0", "arm64"), /duplicate/);
  const size = manifest();
  size.artifacts[0].byte_size = 0;
  assert.throws(() => validateManifest(size, "1.0.0", "arm64"), /byte size/);
  const digest = manifest();
  digest.artifacts[0].sha256 = "AB".repeat(32);
  assert.throws(() => validateManifest(digest, "1.0.0", "arm64"), /SHA-256/);
  const extra = manifest();
  extra.token = "secret";
  assert.throws(() => validateManifest(extra, "1.0.0", "arm64"), /unexpected/);
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
