import { createHash } from "node:crypto";
import { existsSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { relative, resolve } from "node:path";
import { validateManifest } from "../packages/manifest/validate.ts";
import { validateContract } from "../packages/contracts/validate.ts";
import type { ContractKind } from "../packages/contracts/types.ts";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

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
  console.log("\n[check] manifest examples and JSON contract corpus");

  const manifestPaths = ["templates/continuity-app/continuity.manifest.json"];
  for (const path of manifestPaths) {
    const result = validateManifest(await readJson(path));
    if (!result.valid) {
      const locations = result.errors
        .map((issue) => `${issue.path} (${issue.code})`)
        .join(", ");
      fail(`Manifest validation failed for ${path}: ${locations}`);
    }
  }

  const expectedSchemas = [
    "continuity-artifact.schema.json",
    "continuity-event.schema.json",
    "continuity-proof.schema.json",
    "continuity-reflection.schema.json",
    "signed-request-envelope-v1.schema.json",
  ];
  const schemaFiles = listJsonFiles("packages/contracts/schemas");
  for (const name of expectedSchemas) {
    assert(
      schemaFiles.some((path) => path.endsWith(`/${name}`)),
      `Missing contract schema: ${name}`,
    );
  }
  for (const path of schemaFiles) {
    const schema = await readJson(path);
    assert(isRecord(schema), `Contract schema must be a JSON object: ${path}`);
    assert(
      schema.$schema === "https://json-schema.org/draft/2020-12/schema",
      `Contract schema must declare JSON Schema 2020-12: ${path}`,
    );
    assert(
      typeof schema.$id === "string",
      `Contract schema must declare an $id: ${path}`,
    );
  }

  const fixtureFiles = listJsonFiles("packages/contracts/fixtures");
  assert(
    fixtureFiles.length >= 6,
    "The contract harness must include the required golden fixture cases",
  );
  const contractKinds = new Set<ContractKind>([
    "ContinuityEvent",
    "ContinuityArtifact",
    "ContinuityReflection",
    "ContinuityProof",
    "SignedRequestEnvelopeV1",
  ]);
  for (const path of fixtureFiles) {
    const fixture = await readJson(path);
    assert(
      isRecord(fixture),
      `Contract fixture must have an object root: ${path}`,
    );
    assert(
      Array.isArray(fixture.cases),
      `Contract fixture must contain a cases array: ${path}`,
    );
    assert(
      fixture.cases.length > 0,
      `Contract fixture must contain at least one case: ${path}`,
    );

    for (const [index, fixtureCase] of fixture.cases.entries()) {
      const location = `${path}#cases[${index}]`;
      assert(
        isRecord(fixtureCase),
        `Fixture case must be an object: ${location}`,
      );
      assert(
        typeof fixtureCase.name === "string",
        `Fixture case needs a name: ${location}`,
      );
      assert(
        typeof fixtureCase.contract === "string" &&
          contractKinds.has(fixtureCase.contract as ContractKind),
        `Fixture case has an unknown contract: ${location}`,
      );
      assert(
        typeof fixtureCase.expectedValid === "boolean",
        `Fixture case needs expectedValid: ${location}`,
      );
      const result = await validateContract(
        fixtureCase.contract as ContractKind,
        fixtureCase.value,
      );
      assert(
        result.valid === fixtureCase.expectedValid,
        `Fixture validation disagrees with expectedValid: ${location}`,
      );
      if (typeof fixtureCase.expectedIssueCode === "string") {
        assert(
          result.issues.some(
            (issue) => issue.code === fixtureCase.expectedIssueCode,
          ),
          `Fixture is missing expected issue ${fixtureCase.expectedIssueCode}: ${location}`,
        );
      }
    }
  }
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
    "{{INSTALL_COMMAND}}",
    "data-copy-command",
    "data-shot-toggle",
    "{{REPOSITORY_URL}}",
    "https://community.tohseno.com",
    "The fastest way to prototype iOS apps",
    "The open source app blueprint system for builders that have infinite ideas",
    "Get rid of your recurring thoughts",
    "INFINITE SHOTS.",
    "Copy one liner installer",
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
    "no telemetry",
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

async function validateOneshotPin(): Promise<void> {
  console.log("\n[check] canonical installer and legacy oneshot pin");

  const installer = await readText("apps/site/public/install.sh");
  assert(
    installer.startsWith("#!/bin/sh\n"),
    "install.sh must remain a portable POSIX shell script",
  );
  for (const phrase of [
    'CLI_VERSION="0.3.1"',
    'install_root="${TOHSENO_INSTALL_HOME:-$HOME/.tohseno}"',
    "TOHSENO_INSTALL_CLI_SHA256",
    "checksum mismatch",
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
    "install.sh must pin the prepared CLI artifact with a complete SHA-256 digest",
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

  const script = await readText("apps/site/public/oneshot.sh");
  const pinMatch = script.match(/^TOHSENO_PIN="([0-9a-f]{40})"$/m);
  assert(
    pinMatch !== null,
    "oneshot.sh must embed TOHSENO_PIN as a full 40-character commit hash",
  );
  const pin = pinMatch[1]!;
  assert(
    pin === "48bada35f885216c8c2bf3ab4d51d0c935e2e01e",
    "The thin oneshot must pin the exact published CLI 0.3.1 release commit",
  );
  const head = (await capture(["git", "rev-parse", "HEAD"])).trim();
  const committedScript = await capture([
    "git",
    "show",
    "HEAD:apps/site/public/oneshot.sh",
  ]);
  const parent = committedScript === script
    ? (await capture(["git", "rev-parse", "HEAD^"])).trim()
    : head;
  assert(
    pin === parent,
    "TOHSENO_PIN must be the direct parent of the current or pending serving commit",
  );
  const installerShaMatch = script.match(
    /^PINNED_INSTALLER_SHA256="([0-9a-f]{64})"$/m,
  );
  assert(
    installerShaMatch !== null,
    "oneshot.sh must pin the released installer's complete SHA-256 digest",
  );
  const pinnedInstaller = await capture([
    "git",
    "show",
    `${pin}:apps/site/public/install.sh`,
  ]);
  assert(
    createHash("sha256").update(pinnedInstaller).digest("hex") ===
      installerShaMatch[1],
    "oneshot.sh installer checksum does not match the pinned commit",
  );
  assert(
    pinnedInstaller.includes('CLI_VERSION="0.3.1"') &&
      pinnedInstaller.includes(
        'CLI_SHA256_DEFAULT="a8cbee45aacb658083c435298c4e83be062f0daa45c73951c837bc130ef37a5e"',
      ),
    "The pinned oneshot installer does not contain the published CLI 0.3.1 identity",
  );
  for (const phrase of [
    "thin entry point",
    "raw.githubusercontent.com/jpfraneto/tohseno/${TOHSENO_PIN}/apps/site/public/install.sh",
    "checksum mismatch for the pinned installer",
    '/bin/sh "$installer_path" "$@"',
  ]) {
    assert(
      script.includes(phrase),
      `oneshot.sh is missing pinned delegator behavior: ${phrase}`,
    );
  }
  for (const obsoleteCreatorStep of [
    'mkdir -p "$target"',
    'cp -R "$rails_dir/templates/continuity-app/."',
    'git -C "$target" init',
    'agent_cmd="$(first_agent)"',
  ]) {
    assert(
      !script.includes(obsoleteCreatorStep),
      `oneshot.sh must not regain the competing workspace creator step: ${obsoleteCreatorStep}`,
    );
  }
  for (const requiredCliFile of [
    "packages/cli/package.json",
    "packages/cli/src/bin.ts",
    "packages/cli/src/cli.ts",
  ]) {
    await readText(requiredCliFile);
  }

  try {
    await capture(["git", "merge-base", "--is-ancestor", pin, "HEAD"]);
  } catch {
    fail(
      `TOHSENO_PIN ${pin} is not an ancestor of HEAD. The pin must reference a ` +
        "published CLI commit that is already part of this history.",
    );
  }

  for (const required of [
    "templates/continuity-app/continuity.manifest.json",
    "templates/continuity-app/project.yml",
    "templates/continuity-app/Writing.xcodeproj/project.pbxproj",
    "templates/continuity-app/App/WritingApp.swift",
    "templates/continuity-app/App/AppConfig.swift",
    "templates/continuity-app/App/Identity/BIP39.swift",
    "templates/continuity-app/App/Resources/bip39-english.txt",
    "templates/continuity-app/Tests/BIP39Tests.swift",
    "templates/continuity-app/site/index.html",
    "templates/continuity-app/scripts/setup.ts",
    "templates/continuity-app/fastlane/Fastfile",
    "templates/continuity-app/README.md",
    "skills/continuity-app/SKILL.md",
  ]) {
    try {
      await capture(["git", "cat-file", "-e", `${pin}:${required}`]);
    } catch {
      fail(
        `Published CLI pin ${pin} is missing ${required}; its trust record is incomplete`,
      );
    }
  }

  // Keep the released CLI pin auditable against public main.
  // Network may legitimately be absent (offline dev, CI sandbox): skip with a
  // warning then, but if the remote answers, the pin must be reachable from
  // its main.
  let remoteMain = "";
  try {
    await capture([
      "git",
      "-c",
      "core.askPass=true",
      "fetch",
      "--quiet",
      "origin",
      "main",
    ]);
    remoteMain = (await capture(["git", "rev-parse", "FETCH_HEAD"])).trim();
  } catch {
    console.log(
      "  ! origin unreachable — skipped verifying the pin is published; run again online before releasing",
    );
  }
  if (remoteMain !== "") {
    try {
      await capture(["git", "merge-base", "--is-ancestor", pin, remoteMain]);
    } catch {
      fail(
        `TOHSENO_PIN ${pin} is not reachable from origin/main (${remoteMain.slice(0, 7)}). ` +
          "Push and publish the CLI release commit before deploying this thin installer.",
      );
    }
  }
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
  const paths = output.split("\0").filter(Boolean).sort();
  for (const path of paths) {
    const name = path.split("/").at(-1) ?? path;
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
  await validateOneshotPin();
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
