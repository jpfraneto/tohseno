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
  if (!existsSync(directory)) fail(`Required directory is missing: ${relativeDirectory}`);

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

  const manifestPaths = [
    "examples/anky/continuity.manifest.json",
    "examples/daily-observation/continuity.manifest.json",
    "templates/continuity-app/continuity.manifest.json",
  ];
  for (const path of manifestPaths) {
    const result = validateManifest(await readJson(path));
    if (!result.valid) {
      const locations = result.errors.map((issue) => `${issue.path} (${issue.code})`).join(", ");
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
    assert(typeof schema.$id === "string", `Contract schema must declare an $id: ${path}`);
  }

  const fixtureFiles = listJsonFiles("packages/contracts/fixtures");
  assert(fixtureFiles.length >= 6, "The contract harness must include the required golden fixture cases");
  const contractKinds = new Set<ContractKind>([
    "ContinuityEvent",
    "ContinuityArtifact",
    "ContinuityReflection",
    "ContinuityProof",
    "SignedRequestEnvelopeV1",
  ]);
  for (const path of fixtureFiles) {
    const fixture = await readJson(path);
    assert(isRecord(fixture), `Contract fixture must have an object root: ${path}`);
    assert(Array.isArray(fixture.cases), `Contract fixture must contain a cases array: ${path}`);
    assert(fixture.cases.length > 0, `Contract fixture must contain at least one case: ${path}`);

    for (const [index, fixtureCase] of fixture.cases.entries()) {
      const location = `${path}#cases[${index}]`;
      assert(isRecord(fixtureCase), `Fixture case must be an object: ${location}`);
      assert(typeof fixtureCase.name === "string", `Fixture case needs a name: ${location}`);
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
          result.issues.some((issue) => issue.code === fixtureCase.expectedIssueCode),
          `Fixture is missing expected issue ${fixtureCase.expectedIssueCode}: ${location}`,
        );
      }
    }
  }
}

async function validateStaticSurface(): Promise<void> {
  console.log("\n[check] static and deployment surface");

  const index = await readText("apps/site/public/index.html");
  const intake = await readText("apps/site/public/intake.html");
  const privacy = await readText("apps/site/public/privacy.html");
  const robots = await readText("apps/site/public/robots.txt");
  const htmlPages: Array<[string, string]> = [
    ["landing", index],
    ["intake", intake],
    ["privacy", privacy],
  ];
  const codeAssets = [
    await readText("apps/site/public/styles.css"),
    await readText("apps/site/public/app.js"),
  ];

  for (const required of [
    "{{ONESHOT_COMMAND}}",
    "data-copy-command",
    "{{REPOSITORY_URL}}",
    'href="/intake"',
    'href="/privacy"',
  ]) {
    assert(index.includes(required), `Landing page is missing required hero contract: ${required}`);
  }
  assert(!index.includes("<form"), "Landing page must stay a single hero without an intake form");

  for (const required of [
    'action="/api/submissions"',
    'name="markdown"',
    'name="email"',
    'name="operatingMode"',
    'value="self-hosted"',
    'value="client-owned"',
    'value="anky-operated"',
    'href="/privacy"',
  ]) {
    assert(intake.includes(required), `Intake page is missing required form contract: ${required}`);
  }
  for (const [label, page] of htmlPages) {
    assert(!/<script(?![^>]*\bsrc=)[^>]*>/iu.test(page), `The ${label} page must not use inline scripts`);
    assert(
      !/<(?:script|link|img|iframe|frame|embed|object|source|video|audio|form)\b[^>]*\b(?:src|href|action|data)\s*=\s*["'](?:https?:)?\/\//iu.test(page),
      `The ${label} page must not load resources from or submit to another origin`,
    );
  }
  assert(index.includes('src="/app.js"'), "Landing JavaScript must be a separate same-origin asset");
  assert(intake.includes('src="/app.js"'), "Intake JavaScript must be a separate same-origin asset");
  assert(
    !codeAssets.some((source) =>
      /\bhttps?:\/\/|(?:src|href)\s*=\s*["']\/\/|url\(\s*["']?\/\//iu.test(source)
    ),
    "Public style and script assets must not reference other origins",
  );

  for (const phrase of [
    "encrypted at rest",
    "bearer capabilities",
    "Anky, Inc.",
    "support@anky.app",
  ]) {
    assert(privacy.includes(phrase), `Privacy page is missing required disclosure: ${phrase}`);
  }
  for (const privateRoute of ["Disallow: /api/", "Disallow: /c", "Disallow: /status"]) {
    assert(robots.includes(privateRoute), `robots.txt is missing: ${privateRoute}`);
  }

  const environmentExample = await readText(".env.example");
  for (const variable of [
    "NODE_ENV",
    "PORT",
    "BASE_URL",
    "DATABASE_PATH",
    "TOHSENO_BACKUP_PATH",
    "TRUST_PROXY",
    "TOHSENO_DATA_KEY",
    "TOHSENO_OPERATOR_TOKEN",
    "PAYMENTS_MODE",
    "STRIPE_SECRET_KEY",
    "STRIPE_WEBHOOK_SECRET",
    "STRIPE_SELF_HOSTED_PRICE_ID",
    "STRIPE_CLIENT_SETUP_PRICE_ID",
    "STRIPE_CLIENT_MONTHLY_PRICE_ID",
    "EMAIL_MODE",
    "RESEND_API_KEY",
    "EMAIL_FROM",
  ]) {
    assert(
      new RegExp(`^${variable}=`, "mu").test(environmentExample),
      `.env.example is missing ${variable}`,
    );
  }

  const dockerfile = await readText("Dockerfile");
  assert(dockerfile.includes("FROM oven/bun:"), "Dockerfile must use the official Bun image");
  assert(dockerfile.includes("ENTRYPOINT"), "Production container must initialize its persistent volume through an entrypoint");
  assert(dockerfile.includes("su-exec"), "Production container must drop privileges before the Bun process starts");
  const entrypoint = await readText("scripts/container-entrypoint.sh");
  assert(entrypoint.includes("chown -R bun:bun /data"), "Container entrypoint must initialize mounted-volume ownership recursively");
  assert(entrypoint.includes("chmod -R go-rwx /data"), "Container entrypoint must remove group/world access from persisted data");
  assert(entrypoint.includes("umask 0077"), "Container entrypoint must create persisted data as owner-only");
  assert(entrypoint.includes('exec su-exec bun "$@"'), "Container entrypoint must run Bun as the non-root user");
  assert(
    !/^\s*VOLUME\b/m.test(dockerfile),
    "Dockerfile must use the Railway-mounted volume instead of an unsupported Docker VOLUME declaration",
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
  assert(railwayConfig.build.builder === "DOCKERFILE", "Railway must use the Dockerfile builder");
  assert(
    railwayConfig.deploy.startCommand === undefined,
    "Railway must preserve the Docker ENTRYPOINT instead of overriding it with a start command",
  );
  assert(railwayConfig.deploy.healthcheckPath === "/healthz", "Railway health check path is incorrect");
  assert(railwayConfig.deploy.restartPolicyType === "ON_FAILURE", "Railway restart policy is incorrect");
}

async function validateOneshotPin(): Promise<void> {
  console.log("\n[check] oneshot bootstrap pin");

  const script = await readText("apps/site/public/oneshot.sh");
  const pinMatch = script.match(/^TOHSENO_PIN="([0-9a-f]{40})"$/m);
  assert(pinMatch !== null, "oneshot.sh must embed TOHSENO_PIN as a full 40-character commit hash");
  const pin = pinMatch[1]!;

  try {
    await capture(["git", "merge-base", "--is-ancestor", pin, "HEAD"]);
  } catch {
    fail(
      `TOHSENO_PIN ${pin} is not an ancestor of HEAD. The pin must reference a ` +
        "released rails commit that is already part of this history; bump it only " +
        "in a follow-up commit after the release it points to.",
    );
  }

  for (const required of [
    "templates/continuity-app/MASTER_PROMPT.md",
    "templates/continuity-app/continuity.manifest.json",
    "templates/continuity-app/EVOLUTION.md",
    "templates/continuity-app/OPERATOR.md",
    "templates/continuity-app/README.md",
    "examples/anky/MASTER_PROMPT.md",
    "examples/anky/continuity.manifest.json",
    "examples/daily-observation/MASTER_PROMPT.md",
    "examples/daily-observation/continuity.manifest.json",
    "skills/continuity-app/SKILL.md",
  ]) {
    try {
      await capture(["git", "cat-file", "-e", `${pin}:${required}`]);
    } catch {
      fail(`Pinned rails commit ${pin} is missing ${required}; the oneshot workspace would be incomplete`);
    }
  }
}

async function validateRepositoryHygiene(): Promise<void> {
  console.log("\n[check] tracked/unignored file and secret hygiene");
  const output = await capture(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]);
  const paths = output.split("\0").filter(Boolean).sort();
  for (const path of paths) {
    const name = path.split("/").at(-1) ?? path;
    if (name.startsWith(".env") && name !== ".env.example") fail(`Environment file must not be tracked or unignored: ${path}`);
    if (/\.(?:sqlite(?:-wal|-shm)?|db|pem|p12|pfx)$/i.test(name) || /(^|\/)data\//.test(path)) {
      fail(`Private runtime/credential file must not be tracked or unignored: ${path}`);
    }
  }

  const secretPatterns: Array<[string, RegExp]> = [
    ["Stripe live secret", /STRIPE_SECRET_KEY\s*=\s*sk_live_[A-Za-z0-9]{20,}/],
    ["Stripe webhook secret", /STRIPE_WEBHOOK_SECRET\s*=\s*whsec_[A-Za-z0-9]{24,}/],
    ["Resend API key", /RESEND_API_KEY\s*=\s*re_[A-Za-z0-9]{24,}/],
    ["TOHSENO data key", /TOHSENO_DATA_KEY\s*=\s*[A-Za-z0-9+/]{43}=/],
    ["TOHSENO operator token", /TOHSENO_OPERATOR_TOKEN\s*=\s*[^\s#]{32,}/],
    ["private key block", /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----\s*\n(?:[A-Za-z0-9+/=]{20,}\s*\n){2,}/],
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
  const message = error instanceof Error ? error.message : "Unknown check failure";
  console.error(`\n[check] ${message}`);
  process.exitCode = 1;
}
