import { createHash } from "node:crypto";
import { existsSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { relative, resolve } from "node:path";
import { validateAppManifest } from "../packages/manifest/app.ts";
import {
  PUBLIC_SHOT_LIFECYCLES,
  PUBLIC_SHOT_PROTOCOL_VERSION,
  PUBLIC_SHOT_RECORD_KINDS,
} from "../packages/protocol/src/index.ts";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const PRIVATE_PRODUCT_INPUTS = [
  "MASTER_PROMPT.md",
  "MASTER_EVOLUTIONARY_PROMPT.md",
  "TOHSENO_EVOLUTION_PROMPT.md",
] as const;

function fail(message: string): never {
  throw new Error(message);
}

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) fail(message);
}

async function run(label: string, command: string[]): Promise<void> {
  console.log(`\n[check] ${label}`);
  const child = Bun.spawn(command, {
    cwd: ROOT,
    env: process.env,
    stdin: "ignore",
    stdout: "inherit",
    stderr: "inherit",
  });
  const exitCode = await child.exited;
  if (exitCode !== 0) fail(`${label} failed with exit code ${exitCode}`);
}

async function capture(command: string[]): Promise<string> {
  const child = Bun.spawn(command, {
    cwd: ROOT,
    env: process.env,
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  });
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ]);
  if (exitCode !== 0) fail(`${command.join(" ")} failed: ${stderr.trim()}`);
  return stdout;
}

function listJsonFiles(relativeDirectory: string): string[] {
  const directory = resolve(ROOT, relativeDirectory);
  if (!existsSync(directory))
    fail(`Required directory is missing: ${relativeDirectory}`);

  const files: string[] = [];
  const visit = (current: string): void => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = resolve(current, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile() && entry.name.endsWith(".json")) {
        files.push(relative(ROOT, path));
      }
    }
  };
  visit(directory);
  return files.sort();
}

function listRegularFiles(relativeDirectory: string): string[] {
  const directory = resolve(ROOT, relativeDirectory);
  if (!existsSync(directory)) return [];
  const files: string[] = [];
  const visit = (current: string): void => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = resolve(current, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile()) files.push(relative(ROOT, path));
    }
  };
  visit(directory);
  return files.sort();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function readText(path: string): Promise<string> {
  const file = Bun.file(resolve(ROOT, path));
  if (!(await file.exists())) fail(`Required file is missing: ${path}`);
  return file.text();
}

async function readJson(path: string): Promise<unknown> {
  const text = await readText(path);
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return fail(`Invalid JSON: ${path}`);
  }
}

async function validateRepositoryJson(): Promise<void> {
  console.log("\n[check] canonical manifest, protocol, and registry formats");

  for (const path of [
    "templates/ios-kernel/overlay/app.manifest.json",
    "templates/daily-game/overlay/app.manifest.json",
  ]) {
    const result = validateAppManifest(await readJson(path));
    if (!result.valid) {
      const locations = result.errors
        .map((issue) => `${issue.path} (${issue.code})`)
        .join(", ");
      fail(`App manifest validation failed for ${path}: ${locations}`);
    }
  }
  for (const path of [
    "templates/ios-kernel/kernel.json",
    "templates/blank/template.json",
    "templates/daily-game/template.json",
    "skills/daily-challenge/skill.json",
    "skills/local-progress/skill.json",
    "skills/rank-progression/skill.json",
    "skills/share-card/skill.json",
    "packages/skills/app-skill.schema.json",
    "packages/skills/template.schema.json",
    "packages/skills/owner-ladder.schema.json",
    "owner-ladders/appstore-ios.json",
  ]) {
    await readJson(path);
  }

  assert(
    PUBLIC_SHOT_PROTOCOL_VERSION === 1,
    "The public Shot protocol version must remain explicit",
  );
  assert(
    JSON.stringify(PUBLIC_SHOT_LIFECYCLES) ===
      JSON.stringify(["EVOLVING", "PUBLISHED", "APP_STORE"]),
    "The public Shot lifecycle must contain exactly EVOLVING, PUBLISHED, and APP_STORE",
  );
  assert(
    JSON.stringify(PUBLIC_SHOT_RECORD_KINDS) === JSON.stringify([
      "SHOT_CREATED",
      "EVOLUTION_RECORDED",
      "LIFECYCLE_TRANSITIONED",
      "APPCOIN_LINKED",
    ]),
    "The public Shot record kind set changed without a protocol version",
  );
  const protocolSchemaPaths = [
    ...listJsonFiles("packages/identity/schemas"),
    ...listJsonFiles("packages/signer/schemas"),
    ...listJsonFiles("packages/protocol/schemas"),
    ...listJsonFiles("packages/registry/schemas"),
  ];
  assert(
    protocolSchemaPaths.length >= 4,
    "Identity, signer, and public Shot protocol schemas are required",
  );
  for (const path of protocolSchemaPaths) {
    const schema = await readJson(path);
    assert(isRecord(schema), `Protocol schema must be a JSON object: ${path}`);
    assert(
      schema.$schema === "https://json-schema.org/draft/2020-12/schema",
      `Protocol schema must declare JSON Schema 2020-12: ${path}`,
    );
    assert(
      typeof schema.$id === "string",
      `Protocol schema must declare an $id: ${path}`,
    );
  }
  for (const path of [
    "packages/identity/fixtures/builder-identity.json",
    "packages/protocol/fixtures/unsigned-shot-created.json",
    "packages/registry/fixtures/evolving-projection.json",
  ]) {
    await readJson(path);
  }
  const openapi = await readJson("apps/reference-node/openapi.json");
  assert(isRecord(openapi), "Reference node OpenAPI document must be an object");
  assert(openapi.openapi === "3.1.0", "Reference node must publish OpenAPI 3.1");
  assert(isRecord(openapi.paths), "Reference node OpenAPI paths are missing");
  for (const path of [
    "/healthz",
    "/openapi.json",
    "/v1/records",
    "/v1/shots/{shotId}",
    "/v1/shots/{shotId}/records",
  ]) {
    assert(
      Object.hasOwn(openapi.paths, path),
      `Reference node OpenAPI route is missing: ${path}`,
    );
  }
  await readText("apps/reference-node/server.ts");
}

async function validateStaticSurface(): Promise<void> {
  console.log("\n[check] static and deployment surface");

  const index = await readText("apps/site/public/index.html");
  const docs = await readText("apps/site/public/docs.html");
  const privacy = await readText("apps/site/public/privacy.html");
  const robots = await readText("apps/site/public/robots.txt");
  const htmlPages: Array<[string, string]> = [
    ["landing", index],
    ["docs", docs],
    ["privacy", privacy],
  ];
  const codeAssets = [
    await readText("apps/site/public/styles.css"),
    await readText("apps/site/public/landing.css"),
    await readText("apps/site/public/app.js"),
  ];

  for (const required of [
    "{{SOURCE_COMMAND}}",
    "data-copy-command",
    "data-shot-toggle",
    "{{REPOSITORY_URL}}",
    "https://community.tohseno.com",
    "The fastest way to prototype iOS apps",
    "A local intention compiler and open app factory for builders with more ideas than time",
    "Get rid of your recurring thoughts",
    "INFINITE SHOTS.",
    "Copy source command",
    "/shot-icons/shot-100.webp",
    'href="/docs"',
    'href="/privacy"',
  ]) {
    assert(
      index.includes(required),
      `Landing page is missing required hero contract: ${required}`,
    );
  }
  for (const [label, page] of htmlPages) {
    assert(
      !page.includes("<form"),
      `The ${label} page must not contain a form; the site has no intake`,
    );
    assert(
      !/<script(?![^>]*\bsrc=)[^>]*>/iu.test(page),
      `The ${label} page must not use inline scripts`,
    );
    assert(
      !/<(?:script|link|img|iframe|frame|embed|object|source|video|audio|form)\b[^>]*\b(?:src|href|action|data)\s*=\s*["'](?:https?:)?\/\//iu.test(
        page,
      ),
      `The ${label} page must not load resources from or submit to another origin`,
    );
    assert(
      page.includes('src="/app.js"'),
      `The ${label} page's JavaScript must be a separate same-origin asset`,
    );
  }
  assert(
    !codeAssets.some((source) =>
      /\bhttps?:\/\/|(?:src|href)\s*=\s*["']\/\/|url\(\s*["']?\/\//iu.test(
        source,
      ),
    ),
    "Public style and script assets must not reference other origins",
  );
  const publicCopy = htmlPages.map(([, page]) => page).join("\n");
  assert(
    !publicCopy.includes('href="/intake"'),
    "Public pages must not link to the archived intake product",
  );
  assert(
    !/(?:managed intake|encrypted intake|order lifecycle|private capsules?|\$88)/iu.test(
      publicCopy,
    ),
    "Public pages must not claim the archived intake/payments product",
  );

  for (const phrase of [
    "no accounts",
    "no TOHSENO telemetry",
    "Anky, Inc.",
    "support@anky.app",
  ]) {
    assert(
      privacy.includes(phrase),
      `Privacy page is missing required disclosure: ${phrase}`,
    );
  }
  assert(
    robots.includes("Allow: /"),
    "robots.txt must allow the public surface",
  );

  const shotIconDirectory = resolve(ROOT, "apps/site/public/shot-icons");
  assert(
    existsSync(shotIconDirectory),
    "Landing page shot icon directory is missing",
  );
  const shotIcons = readdirSync(shotIconDirectory)
    .filter((entry) => /^shot-\d{3}\.webp$/.test(entry))
    .sort();
  assert(
    shotIcons.length === 100,
    "Landing page must ship exactly 100 optimized shot icons",
  );
  for (let sequence = 1; sequence <= 100; sequence += 1) {
    const expected = `shot-${String(sequence).padStart(3, "0")}.webp`;
    assert(
      shotIcons[sequence - 1] === expected,
      `Landing page shot icon is missing: ${expected}`,
    );
    const icon = Bun.file(resolve(shotIconDirectory, expected));
    assert(
      icon.size < 32_000,
      `Landing page shot icon exceeds 32 KB: ${expected}`,
    );
  }
  await readText("apps/site/assets/shot-icon-manifest.json");
  await readText("apps/site/scripts/extract-shot-icons.ts");

  const environmentExample = await readText(".env.example");
  for (const variable of ["NODE_ENV", "PORT", "BASE_URL", "TRUST_PROXY"]) {
    assert(
      new RegExp(`^${variable}=`, "mu").test(environmentExample),
      `.env.example is missing ${variable}`,
    );
  }
  assert(
    !/(?:STRIPE|RESEND|TOHSENO_DATA_KEY|TOHSENO_OPERATOR_TOKEN|DATABASE_PATH)/.test(
      environmentExample,
    ),
    ".env.example must not reintroduce intake-era configuration",
  );

  const dockerfile = await readText("Dockerfile");
  assert(
    dockerfile.includes("FROM oven/bun:"),
    "Dockerfile must use the official Bun image",
  );
  assert(
    /^USER bun$/m.test(dockerfile),
    "Production container must run as the non-root bun user",
  );
  assert(
    !/^\s*VOLUME\b/m.test(dockerfile),
    "The static site needs no volume; a Docker VOLUME declaration is unsupported on Railway",
  );

  const railway = await readText("railway.toml");
  let railwayConfig: unknown;
  try {
    railwayConfig = Bun.TOML.parse(railway);
  } catch {
    fail("railway.toml is not valid TOML");
  }
  assert(isRecord(railwayConfig), "railway.toml must have an object root");
  assert(isRecord(railwayConfig.build), "railway.toml must contain [build]");
  assert(isRecord(railwayConfig.deploy), "railway.toml must contain [deploy]");
  assert(
    railwayConfig.build.builder === "DOCKERFILE",
    "Railway must use the Dockerfile builder",
  );
  assert(
    railwayConfig.deploy.startCommand === undefined,
    "Railway must preserve the Docker ENTRYPOINT instead of overriding it with a start command",
  );
  assert(
    railwayConfig.deploy.healthcheckPath === "/healthz",
    "Railway health check path is incorrect",
  );
  assert(
    railwayConfig.deploy.restartPolicyType === "ON_FAILURE",
    "Railway restart policy is incorrect",
  );
}

async function validateGenesisBoundary(): Promise<void> {
  console.log("\n[check] protocol genesis boundary");

  for (const path of [
    "apps/mobile",
    "apps/tohseno-mobile",
    "templates/tohseno-mobile",
    "skills/tohseno-mobile",
  ]) {
    assert(
      !existsSync(resolve(ROOT, path)),
      `Reserved pre-genesis TOHSENO mobile application path must be absent: ${path}`,
    );
  }

  const ignore = await readText(".gitignore");
  for (const privateInput of PRIVATE_PRODUCT_INPUTS) {
    assert(
      ignore.split(/\r?\n/u).includes(privateInput),
      `Private product input must remain explicitly gitignored: ${privateInput}`,
    );
  }

  const replaceableCode = [
    "apps/reference-node/server.ts",
    ...[
    "packages/identity/src",
    "packages/signer/src",
    "packages/protocol/src",
    "packages/registry/src",
    "packages/node-client/src",
    "apps/reference-node/src",
    ].flatMap((directory) => listRegularFiles(directory)),
  ];
  for (const path of replaceableCode) {
    if (!existsSync(resolve(ROOT, path))) continue;
    const source = await readText(path);
    assert(
      !source.toLowerCase().includes("tohseno.com"),
      `Replaceable protocol and node code must not hardcode a TOHSENO hostname: ${path}`,
    );
  }
}

async function validatePinnedInstallerBoundary(): Promise<void> {
  console.log("\n[check] pinned installer boundary");

  const installer = await readText("apps/site/public/install.sh");
  assert(
    createHash("sha256").update(installer).digest("hex") ===
      "442325c0355ed4b2ba3896367bfd5e143bb0e481c4d84e09df08a702ef9528ca",
    "the canonical installer differs from the reviewed 0.5.0 pin",
  );
  assert(
    installer.startsWith("#!/bin/sh\n"),
    "install.sh must remain a portable POSIX shell script",
  );
  const installerVersionMatch = installer.match(
    /^INSTALLER_VERSION="([0-9]+\.[0-9]+\.[0-9]+)"$/m,
  );
  const installerCliVersionMatch = installer.match(
    /^CLI_VERSION="([0-9]+\.[0-9]+\.[0-9]+)"$/m,
  );
  assert(
    installerVersionMatch !== null &&
      installerCliVersionMatch !== null &&
      installerVersionMatch[1] === "0.5.0" &&
      installerCliVersionMatch[1] === "0.5.0",
    "install.sh must carry the one canonical 0.5.0 installer and CLI version",
  );
  for (const phrase of [
    'install_root="${TOHSENO_INSTALL_HOME:-$HOME/.tohseno}"',
    "TOHSENO_INSTALL_CLI_SHA256",
    "checksum mismatch",
    'MANAGED_HOME_MARKER=".tohseno-managed-home-v1"',
    "pre-release compatibility is unsupported",
    "--non-interactive",
    "--dry-run",
    "TOHSENO_SOURCE_ROOT",
    "No credentials are requested or collected",
  ]) {
    assert(
      installer.includes(phrase),
      `install.sh is missing required managed-install behavior: ${phrase}`,
    );
  }
  const cliChecksum = installer.match(/^CLI_SHA256_DEFAULT="([0-9a-f]{64})"$/m);
  assert(
    cliChecksum !== null,
    "install.sh must pin the published CLI artifact with a complete SHA-256 digest",
  );
  assert(
    cliChecksum?.[1] ===
      "9737b8a87b6c203a5275ec5cf4e6c6a616f9e05e7da3dc8821d7f2b4c3111313" &&
      installer.includes(
        'CLI_TREE_SHA256_DEFAULT="dea1607ca84f056061c890f718c242733cbf089cc4e3f4701d88e750eb236367"',
      ),
    "install.sh must pin the exact published 0.5.0 archive and tree",
  );
  assert(
    !installer.includes("__TOHSENO_CLI_SHA256__"),
    "install.sh still contains an unfinalized checksum placeholder",
  );
  assert(
    !/(?:raw\.githubusercontent\.com|refs\/heads\/|archive\/refs\/heads)/u.test(
      installer,
    ),
    "install.sh must never execute or install mutable repository content",
  );
  assert(
    !/migration|0\.4\.0|oneshot/iu.test(installer),
    "install.sh must not contain pre-0.5 migration or alternate-bootstrap behavior",
  );
  assert(
    !existsSync(resolve(ROOT, "apps/site/public/oneshot.sh")),
    "there must be no alternate bootstrap entry point",
  );
  const server = await readText("apps/site/server.ts");
  assert(
    !server.includes('"/install.sh"') &&
      !server.includes('"/oneshot.sh"'),
    "the pinned installer must remain unserved until the serving commit; the alternate bootstrap must remain absent",
  );
}

async function validateRepositoryHygiene(): Promise<void> {
  console.log("\n[check] tracked/unignored file and secret hygiene");
  const output = await capture([
    "git",
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "-z",
  ]);
  const paths = output
    .split("\0")
    .filter((path) => path !== "" && existsSync(resolve(ROOT, path)))
    .sort();
  for (const path of paths) {
    const name = path.split("/").at(-1) ?? path;
    if (
      PRIVATE_PRODUCT_INPUTS.includes(
        name as (typeof PRIVATE_PRODUCT_INPUTS)[number],
      )
    ) {
      fail(`Private product input must not be tracked or unignored: ${path}`);
    }
    if (name.startsWith(".env") && name !== ".env.example")
      fail(`Environment file must not be tracked or unignored: ${path}`);
    if (
      /\.(?:sqlite(?:-wal|-shm)?|db|pem|p8|p12|pfx|mobileprovision)$/i.test(
        name,
      ) ||
      /(^|\/)data\//.test(path)
    ) {
      fail(
        `Private runtime/credential file must not be tracked or unignored: ${path}`,
      );
    }
  }

  const secretPatterns: Array<[string, RegExp]> = [
    ["Stripe live secret", /sk_live_[A-Za-z0-9]{20,}/],
    ["App Store Connect API key", /AuthKey_[A-Z0-9]{10}\.p8/],
    [
      "private key block",
      /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----\s*\n(?:[A-Za-z0-9+/=]{20,}\s*\n){2,}/,
    ],
  ];
  for (const path of paths) {
    const file = Bun.file(resolve(ROOT, path));
    if (file.size > 5 * 1024 * 1024) continue;
    const source = await file.text();
    for (const [label, pattern] of secretPatterns) {
      if (pattern.test(source)) fail(`${label} appears in ${path}`);
    }
  }
}

async function main(): Promise<void> {
  await run("strict TypeScript", [process.execPath, "run", "typecheck"]);
  await run("test suite", [process.execPath, "test"]);
  await validateRepositoryJson();
  await validateStaticSurface();
  await validateGenesisBoundary();
  await validatePinnedInstallerBoundary();
  await validateRepositoryHygiene();
  await run("unstaged whitespace errors", ["git", "diff", "--check"]);
  await run("staged whitespace errors", ["git", "diff", "--cached", "--check"]);
  console.log("\n[check] all checks passed");
}

try {
  await main();
} catch (error) {
  const message =
    error instanceof Error ? error.message : "Unknown check failure";
  console.error(`\n[check] ${message}`);
  process.exitCode = 1;
}
