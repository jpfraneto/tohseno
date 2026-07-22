import { afterEach, describe, expect, test } from "bun:test";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  detectAppleTeamId,
  main,
  mergeLocalXcconfig,
  parseSetupOptions,
  validateAppStoreConnectCredentials,
  type AppStoreConnectConfig,
} from "../templates/continuity-app/scripts/setup.ts";

const SETUP_SCRIPT = join(import.meta.dir, "..", "templates", "continuity-app", "scripts", "setup.ts");
const TEST_KEY_ID = "KEYID12345";
const temporaryDirectories: string[] = [];

interface ProcessResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

function scratchWorkspace(
  displayName = "Voice Circle",
  bundleId = "com.example.voice-circle",
): string {
  const root = mkdtempSync(join(tmpdir(), "tohseno-setup-test-"));
  temporaryDirectories.push(root);
  mkdirSync(join(root, "scripts"));
  mkdirSync(join(root, "Config"));
  copyFileSync(SETUP_SCRIPT, join(root, "scripts", "setup.ts"));
  writeFileSync(
    join(root, "package.json"),
    `${JSON.stringify({ private: true, type: "module", scripts: { setup: "bun scripts/setup.ts" } }, null, 2)}\n`,
  );
  writeFileSync(
    join(root, "continuity.manifest.json"),
    `${JSON.stringify({ application: { name: displayName, id: bundleId } }, null, 2)}\n`,
  );
  return root;
}

async function runSetupProcess(
  root: string,
  arguments_: string[],
  input: string,
  environment: Record<string, string> = {},
): Promise<ProcessResult> {
  const child = Bun.spawn(["bun", "run", "setup", ...arguments_], {
    cwd: root,
    env: { ...process.env, ...environment },
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });
  child.stdin.write(input);
  child.stdin.end();
  const timeout = setTimeout(() => child.kill(), 5_000);
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ]);
  clearTimeout(timeout);
  return { exitCode, stdout, stderr };
}

function quietOutput(): { write(value: string): void; line(value?: string): void } {
  return { write() {}, line() {} };
}

function testKeyFilename(): string {
  return ["AuthKey", TEST_KEY_ID].join("_") + ".p8";
}

afterEach(() => {
  for (const path of temporaryDirectories.splice(0)) {
    rmSync(path, { recursive: true, force: true });
  }
});

describe("setup Local.xcconfig merge", () => {
  test("replaces only setup-owned assignments", () => {
    const existing = [
      "// owner note",
      "DEV_SECRET = machine-local-placeholder",
      "APP_DISPLAY_NAME = Old Name",
      "CUSTOM_FLAG = YES",
      "",
    ].join("\n");
    const merged = mergeLocalXcconfig(existing, {
      APP_DISPLAY_NAME: "New Name",
      APP_BUNDLE_ID: "com.example.new-name",
      DEVELOPMENT_TEAM: "ABCDE12345",
      REVENUECAT_PUBLIC_KEY: "",
    });

    expect(merged).toContain("DEV_SECRET = machine-local-placeholder");
    expect(merged).toContain("CUSTOM_FLAG = YES");
    expect(merged).toContain("APP_DISPLAY_NAME = New Name");
    expect(merged.match(/^APP_DISPLAY_NAME\s*=/gm)).toHaveLength(1);
    expect(merged).not.toContain("Old Name");
  });
});

describe("setup command", () => {
  test("interactive setup uses manifest defaults, exits, and round-trips prior answers", async () => {
    const root = scratchWorkspace();
    writeFileSync(
      join(root, "Config", "Local.xcconfig"),
      "// keep this comment\nDEV_SECRET = machine-local-placeholder\nAPP_DISPLAY_NAME = Stale\n",
    );
    const environment = { TOHSENO_APPLE_TEAM_ID: "ABCDE12345" };

    const first = await runSetupProcess(
      root,
      [],
      "Party Table\ncom.example.party-table\n\n\n\n",
      environment,
    );
    expect(first.exitCode).toBe(0);
    expect(first.stderr).not.toContain("Setup failed");
    expect(first.stdout).toContain("[Voice Circle]");
    expect(first.stdout).toContain("[com.partytable.app]");

    const firstConfig = JSON.parse(readFileSync(join(root, "app.config.json"), "utf8")) as {
      displayName: string;
      bundleId: string;
      teamId: string;
    };
    expect(firstConfig).toEqual({
      displayName: "Party Table",
      bundleId: "com.example.party-table",
      teamId: "ABCDE12345",
    });

    const second = await runSetupProcess(root, [], "\n\n\n\n\n", environment);
    expect(second.exitCode).toBe(0);
    expect(second.stderr).not.toContain("Setup failed");
    expect(second.stdout).toContain("[Party Table]");
    expect(second.stdout).toContain("[com.example.party-table]");
    expect(readFileSync(join(root, "app.config.json"), "utf8")).toBe(
      `${JSON.stringify(firstConfig, null, 2)}\n`,
    );

    const localConfig = readFileSync(join(root, "Config", "Local.xcconfig"), "utf8");
    expect(localConfig).toContain("// keep this comment");
    expect(localConfig).toContain("DEV_SECRET = machine-local-placeholder");
    expect(localConfig).toContain("APP_DISPLAY_NAME = Party Table");
    expect(localConfig.match(/^APP_DISPLAY_NAME\s*=/gm)).toHaveLength(1);
  });

  test("non-interactive setup takes app identity from the manifest", async () => {
    const root = scratchWorkspace(
      "Realtime Voice Host Around The Table",
      "com.example.realtime-host",
    );
    const result = await runSetupProcess(
      root,
      ["--from-manifest", "--team", "ABCDE12345"],
      "",
    );

    expect(result.exitCode).toBe(0);
    expect(result.stderr).not.toContain("Setup failed");
    expect(JSON.parse(readFileSync(join(root, "app.config.json"), "utf8"))).toEqual({
      displayName: "Realtime Voice Host Around The Table",
      bundleId: "com.example.realtime-host",
      teamId: "ABCDE12345",
    });

    const firstConfig = readFileSync(join(root, "app.config.json"), "utf8");
    const second = await runSetupProcess(
      root,
      ["--from-manifest", "--team", "ABCDE12345"],
      "",
    );
    expect(second.exitCode).toBe(0);
    expect(readFileSync(join(root, "app.config.json"), "utf8")).toBe(firstConfig);
  });

  test("environment equivalents enable non-interactive mode and expand the key path", async () => {
    const root = scratchWorkspace();
    const home = join(root, "owner-home");
    mkdirSync(home);
    const keyPath = join(home, testKeyFilename());
    writeFileSync(keyPath, "test-only-key-file");
    let validated: AppStoreConnectConfig | undefined;
    const environment = {
      HOME: home,
      TOHSENO_FROM_MANIFEST: "1",
      TOHSENO_APPLE_TEAM_ID: "ABCDE12345",
      TOHSENO_ASC_KEY_PATH: `~/${testKeyFilename()}`,
      TOHSENO_ASC_ISSUER_ID: "00000000-0000-4000-8000-000000000000",
    };

    await main([], {
      root,
      environment,
      output: quietOutput(),
      validateAppStoreConnect: async (config) => {
        validated = config;
      },
    });

    expect(parseSetupOptions([], environment)).toMatchObject({
      fromManifest: true,
      team: "ABCDE12345",
      ascKeyPath: `~/${testKeyFilename()}`,
      ascIssuerId: "00000000-0000-4000-8000-000000000000",
    });
    expect(validated).toEqual({
      keyPath,
      keyId: TEST_KEY_ID,
      issuerId: "00000000-0000-4000-8000-000000000000",
    });
  });

  test("failed App Store Connect validation writes neither config file", async () => {
    const root = scratchWorkspace();
    const keyPath = join(root, testKeyFilename());
    writeFileSync(keyPath, "test-only-key-file");
    const priorAppConfig = "{\"sentinel\":true}\n";
    const priorLocalConfig = "FOREIGN_SLOT = preserved\n";
    writeFileSync(join(root, "app.config.json"), priorAppConfig);
    writeFileSync(join(root, "Config", "Local.xcconfig"), priorLocalConfig);

    await expect(main([
      "--from-manifest",
      "--team",
      "ABCDE12345",
      "--asc-key",
      keyPath,
      "--asc-key-id",
      TEST_KEY_ID,
      "--asc-issuer-id",
      "00000000-0000-4000-8000-000000000000",
    ], {
      root,
      output: quietOutput(),
      validateAppStoreConnect: async () => {
        throw new Error("rejected for test");
      },
    })).rejects.toThrow("rejected for test");

    expect(readFileSync(join(root, "app.config.json"), "utf8")).toBe(priorAppConfig);
    expect(readFileSync(join(root, "Config", "Local.xcconfig"), "utf8")).toBe(priorLocalConfig);
  });
});

describe("Apple setup integration helpers", () => {
  test("prefers Xcode's last selected Team ID without exposing account output", async () => {
    const commands: string[][] = [];
    const detected = await detectAppleTeamId(async (command) => {
      commands.push(command);
      return { exitCode: 0, stdout: "ABCDE12345\n" };
    }, undefined);

    expect(detected).toEqual({
      teamId: "ABCDE12345",
      source: "Xcode's selected account",
    });
    expect(commands).toHaveLength(1);
  });

  test("builds a 64-byte ES256 JWT and performs the read-only apps request", async () => {
    const root = scratchWorkspace();
    const keyPair = await crypto.subtle.generateKey(
      { name: "ECDSA", namedCurve: "P-256" },
      true,
      ["sign", "verify"],
    );
    const privateKey = new Uint8Array(await crypto.subtle.exportKey("pkcs8", keyPair.privateKey));
    const encoded = Buffer.from(privateKey).toString("base64").match(/.{1,64}/g)?.join("\n") ?? "";
    const keyPath = join(root, testKeyFilename());
    writeFileSync(
      keyPath,
      `-----BEGIN PRIVATE KEY-----\n${encoded}\n-----END PRIVATE KEY-----\n`,
    );
    let authorization = "";
    let requestedUrl = "";

    await validateAppStoreConnectCredentials({
      keyPath,
      keyId: TEST_KEY_ID,
      issuerId: "00000000-0000-4000-8000-000000000000",
    }, async (input, init) => {
      requestedUrl = String(input);
      authorization = new Headers(init?.headers).get("Authorization") ?? "";
      return new Response("{}", { status: 200 });
    });

    expect(requestedUrl).toBe("https://api.appstoreconnect.apple.com/v1/apps?limit=1");
    expect(authorization.startsWith("Bearer ")).toBe(true);
    const [header, payload, signature] = authorization.slice("Bearer ".length).split(".");
    expect(JSON.parse(Buffer.from(header!, "base64url").toString("utf8"))).toEqual({
      alg: "ES256",
      kid: TEST_KEY_ID,
      typ: "JWT",
    });
    const claims = JSON.parse(Buffer.from(payload!, "base64url").toString("utf8")) as {
      iss: string;
      iat: number;
      exp: number;
      aud: string;
    };
    expect(claims.iss).toBe("00000000-0000-4000-8000-000000000000");
    expect(claims.aud).toBe("appstoreconnect-v1");
    expect(claims.exp - claims.iat).toBe(19 * 60);
    expect(Buffer.from(signature!, "base64url")).toHaveLength(64);
  });
});
