import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
} from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { buildCliRelease } from "../scripts/package-release.ts";
import { waitForProcessExit } from "../factory/runtime/shared.ts";
import {
  REPOSITORY_ROOT,
  runGit,
  runProcess,
  withScratchEnvironment,
  writeExecutable,
} from "./helpers.ts";

const INSTALLER = join(REPOSITORY_ROOT, "apps", "site", "public", "install.sh");

function sha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

async function fakeBunArchive(root: string): Promise<string> {
  const distribution = join(root, "fake managed Bun");
  mkdirSync(distribution, { recursive: true });
  writeExecutable(distribution, "bun", [
    "#!/bin/sh",
    `exec ${JSON.stringify(process.execPath)} \"$@\"`,
  ].join("\n"));
  const archive = join(root, "fake-bun.zip");
  const zip = await runProcess(["/usr/bin/zip", "-q", archive, "bun"], distribution, {
    PATH: "/usr/bin:/bin",
  });
  if (zip.exitCode !== 0) throw new Error(zip.stderr);
  return archive;
}

function envelope(stdout: string): any {
  return JSON.parse(stdout.trim()) as any;
}

describe("managed installer", () => {
  test("installs without a pre-existing Bun, re-runs safely, and drives an isolated shot acceptance flow", async () => {
    await withScratchEnvironment(async (scratch) => {
      const releaseArchive = join(scratch.root, "artifacts", "tohseno-cli-0.2.0.tar.gz");
      const releaseManifest = join(scratch.root, "artifacts", "tohseno-cli-0.2.0.json");
      const release = buildCliRelease({ output: releaseArchive, manifest: releaseManifest });
      const repeatedArchive = join(scratch.root, "repeated", "tohseno-cli-0.2.0.tar.gz");
      const repeatedRelease = buildCliRelease({
        output: repeatedArchive,
        manifest: join(scratch.root, "repeated", "tohseno-cli-0.2.0.json"),
      });
      expect(repeatedRelease.sha256).toBe(release.sha256);
      expect(readFileSync(repeatedArchive)).toEqual(readFileSync(releaseArchive));
      const bunArchive = await fakeBunArchive(scratch.root);
      const installHome = join(scratch.home, ".tohseno");
      const shots = join(scratch.root, "installed shots with spaces");
      const environment: Record<string, string | undefined> = {
        HOME: scratch.home,
        SHELL: "/bin/sh",
        PATH: "/usr/bin:/bin",
        GIT_CONFIG_NOSYSTEM: "1",
        GIT_CONFIG_GLOBAL: "/dev/null",
        GIT_TERMINAL_PROMPT: "0",
        TOHSENO_INSTALL_HOME: installHome,
        TOHSENO_INSTALL_CLI_URL: releaseArchive,
        TOHSENO_INSTALL_CLI_SHA256: release.sha256,
        TOHSENO_INSTALL_BUN_URL: bunArchive,
        TOHSENO_INSTALL_BUN_SHA256: sha256(bunArchive),
        TOHSENO_SHOTS_DIR: shots,
      };

      const noBun = await runProcess(["/bin/sh", "-c", "command -v bun"], scratch.root, environment);
      expect(noBun.exitCode).not.toBe(0);

      const firstInstall = await runProcess([
        "/bin/sh", INSTALLER, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, environment);
      expect(firstInstall.exitCode).toBe(0);
      expect(firstInstall.stderr).toBe("");
      expect(firstInstall.stdout).toContain("Installed managed Bun 1.2.18");
      const executable = join(installHome, "bin", "tohseno");
      expect(existsSync(executable)).toBe(true);
      expect((await runProcess([executable, "--version"], scratch.root, environment)).stdout.trim()).toBe("0.2.0");

      const created = await runProcess([
        executable,
        "create", "installed-acceptance",
        "--platform", "ios",
        "--no-launch",
        "--no-interactive",
      ], scratch.root, environment);
      expect(created.exitCode).toBe(0);
      const shot = join(shots, "installed-acceptance");
      expect(existsSync(join(shot, ".git"))).toBe(true);

      const cloudflared = writeExecutable(scratch.binDirectory, "cloudflared", [
        "#!/bin/sh",
        "printf '%s\\n' 'INF Requesting new quick Tunnel on trycloudflare.com' >&2",
        "printf '%s\\n' 'INF + https://installer-acceptance-42.trycloudflare.com +' >&2",
        "trap 'exit 0' TERM INT",
        "while :; do sleep 1; done",
      ].join("\n"));
      let supervisorPid = 0;
      let apiPid = 0;
      let tunnelPid = 0;
      try {
        const startedProcess = await runProcess([
          executable, "machine", "dev", "start", "--tunnel", "--cloudflared", cloudflared, "--json",
        ], shot, environment);
        expect(startedProcess.exitCode).toBe(0);
        expect(startedProcess.stderr).toBe("");
        const started = envelope(startedProcess.stdout);
        expect(started).toMatchObject({
          schemaVersion: 1,
          ok: true,
          operation: "dev.start",
          result: {
            state: "running",
            healthy: true,
            tunnel: {
              url: "https://installer-acceptance-42.trycloudflare.com",
              developmentOnly: true,
            },
          },
        });
        apiPid = started.result.api.pid;
        tunnelPid = started.result.tunnel.pid;
        supervisorPid = JSON.parse(
          readFileSync(join(shot, ".tohseno", "run", "state.json"), "utf8"),
        ).supervisor.pid as number;

        const health = await (await fetch(started.result.api.healthUrl)).json() as Record<string, unknown>;
        expect(health).toMatchObject({ status: "ok", ready: true, service: "shot-api" });
        expect(readFileSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"), "utf8"))
          .toContain("https:/$()/installer-acceptance-42.trycloudflare.com");
        expect(existsSync(join(shot, ".tohseno", "data", "development.sqlite3"))).toBe(true);

        const status = envelope((await runProcess([
          executable, "machine", "dev", "status", "--json",
        ], shot, environment)).stdout);
        expect(status.result).toMatchObject({ state: "running", healthy: true });
        const logs = envelope((await runProcess([
          executable, "machine", "dev", "logs", "--service", "all", "--json",
        ], shot, environment)).stdout);
        expect(logs.result.logs.api.join("\n")).toContain('"event":"startup"');
        const verified = await runProcess([executable, "machine", "verify", "--json"], shot, environment);
        expect(verified.exitCode).toBe(0);
        expect(envelope(verified.stdout).result.valid).toBe(true);
      } finally {
        await runProcess([executable, "machine", "dev", "stop", "--json"], shot, environment);
      }

      expect(await waitForProcessExit(supervisorPid, 2_000)).toBe(true);
      expect(await waitForProcessExit(apiPid, 2_000)).toBe(true);
      expect(await waitForProcessExit(tunnelPid, 2_000)).toBe(true);
      expect(existsSync(join(shot, ".tohseno", "run", "state.json"))).toBe(false);
      expect(existsSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"))).toBe(false);
      expect((await runGit(["status", "--porcelain"], shot, environment)).stdout).toBe("");

      const secondInstall = await runProcess([
        "/bin/sh", INSTALLER, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, environment);
      expect(secondInstall.exitCode).toBe(0);
      expect(secondInstall.stdout).toContain("TOHSENO 0.2.0 already verified");
      expect(secondInstall.stdout).toContain("Managed Bun 1.2.18 already verified");

      const tamperedHome = join(scratch.root, "symlinked install");
      mkdirSync(join(tamperedHome, "versions"), { recursive: true });
      symlinkSync(scratch.root, join(tamperedHome, "versions", "0.2.0"), "dir");
      const tampered = await runProcess([
        "/bin/sh", INSTALLER, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, {
        ...environment,
        TOHSENO_INSTALL_HOME: tamperedHome,
      });
      expect(tampered.exitCode).toBe(1);
      expect(tampered.stderr).toContain("managed CLI version is not a real directory");

      const rejectedHome = join(scratch.root, "rejected install");
      const rejected = await runProcess([
        "/bin/sh", INSTALLER, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, {
        ...environment,
        TOHSENO_INSTALL_HOME: rejectedHome,
        TOHSENO_INSTALL_CLI_SHA256: "0".repeat(64),
      });
      expect(rejected.exitCode).toBe(1);
      expect(rejected.stderr).toContain("checksum mismatch");
      expect(existsSync(join(rejectedHome, "versions", "0.2.0"))).toBe(false);
    });
  }, 45_000);
});
