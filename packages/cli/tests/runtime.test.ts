import { describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { Database } from "bun:sqlite";
import { main } from "../src/cli.ts";
import {
  delay,
  isProcessAlive,
  parseQuickTunnelUrl,
  waitForProcessExit,
} from "../factory/runtime/shared.ts";
import { inspectEndpoint, inspectProduction } from "../factory/runtime/production.ts";
import {
  createMemoryIo,
  REPOSITORY_ROOT,
  runGit,
  runProcess,
  withScratchEnvironment,
  writeExecutable,
  type ScratchEnvironment,
} from "./helpers.ts";

interface Envelope {
  schemaVersion: 1;
  ok: boolean;
  operation: string;
  shot: string | null;
  result?: any;
  error?: { code: string; message: string; details?: any };
}

async function createShot(scratch: ScratchEnvironment, slug: string): Promise<string> {
  const io = createMemoryIo();
  const exitCode = await main([
    "create", slug, "--platform", "ios", "--no-launch", "--no-interactive",
  ], {
    cwd: scratch.root,
    environment: scratch.environment,
    io,
    sourceRoot: REPOSITORY_ROOT,
  });
  if (exitCode !== 0) throw new Error(io.stderr.join("\n"));
  return join(scratch.shotsDirectory, slug);
}

async function machine(
  scratch: ScratchEnvironment,
  shot: string,
  arguments_: readonly string[],
): Promise<{ exitCode: number; envelope: Envelope; stdout: string[]; stderr: string[] }> {
  const io = createMemoryIo();
  const exitCode = await main(["machine", ...arguments_, "--json"], {
    cwd: shot,
    environment: scratch.environment,
    io,
  });
  expect(io.stdout).toHaveLength(1);
  const envelope = JSON.parse(io.stdout[0]!) as Envelope;
  expect(envelope.schemaVersion).toBe(1);
  return { exitCode, envelope, stdout: io.stdout, stderr: io.stderr };
}

async function stopQuietly(scratch: ScratchEnvironment, shot: string): Promise<void> {
  try {
    await machine(scratch, shot, ["dev", "stop"]);
  } catch {
    // Scratch cleanup remains the final guard in a failed test.
  }
}

describe("shot-local machine runtime", () => {
  test("acceptance flow starts API/SQLite, is idempotent, reports status/logs, verifies, and stops cleanly", async () => {
    await withScratchEnvironment(async (scratch) => {
      const privateSentinel = "runtime-secret-must-never-appear-6f17";
      scratch.environment.OPENAI_API_KEY = privateSentinel;
      scratch.environment.DEV_SECRET = privateSentinel;
      const shot = await createShot(scratch, "runtime-acceptance");
      expect(existsSync(join(shot, "Backend", "server.ts"))).toBe(true);
      expect(existsSync(join(shot, ".tohseno", "machine.ts"))).toBe(true);

      try {
        const started = await machine(scratch, shot, ["dev", "start"]);
        expect(started.exitCode).toBe(0);
        expect(started.stderr).toEqual([]);
        expect(started.envelope.ok).toBe(true);
        expect(started.envelope.result).toMatchObject({
          state: "running",
          healthy: true,
          api: { running: true, healthy: true },
          tunnel: { requested: false },
          endpoint: { configured: true },
        });
        const instanceId = started.envelope.result.instanceId as string;
        const apiPid = started.envelope.result.api.pid as number;
        const supervisorPid = JSON.parse(readFileSync(join(shot, ".tohseno", "run", "state.json"), "utf8")).supervisor.pid as number;
        const health = await (await fetch(started.envelope.result.api.healthUrl)).json() as Record<string, unknown>;
        expect(health).toMatchObject({ status: "ok", ready: true, service: "shot-api" });

        const databasePath = join(shot, ".tohseno", "data", "development.sqlite3");
        expect(existsSync(databasePath)).toBe(true);
        const database = new Database(databasePath, { readonly: true });
        expect(database.query<{ version: number }, []>("SELECT version FROM schema_migrations").get()?.version).toBe(1);
        database.close();
        expect(readFileSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"), "utf8"))
          .toContain("TOHSENO_API_BASE_URL = http:/$()/127.0.0.1:");

        const repeated = await machine(scratch, shot, ["dev", "start"]);
        expect(repeated.exitCode).toBe(0);
        expect(repeated.envelope.result.instanceId).toBe(instanceId);
        expect(repeated.envelope.result.api.pid).toBe(apiPid);

        const status = await machine(scratch, shot, ["dev", "status"]);
        expect(status.exitCode).toBe(0);
        expect(status.envelope.result).toMatchObject({ state: "running", healthy: true, instanceId });

        const logs = await machine(scratch, shot, ["dev", "logs", "--service", "all", "--lines", "50"]);
        expect(logs.exitCode).toBe(0);
        expect(logs.envelope.result.logs.api.join("\n")).toContain('"event":"startup"');
        expect(JSON.stringify(logs.envelope)).not.toContain(privateSentinel);
        expect(readFileSync(join(shot, ".tohseno", "run", "logs", "api.log"), "utf8"))
          .not.toContain(privateSentinel);

        const verification = await machine(scratch, shot, ["verify"]);
        expect(verification.exitCode).toBe(0);
        expect(verification.envelope.result.valid).toBe(true);
        expect(verification.stdout).toHaveLength(1);

        const production = await machine(scratch, shot, ["production", "inspect"]);
        expect(production.exitCode).toBe(0);
        expect(production.envelope.result.productionReady).toBe(false);
        expect(production.envelope.result.blockers).toContain("production API endpoint is not configured");
        expect(production.envelope.result.capabilities.proposed).toContain("production.deploy");

        const gitStatus = await runGit(["status", "--porcelain"], shot, scratch.environment);
        expect(gitStatus.stdout).toBe("");
        for (const path of [
          ".tohseno/data/development.sqlite3",
          ".tohseno/run/state.json",
          ".tohseno/run/logs/api.log",
          "Config/DevelopmentEndpoint.xcconfig",
        ]) {
          expect((await runGit(["check-ignore", "--quiet", "--no-index", path], shot, scratch.environment)).exitCode, path).toBe(0);
        }

        const stopped = await machine(scratch, shot, ["dev", "stop"]);
        expect(stopped.exitCode).toBe(0);
        expect(stopped.envelope.result.stopped).toBe(true);
        expect(await waitForProcessExit(supervisorPid, 2_000)).toBe(true);
        expect(await waitForProcessExit(apiPid, 2_000)).toBe(true);
        expect(existsSync(join(shot, ".tohseno", "run", "state.json"))).toBe(false);
        expect(existsSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"))).toBe(false);
        expect(existsSync(databasePath)).toBe(true);
        expect(existsSync(join(shot, ".tohseno", "run", "logs", "api.log"))).toBe(true);

        const stoppedAgain = await machine(scratch, shot, ["dev", "stop"]);
        expect(stoppedAgain.exitCode).toBe(0);
        expect(stoppedAgain.envelope.result.stopped).toBe(false);
      } finally {
        await stopQuietly(scratch, shot);
      }
    });
  }, 30_000);

  test("supervises a realistic fake Quick Tunnel and cleans partial tunnel failure", async () => {
    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "tunnel-success");
      const cloudflared = writeExecutable(scratch.binDirectory, "cloudflared", [
        "#!/bin/sh",
        "printf '%s\\n' '2026-07-22T12:00:00Z INF Requesting new quick Tunnel on trycloudflare.com...' >&2",
        "printf '%s\\n' '2026-07-22T12:00:01Z INF + https://gentle-river-42.trycloudflare.com +' >&2",
        "trap 'exit 0' TERM INT",
        "while :; do sleep 1; done",
      ].join("\n"));
      try {
        const started = await machine(scratch, shot, ["dev", "start", "--tunnel", "--cloudflared", cloudflared]);
        expect(started.exitCode).toBe(0);
        expect(started.envelope.result.tunnel).toMatchObject({
          requested: true,
          running: true,
          url: "https://gentle-river-42.trycloudflare.com",
          developmentOnly: true,
        });
        expect(readFileSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"), "utf8"))
          .toContain("https:/$()/gentle-river-42.trycloudflare.com");
        expect(readFileSync(join(shot, "Config", "Production.xcconfig"), "utf8"))
          .not.toContain("gentle-river-42");
      } finally {
        await stopQuietly(scratch, shot);
      }

      rmSync(cloudflared, { force: true });
      scratch.environment.PATH = [scratch.binDirectory, "/usr/bin", "/bin"].join(":");
      const missing = await machine(scratch, shot, ["dev", "start", "--tunnel"]);
      expect(missing.exitCode).toBe(3);
      expect(missing.envelope.error?.code).toBe("MISSING_DEPENDENCY");

      const failing = writeExecutable(scratch.binDirectory, "cloudflared", "#!/bin/sh\nprintf 'tunnel failed before URL\\n' >&2\nexit 17");
      const failed = await machine(scratch, shot, [
        "dev", "start", "--tunnel", "--cloudflared", failing, "--readiness-timeout-ms", "500",
      ]);
      expect(failed.exitCode).toBe(4);
      expect(failed.envelope.error?.code).toBe("UNHEALTHY_SERVICES");
      expect(failed.stdout).toHaveLength(1);
      expect(failed.stderr).toEqual([]);
      expect(existsSync(join(shot, ".tohseno", "run", "state.json"))).toBe(false);
      expect(existsSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"))).toBe(false);
      expect((await machine(scratch, shot, ["dev", "status"])).envelope.result.state).toBe("stopped");
    });
  }, 30_000);

  test("reports an unexpectedly exited API as unhealthy with stable JSON and exit code", async () => {
    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "unhealthy-status");
      try {
        const started = await machine(scratch, shot, ["dev", "start"]);
        const apiPid = started.envelope.result.api.pid as number;
        const state = JSON.parse(readFileSync(join(shot, ".tohseno", "run", "state.json"), "utf8"));
        const supervisorPid = state.supervisor.pid as number;
        process.kill(apiPid, "SIGKILL");
        expect(await waitForProcessExit(apiPid, 2_000)).toBe(true);
        expect(await waitForProcessExit(supervisorPid, 2_000)).toBe(true);
        await delay(50);

        const status = await machine(scratch, shot, ["dev", "status"]);
        expect(status.exitCode).toBe(4);
        expect(status.stdout).toHaveLength(1);
        expect(status.envelope).toMatchObject({
          ok: false,
          operation: "dev.status",
          error: {
            code: "UNHEALTHY_SERVICES",
            details: { status: { state: "unhealthy", healthy: false } },
          },
        });
        expect(existsSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"))).toBe(false);
      } finally {
        await stopQuietly(scratch, shot);
      }
    });
  }, 20_000);

  test("reports an unavailable simulator without undoing a healthy shot or API", async () => {
    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "no-simulator");
      writeExecutable(scratch.binDirectory, "xcodebuild", "#!/bin/sh\nexit 0");
      writeExecutable(scratch.binDirectory, "xcrun", [
        "#!/bin/sh",
        "if [ \"${1:-}\" = simctl ] && [ \"${2:-}\" = list ]; then",
        "  printf '%s\\n' '{\"devices\":{}}'",
        "  exit 0",
        "fi",
        "exit 1",
      ].join("\n"));
      try {
        const started = await machine(scratch, shot, ["dev", "start"]);
        expect(started.exitCode).toBe(0);
        const inspected = await machine(scratch, shot, ["ios", "inspect"]);
        expect(inspected.exitCode).toBe(0);
        expect(inspected.envelope.result).toMatchObject({
          xcode: { available: true },
          simulator: { available: false, devices: [] },
          development: { healthy: true },
        });

        const launched = await machine(scratch, shot, ["ios", "launch"]);
        expect(launched.exitCode).toBe(3);
        expect(launched.envelope.error?.code).toBe("MISSING_DEPENDENCY");
        expect(launched.envelope.error?.message).toContain("no available iPhone simulator");
        expect((await machine(scratch, shot, ["dev", "status"])).envelope.result.healthy).toBe(true);
      } finally {
        await stopQuietly(scratch, shot);
      }
    });
  }, 20_000);

  test("can stop an in-progress partial start and leaves no owned processes or endpoint state", async () => {
    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "stop-during-start");
      const cloudflared = writeExecutable(scratch.binDirectory, "cloudflared", [
        "#!/bin/sh",
        "printf '%s\\n' 'INF waiting for a fixture tunnel allocation' >&2",
        "trap 'exit 0' TERM INT",
        "while :; do sleep 1; done",
      ].join("\n"));
      const starting = machine(scratch, shot, [
        "dev", "start", "--tunnel", "--cloudflared", cloudflared, "--readiness-timeout-ms", "10000",
      ]);

      const lockPath = join(shot, ".tohseno", "run", "start.lock", "owner.json");
      const readyPath = join(shot, ".tohseno", "run", "api-ready.json");
      const deadline = Date.now() + 3_000;
      while ((!existsSync(lockPath) || !existsSync(readyPath)) && Date.now() < deadline) await delay(25);
      expect(existsSync(lockPath)).toBe(true);
      expect(existsSync(readyPath)).toBe(true);
      const supervisorPid = JSON.parse(readFileSync(lockPath, "utf8")).pid as number;
      const apiPid = JSON.parse(readFileSync(readyPath, "utf8")).pid as number;

      const stopped = await machine(scratch, shot, ["dev", "stop"]);
      const failedStart = await starting;
      expect(stopped.exitCode).toBe(0);
      expect(stopped.envelope.result.stopped).toBe(true);
      expect(failedStart.exitCode).toBe(4);
      expect(failedStart.envelope.error?.code).toBe("UNHEALTHY_SERVICES");
      expect(await waitForProcessExit(supervisorPid, 2_000)).toBe(true);
      expect(await waitForProcessExit(apiPid, 2_000)).toBe(true);
      expect(existsSync(join(shot, ".tohseno", "run", "state.json"))).toBe(false);
      expect(existsSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"))).toBe(false);
    });
  }, 20_000);

  test("refuses runtime directories redirected outside the shot", async () => {
    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "runtime-boundary");
      const outside = join(scratch.root, "outside runtime target");
      mkdirSync(outside);
      symlinkSync(outside, join(shot, ".tohseno", "run"), "dir");

      const status = await machine(scratch, shot, ["dev", "status"]);
      expect(status.exitCode).toBe(2);
      expect(status.envelope.error?.code).toBe("INVALID_CONFIGURATION");
      expect(status.envelope.error?.message).toContain("real directory inside the shot");
      expect(readdirSync(outside)).toEqual([]);
    });
  });

  test("refuses symlinked verifier and production configuration files", async () => {
    await withScratchEnvironment(async (scratch) => {
      const productionShot = await createShot(scratch, "production-file-boundary");
      const outsideProduction = join(scratch.root, "outside-production.xcconfig");
      writeFileSync(outsideProduction, "PRODUCTION_API_BASE_URL = https:/$()/api.example.com\n");
      rmSync(join(productionShot, "Config", "Production.xcconfig"));
      symlinkSync(outsideProduction, join(productionShot, "Config", "Production.xcconfig"));
      const production = await machine(scratch, productionShot, ["production", "inspect"]);
      expect(production.exitCode).toBe(2);
      expect(production.envelope.error?.code).toBe("INVALID_CONFIGURATION");
      expect(production.envelope.error?.message).toContain("must be a regular file");

      const verifierShot = await createShot(scratch, "verifier-file-boundary");
      const outsideVerifier = join(scratch.root, "outside-verifier.ts");
      writeFileSync(outsideVerifier, "process.exit(0);\n");
      rmSync(join(verifierShot, ".tohseno", "verify.ts"));
      symlinkSync(outsideVerifier, join(verifierShot, ".tohseno", "verify.ts"));
      const verified = await machine(scratch, verifierShot, ["verify"]);
      expect(verified.exitCode).toBe(2);
      expect(verified.envelope.error?.code).toBe("INVALID_CONFIGURATION");
      expect(verified.envelope.error?.message).toContain("shot-local verifier must be a regular file");
    });
  });

  test("concurrent starts converge and multiple shots use independent processes and ports", async () => {
    await withScratchEnvironment(async (scratch) => {
      const first = await createShot(scratch, "simultaneous-one");
      const second = await createShot(scratch, "simultaneous-two");
      try {
        const [left, right] = await Promise.all([
          machine(scratch, first, ["dev", "start"]),
          machine(scratch, first, ["dev", "start"]),
        ]);
        expect(left.exitCode).toBe(0);
        expect(right.exitCode).toBe(0);
        expect(left.envelope.result.instanceId).toBe(right.envelope.result.instanceId);
        expect(left.envelope.result.api.pid).toBe(right.envelope.result.api.pid);

        const other = await machine(scratch, second, ["dev", "start"]);
        expect(other.exitCode).toBe(0);
        expect(other.envelope.result.instanceId).not.toBe(left.envelope.result.instanceId);
        expect(other.envelope.result.api.port).not.toBe(left.envelope.result.api.port);
        expect(other.envelope.result.api.pid).not.toBe(left.envelope.result.api.pid);
      } finally {
        await Promise.all([stopQuietly(scratch, first), stopQuietly(scratch, second)]);
      }
    });
  }, 30_000);

  test("readiness failure cleans partial state, stale PIDs recover, and stop refuses an unrelated PID", async () => {
    await withScratchEnvironment(async (scratch) => {
      const broken = await createShot(scratch, "broken-api");
      writeFileSync(join(broken, "Backend", "server.ts"), "console.error(JSON.stringify({event: 'fixture_failure'})); process.exit(19);\n");
      const failed = await machine(scratch, broken, ["dev", "start", "--readiness-timeout-ms", "500"]);
      expect(failed.exitCode).toBe(4);
      expect(failed.envelope.error?.code).toBe("UNHEALTHY_SERVICES");
      expect(existsSync(join(broken, ".tohseno", "run", "state.json"))).toBe(false);
      expect(existsSync(join(broken, "Config", "DevelopmentEndpoint.xcconfig"))).toBe(false);

      const stale = await createShot(scratch, "stale-runtime");
      try {
        const original = await machine(scratch, stale, ["dev", "start"]);
        const oldInstance = original.envelope.result.instanceId;
        const oldState = JSON.parse(readFileSync(join(stale, ".tohseno", "run", "state.json"), "utf8"));
        await machine(scratch, stale, ["dev", "stop"]);
        oldState.supervisor.pid = 999_999;
        oldState.api.pid = 999_998;
        writeFileSync(join(stale, ".tohseno", "run", "state.json"), `${JSON.stringify(oldState)}\n`);
        writeFileSync(join(stale, "Config", "DevelopmentEndpoint.xcconfig"), "stale fixture\n");
        const recovered = await machine(scratch, stale, ["dev", "start"]);
        expect(recovered.exitCode).toBe(0);
        expect(recovered.envelope.result.instanceId).not.toBe(oldInstance);

        await machine(scratch, stale, ["dev", "stop"]);
        const unrelated = Bun.spawn([process.execPath, "-e", "setInterval(() => {}, 1000)"], {
          stdin: "ignore",
          stdout: "ignore",
          stderr: "ignore",
        });
        try {
          const lockDirectory = join(stale, ".tohseno", "run", "start.lock");
          mkdirSync(lockDirectory);
          writeFileSync(join(lockDirectory, "owner.json"), `${JSON.stringify({
            schemaVersion: 1,
            pid: unrelated.pid,
            attemptId: "stale-live-pid",
            commandContains: ["definitely-not-this-command"],
          })}\n`);
          utimesSync(lockDirectory, new Date(0), new Date(0));
          const staleLockStatus = await machine(scratch, stale, ["dev", "status"]);
          expect(staleLockStatus.envelope.result.issues).toContain("stale development start lock detected");
          const lockRecovered = await machine(scratch, stale, ["dev", "start"]);
          expect(lockRecovered.exitCode).toBe(0);
          expect(isProcessAlive(unrelated.pid)).toBe(true);
          await machine(scratch, stale, ["dev", "stop"]);

          const fakeState = {
            ...oldState,
            instanceId: "reused-pid-fixture",
            supervisor: { pid: unrelated.pid, role: "supervisor", commandContains: ["definitely-not-this-command"] },
            api: { ...oldState.api, pid: 999_997 },
            tunnel: null,
          };
          writeFileSync(join(stale, ".tohseno", "run", "state.json"), `${JSON.stringify(fakeState)}\n`);
          const stopped = await machine(scratch, stale, ["dev", "stop"]);
          expect(stopped.exitCode).toBe(0);
          expect(stopped.envelope.result.refusedPids).toContain(unrelated.pid);
          expect(isProcessAlive(unrelated.pid)).toBe(true);
        } finally {
          unrelated.kill("SIGTERM");
          await unrelated.exited;
        }
      } finally {
        await stopQuietly(scratch, stale);
      }
    });
  }, 30_000);
});

describe("token operations", () => {
  const TOKEN_ADDRESS = "0x1111111111111111111111111111111111111111";
  const TOKEN_TX = `0x${"2".repeat(64)}`;

  function fakeBankrHome(scratch: ScratchEnvironment): void {
    mkdirSync(join(scratch.home, ".bankr"), { recursive: true });
    writeFileSync(join(scratch.home, ".bankr", "config.json"), "{}\n", { mode: 0o600 });
  }

  test("status, guarded launch, manifest record, key hygiene, and fees passthrough", async () => {
    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "token-flow");

      const bare = await machine(scratch, shot, ["token", "status"]);
      expect(bare.exitCode).toBe(0);
      // CLI availability depends on the host (a real npx may resolve); auth
      // and the token record are controlled by the scratch environment.
      expect(bare.envelope.result).toMatchObject({
        authenticated: false,
        token: null,
      });
      expect(typeof bare.envelope.result.bankrCliAvailable).toBe("boolean");

      const inventory = await machine(scratch, shot, ["operations"]);
      const operations = (inventory.envelope.result.commands as Array<{ operation: string }>)
        .map((command) => command.operation);
      expect(operations).toContain("token.status");
      expect(operations).toContain("token.launch");
      expect(operations).toContain("token.fees");

      const noAuth = await machine(scratch, shot, [
        "token", "launch", "--name", "Continuity", "--symbol", "CONT", "--chain", "base", "--yes",
      ]);
      expect(noAuth.exitCode).toBe(3);
      expect(noAuth.envelope.error?.code).toBe("MISSING_DEPENDENCY");
      expect(noAuth.envelope.error?.message).toContain("npx @bankr/cli login email");

      const sentinel = "bk_token-secret-must-never-appear-9c41";
      scratch.environment.BANKR_API_KEY = sentinel;
      fakeBankrHome(scratch);
      const keyWitness = join(scratch.root, "bankr-saw-key");
      writeExecutable(scratch.binDirectory, "bankr", [
        "#!/bin/sh",
        `if [ -n \"$BANKR_API_KEY\" ]; then printf 'yes' > ${JSON.stringify(keyWitness)}; fi`,
        "if [ \"$1\" = \"fees\" ]; then printf '{\"claimable\":\"1.5\"}\\n'; exit 0; fi",
        `printf 'Launched with key %s\\n' \"$BANKR_API_KEY\"`,
        `printf 'Token: ${TOKEN_ADDRESS}\\n'`,
        `printf 'Tx: ${TOKEN_TX}\\n'`,
      ].join("\n"));

      const unapproved = await machine(scratch, shot, [
        "token", "launch", "--name", "Continuity", "--symbol", "CONT", "--chain", "base",
      ]);
      expect(unapproved.exitCode).toBe(2);
      expect(unapproved.envelope.error?.code).toBe("INVALID_CONFIGURATION");
      expect(unapproved.envelope.error?.message).toContain("--yes");
      expect(unapproved.envelope.error?.message).toContain("IRREVERSIBLE");
      expect(unapproved.envelope.error?.message).toContain("0.7% swap fee");

      const badChain = await machine(scratch, shot, [
        "token", "launch", "--name", "Continuity", "--symbol", "CONT", "--chain", "ethereum", "--yes",
      ]);
      expect(badChain.exitCode).toBe(2);

      const launched = await machine(scratch, shot, [
        "token", "launch", "--name", "Continuity", "--symbol", "CONT", "--chain", "base",
        "--fee-recipient", "owner.eth", "--fee-type", "ens", "--yes",
      ]);
      expect(launched.exitCode).toBe(0);
      expect(launched.stderr).toEqual([]);
      expect(launched.envelope.result).toMatchObject({
        launched: true,
        parsed: true,
        token: {
          provider: "bankr",
          chain: "base",
          name: "Continuity",
          symbol: "CONT",
          feeRecipient: "owner.eth",
          address: TOKEN_ADDRESS,
          txHash: TOKEN_TX,
        },
      });

      const manifestRaw = readFileSync(join(shot, "continuity.manifest.json"), "utf8");
      expect(JSON.parse(manifestRaw).token.address).toBe(TOKEN_ADDRESS);
      const validated = await runProcess(
        [process.execPath, join(shot, ".tohseno", "manifest", "cli.ts"), join(shot, "continuity.manifest.json")],
        shot,
        scratch.environment,
      );
      expect(validated.exitCode).toBe(0);

      const logPath = join(shot, ".tohseno", "run", "logs", "token.log");
      const log = readFileSync(logPath, "utf8");
      expect(log).toContain("bankr_launch");
      expect(log).toContain("[redacted]");
      expect(log).not.toContain(sentinel);
      expect(JSON.stringify(launched.envelope)).not.toContain(sentinel);
      expect(manifestRaw).not.toContain(sentinel);
      expect(readFileSync(keyWitness, "utf8")).toBe("yes");
      expect((await runGit(["check-ignore", "--quiet", "--no-index", ".tohseno/run/logs/token.log"], shot, scratch.environment)).exitCode).toBe(0);

      const again = await machine(scratch, shot, [
        "token", "launch", "--name", "Second", "--symbol", "TWO", "--chain", "base", "--yes",
      ]);
      expect(again.exitCode).toBe(2);
      expect(again.envelope.error?.message).toContain("already recorded");

      const status = await machine(scratch, shot, ["token", "status"]);
      expect(status.exitCode).toBe(0);
      expect(status.envelope.result).toMatchObject({
        bankrCliAvailable: true,
        authenticated: true,
        token: { address: TOKEN_ADDRESS },
      });

      const fees = await machine(scratch, shot, ["token", "fees"]);
      expect(fees.exitCode).toBe(0);
      expect(fees.envelope.result).toMatchObject({
        address: TOKEN_ADDRESS,
        fees: { claimable: "1.5" },
      });
    });
  }, 30_000);

  test("a failing bankr launch surfaces the error verbatim and leaves the manifest untouched", async () => {
    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "token-failure");
      fakeBankrHome(scratch);
      writeExecutable(
        scratch.binDirectory,
        "bankr",
        "#!/bin/sh\nprintf 'rate limited: one launch per minute\\n' >&2\nexit 9",
      );
      const before = readFileSync(join(shot, "continuity.manifest.json"), "utf8");
      const failed = await machine(scratch, shot, [
        "token", "launch", "--name", "Continuity", "--symbol", "CONT", "--chain", "robinhood", "--yes",
      ]);
      expect(failed.exitCode).toBe(5);
      expect(failed.envelope.error?.code).toBe("INTERNAL_FAILURE");
      expect(failed.envelope.error?.message).toContain("rate limited: one launch per minute");
      expect(failed.envelope.error?.details?.hint).toContain("one launch per minute");
      expect(failed.stdout).toHaveLength(1);
      expect(failed.stderr).toEqual([]);
      expect(readFileSync(join(shot, "continuity.manifest.json"), "utf8")).toBe(before);

      const fees = await machine(scratch, shot, ["token", "fees"]);
      expect(fees.exitCode).toBe(2);
      expect(fees.envelope.error?.code).toBe("INVALID_CONFIGURATION");
    });
  }, 30_000);
});

describe("runtime parsing and production endpoint gates", () => {
  test("parses only realistic HTTPS trycloudflare origins", () => {
    expect(parseQuickTunnelUrl("INF Visit https://soft-field-9.trycloudflare.com now"))
      .toBe("https://soft-field-9.trycloudflare.com");
    expect(parseQuickTunnelUrl("http://soft-field-9.trycloudflare.com")).toBeNull();
    expect(parseQuickTunnelUrl("https://trycloudflare.com.evil.example")).toBeNull();
  });

  test("Release build validation rejects missing, localhost, and Quick Tunnel endpoints", async () => {
    const script = join(REPOSITORY_ROOT, "templates", "continuity-app", "scripts", "validate-production-endpoint.sh");
    const environment = { ...process.env, CONFIGURATION: "Release" };
    expect((await runProcess([script], REPOSITORY_ROOT, { ...environment, TOHSENO_API_BASE_URL: "" })).exitCode).toBe(1);
    expect((await runProcess([script], REPOSITORY_ROOT, { ...environment, TOHSENO_API_BASE_URL: "http://localhost:3000" })).exitCode).toBe(1);
    expect((await runProcess([script], REPOSITORY_ROOT, { ...environment, TOHSENO_API_BASE_URL: "https://random.trycloudflare.com" })).exitCode).toBe(1);
    expect((await runProcess([script], REPOSITORY_ROOT, { ...environment, TOHSENO_API_BASE_URL: "https://api.example.com" })).exitCode).toBe(0);
    expect((await runProcess([script], REPOSITORY_ROOT, { ...environment, TOHSENO_API_BASE_URL: "https://api.example.com/path" })).exitCode).toBe(1);
  });

  test("production inspection reports endpoint, persistence, backups, safe secret references, and capabilities", async () => {
    expect(inspectEndpoint("http://localhost:3000")).toMatchObject({
      configured: true,
      stableHttps: false,
      localhost: true,
      valid: false,
    });
    expect(inspectEndpoint("https://temporary.trycloudflare.com")).toMatchObject({
      quickTunnel: true,
      valid: false,
    });
    expect(inspectEndpoint("https://api.example.com")).toMatchObject({
      stableHttps: true,
      valid: true,
    });

    await withScratchEnvironment(async (scratch) => {
      const shot = await createShot(scratch, "production-inspection");
      const baseline = inspectProduction(shot);
      expect(baseline).toMatchObject({
        productionReady: false,
        endpoint: { configured: false },
        persistence: { engine: "sqlite", configured: false, semantics: "single-instance" },
        backups: { configured: false },
        secrets: { required: 0, unresolved: [] },
      });
      expect(baseline.capabilities.implemented).toContain("production.inspect");
      expect(baseline.capabilities.proposed).toContain("production.deploy");

      writeFileSync(join(shot, "Config", "Production.xcconfig"), [
        "// test production origin",
        "PRODUCTION_API_BASE_URL = https:/$()/api.example.com",
        "",
      ].join("\n"));
      const operationsPath = join(shot, "operations", "production.json");
      const operations = JSON.parse(readFileSync(operationsPath, "utf8"));
      operations.persistence.configured = true;
      operations.backups = { configured: true, strategy: "file:/backups/shot.sqlite3" };
      operations.requiredSecrets = [{
        slot: "provider-api-key",
        reference: "env:PROVIDER_API_KEY",
        resolved: false,
      }];
      operations.capabilities.deploy = "implemented";
      writeFileSync(operationsPath, `${JSON.stringify(operations, null, 2)}\n`);

      const unresolved = inspectProduction(shot);
      expect(unresolved.productionReady).toBe(false);
      expect(unresolved.endpoint).toMatchObject({ value: "https://api.example.com", valid: true });
      expect(unresolved.secrets.unresolved).toEqual(["provider-api-key"]);
      expect(JSON.stringify(unresolved)).not.toContain("PROVIDER_API_KEY");

      operations.requiredSecrets[0].resolved = true;
      writeFileSync(operationsPath, `${JSON.stringify(operations, null, 2)}\n`);
      expect(inspectProduction(shot).productionReady).toBe(true);

      operations.requiredSecrets[0].reference = "test-secret-value-must-not-be-stored";
      writeFileSync(operationsPath, `${JSON.stringify(operations, null, 2)}\n`);
      expect(() => inspectProduction(shot)).toThrow("never secret values");
    });
  }, 20_000);
});
