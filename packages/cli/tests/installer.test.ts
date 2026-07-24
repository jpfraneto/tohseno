import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  statSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, test } from "bun:test";
import {
  assertThirdPartyPackageIdentity,
  buildCliRelease,
  thirdPartyTreeSha256,
} from "../scripts/package-release.ts";
import { CLI_VERSION } from "../src/constants.ts";
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

function stagedInstaller(root: string): string {
  const source = readFileSync(INSTALLER, "utf8")
    .replace(
      /^INSTALLER_VERSION="[0-9]+\.[0-9]+\.[0-9]+"$/m,
      `INSTALLER_VERSION="${CLI_VERSION}"`,
    )
    .replace(
      /^CLI_VERSION="[0-9]+\.[0-9]+\.[0-9]+"$/m,
      `CLI_VERSION="${CLI_VERSION}"`,
    );
  const path = join(root, "install-under-test.sh");
  writeFileSync(path, source, { mode: 0o755 });
  return path;
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

async function fakeCloudflaredArchive(root: string): Promise<string> {
  const distribution = join(root, "fake cloudflared");
  mkdirSync(distribution, { recursive: true });
  writeExecutable(distribution, "cloudflared", [
    "#!/bin/sh",
    "exit 0",
  ].join("\n"));
  const archive = join(root, "fake-cloudflared.tgz");
  const tar = await runProcess(
    ["/usr/bin/tar", "-czf", archive, "cloudflared"],
    distribution,
    { PATH: "/usr/bin:/bin" },
  );
  if (tar.exitCode !== 0) throw new Error(tar.stderr);
  return archive;
}

function envelope(stdout: string): any {
  return JSON.parse(stdout.trim()) as any;
}

describe("managed installer", () => {
  test("rejects managed dependency identity, content, or mode drift before packaging", async () => {
    await withScratchEnvironment(async (scratch) => {
      const dependency = join(scratch.root, "third-party-dependency");
      mkdirSync(dependency);
      writeFileSync(
        join(dependency, "package.json"),
        `${JSON.stringify({ name: "ws", version: "8.21.1" })}\n`,
      );
      const expectedTreeSha256 = thirdPartyTreeSha256(dependency);
      expect(() =>
        assertThirdPartyPackageIdentity({
          directory: dependency,
          packageName: "ws",
          version: "8.21.1",
          treeSha256: expectedTreeSha256,
        })
      ).not.toThrow();

      writeFileSync(
        join(dependency, "package.json"),
        `${JSON.stringify({ name: "not-ws", version: "8.21.1" })}\n`,
      );
      expect(() =>
        assertThirdPartyPackageIdentity({
          directory: dependency,
          packageName: "ws",
          version: "8.21.1",
          treeSha256: expectedTreeSha256,
        })
      ).toThrow(
        "expected ws@8.21.1, found not-ws@8.21.1",
      );

      writeFileSync(
        join(dependency, "package.json"),
        `${JSON.stringify({ name: "ws", version: "8.22.0" })}\n`,
      );
      expect(() =>
        assertThirdPartyPackageIdentity({
          directory: dependency,
          packageName: "ws",
          version: "8.21.1",
          treeSha256: expectedTreeSha256,
        })
      ).toThrow(
        "expected ws@8.21.1, found ws@8.22.0",
      );

      writeFileSync(
        join(dependency, "package.json"),
        `${JSON.stringify({ name: "ws", version: "8.21.1" })}\n`,
      );
      writeFileSync(join(dependency, "injected.js"), "export default 1;\n");
      expect(() =>
        assertThirdPartyPackageIdentity({
          directory: dependency,
          packageName: "ws",
          version: "8.21.1",
          treeSha256: expectedTreeSha256,
        })
      ).toThrow("managed release dependency tree mismatch");

      unlinkSync(join(dependency, "injected.js"));
      chmodSync(join(dependency, "package.json"), 0o755);
      expect(() =>
        assertThirdPartyPackageIdentity({
          directory: dependency,
          packageName: "ws",
          version: "8.21.1",
          treeSha256: expectedTreeSha256,
        })
      ).toThrow("managed release dependency tree mismatch");
    });
  });

  test("installs without a pre-existing Bun, re-runs safely, and drives an isolated shot acceptance flow", async () => {
    await withScratchEnvironment(async (scratch) => {
      const releaseArchive = join(
        scratch.root,
        "artifacts",
        `tohseno-cli-${CLI_VERSION}.tar.gz`,
      );
      const releaseManifest = join(
        scratch.root,
        "artifacts",
        `tohseno-cli-${CLI_VERSION}.json`,
      );
      const release = buildCliRelease({ output: releaseArchive, manifest: releaseManifest });
      const repeatedArchive = join(
        scratch.root,
        "repeated",
        `tohseno-cli-${CLI_VERSION}.tar.gz`,
      );
      const repeatedRelease = buildCliRelease({
        output: repeatedArchive,
        manifest: join(
          scratch.root,
          "repeated",
          `tohseno-cli-${CLI_VERSION}.json`,
        ),
      });
      expect(repeatedRelease.sha256).toBe(release.sha256);
      expect(repeatedRelease.treeSha256).toBe(release.treeSha256);
      expect(readFileSync(repeatedArchive)).toEqual(readFileSync(releaseArchive));
      const bunArchive = await fakeBunArchive(scratch.root);
      const cloudflaredArchive = await fakeCloudflaredArchive(scratch.root);
      const installer = stagedInstaller(scratch.root);
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
        TOHSENO_INSTALL_CLI_TREE_SHA256: release.treeSha256,
        TOHSENO_INSTALL_BUN_URL: bunArchive,
        TOHSENO_INSTALL_BUN_SHA256: sha256(bunArchive),
        TOHSENO_SHOTS_DIR: shots,
      };

      const noBun = await runProcess(["/bin/sh", "-c", "command -v bun"], scratch.root, environment);
      expect(noBun.exitCode).not.toBe(0);

      const firstInstall = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, environment);
      expect(firstInstall.exitCode).toBe(0);
      expect(firstInstall.stderr).toBe("");
      expect(firstInstall.stdout).toContain("TOHSENO will install:");
      expect(firstInstall.stdout).toContain(
        "TOHSENO does not create an account, request credentials, or upload your app",
      );
      expect(firstInstall.stdout).toContain("You do not need to install Bun.");
      expect(firstInstall.stdout).toContain("✅ TOHSENO IS READY");
      expect(firstInstall.stdout).toContain("    tohseno");
      expect(firstInstall.stdout).toContain("    tohseno doctor");
      expect(firstInstall.stdout).toContain("Installed managed Bun 1.2.18");
      expect(statSync(installHome).mode & 0o777).toBe(0o700);
      expect(statSync(join(installHome, "bin")).mode & 0o777).toBe(0o700);
      const executable = join(installHome, "bin", "tohseno");
      expect(existsSync(executable)).toBe(true);
      expect((await runProcess([executable, "--version"], scratch.root, environment)).stdout.trim()).toBe(CLI_VERSION);
      const installedCli = join(
        installHome,
        "versions",
        CLI_VERSION,
        "factory-source",
        "packages",
        "cli",
      );
      const middleware = join(
        installedCli,
        "node_modules",
        "serve-sim",
        "dist",
        "middleware.js",
      );
      expect(existsSync(middleware)).toBe(true);
      expect(existsSync(
        join(installedCli, "node_modules", "ws", "LICENSE"),
      )).toBe(true);
      const middlewareProbe = await runProcess([
        process.execPath,
        "--input-type=module",
        "--eval",
        `await import(${JSON.stringify(pathToFileURL(middleware).href)}); process.stdout.write("serve-sim ready")`,
      ], installedCli, environment);
      expect(middlewareProbe.exitCode).toBe(0);
      expect(middlewareProbe.stderr).toBe("");
      expect(middlewareProbe.stdout).toBe("serve-sim ready");

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
      expect(existsSync(join(shot, "app.manifest.json"))).toBe(true);
      expect(existsSync(join(shot, "tohseno.skills.lock"))).toBe(true);
      expect(existsSync(join(shot, "Shot.xcodeproj"))).toBe(true);
      expect(existsSync(join(shot, "Server"))).toBe(false);
      expect(existsSync(join(shot, "Config", "DevelopmentEndpoint.xcconfig"))).toBe(false);
      const verified = await runProcess(
        [executable, "machine", "verify", "--json"],
        shot,
        environment,
      );
      expect(verified.exitCode).toBe(0);
      expect(envelope(verified.stdout).result.valid).toBe(true);
      expect(JSON.parse(
        readFileSync(join(shot, "app.manifest.json"), "utf8"),
      )).toMatchObject({ kind: "app", schemaVersion: "1.0.0" });
      expect(JSON.parse(
        readFileSync(join(shot, "tohseno.skills.json"), "utf8"),
      )).toEqual({ schemaVersion: 1, template: "blank", skills: [] });
      expect((await runGit(["status", "--porcelain"], shot, environment)).stdout).toBe("");

      const secondInstall = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, environment);
      expect(secondInstall.exitCode).toBe(0);
      expect(secondInstall.stdout).toContain(
        `TOHSENO ${CLI_VERSION} already verified`,
      );
      expect(secondInstall.stdout).toContain("Managed Bun 1.2.18 already verified");

      const profileVictim = join(scratch.root, "profile-victim");
      const shellProfile = join(scratch.home, ".profile");
      writeFileSync(profileVictim, "owner content\n");
      symlinkSync(profileVictim, shellProfile);
      const unsafeProfileInstall = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--without-cloudflared",
      ], scratch.root, environment);
      expect(unsafeProfileInstall.exitCode).toBe(0);
      expect(unsafeProfileInstall.stdout).toContain(
        "Skipped shell profile update because the target is not a safe regular file",
      );
      expect(readFileSync(profileVictim, "utf8")).toBe("owner content\n");
      unlinkSync(shellProfile);

      linkSync(profileVictim, shellProfile);
      const hardlinkedProfileInstall = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--without-cloudflared",
      ], scratch.root, environment);
      expect(hardlinkedProfileInstall.exitCode).toBe(0);
      expect(hardlinkedProfileInstall.stdout).toContain(
        "Skipped shell profile update because the target is not a safe regular file",
      );
      expect(readFileSync(profileVictim, "utf8")).toBe("owner content\n");
      unlinkSync(shellProfile);

      const cloudflaredEnvironment = {
        ...environment,
        TOHSENO_INSTALL_CLOUDFLARED_URL: cloudflaredArchive,
        TOHSENO_INSTALL_CLOUDFLARED_SHA256: sha256(cloudflaredArchive),
      };
      const cloudflaredInstall = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--no-modify-path",
      ], scratch.root, cloudflaredEnvironment);
      expect(cloudflaredInstall.exitCode).toBe(0);
      expect(cloudflaredInstall.stdout).toContain(
        "Installed managed cloudflared",
      );
      const managedCloudflared = join(
        installHome,
        "tools",
        "cloudflared-2026.5.2",
      );
      const cloudflaredSource = readFileSync(managedCloudflared);
      writeFileSync(
        managedCloudflared,
        `${cloudflaredSource.toString("utf8")}\n# drift\n`,
      );
      chmodSync(managedCloudflared, 0o755);
      const wrapperRejectedCloudflared = await runProcess(
        [executable, "--version"],
        scratch.root,
        cloudflaredEnvironment,
      );
      expect(wrapperRejectedCloudflared.exitCode).toBe(1);
      expect(wrapperRejectedCloudflared.stderr).toContain(
        "cloudflared binary differs",
      );
      writeFileSync(managedCloudflared, cloudflaredSource);
      chmodSync(managedCloudflared, 0o755);

      const installedConstants = join(
        installHome,
        "versions",
        CLI_VERSION,
        "factory-source",
        "packages",
        "cli",
        "src",
        "constants.ts",
      );
      const constantsSource = readFileSync(installedConstants);
      writeFileSync(installedConstants, `${constantsSource.toString("utf8")}\n// drift\n`);
      const wrapperRejectedContent = await runProcess(
        [executable, "--version"],
        scratch.root,
        environment,
      );
      expect(wrapperRejectedContent.exitCode).toBe(1);
      expect(wrapperRejectedContent.stderr).toContain("CLI tree differs");
      const installerRejectedContent = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, environment);
      expect(installerRejectedContent.exitCode).toBe(1);
      expect(installerRejectedContent.stderr).toContain(
        "CLI tree failed integrity verification",
      );
      writeFileSync(installedConstants, constantsSource);

      const cliArtifactMarker = join(
        installHome,
        "versions",
        CLI_VERSION,
        ".artifact.sha256",
      );
      const cliArtifactMarkerSource = readFileSync(cliArtifactMarker);
      writeFileSync(
        cliArtifactMarker,
        Buffer.concat([cliArtifactMarkerSource, Buffer.from("\n")]),
      );
      const wrapperRejectedMarker = await runProcess(
        [executable, "--version"],
        scratch.root,
        environment,
      );
      expect(wrapperRejectedMarker.exitCode).toBe(1);
      expect(wrapperRejectedMarker.stderr).toContain(
        "CLI artifact marker differs",
      );
      writeFileSync(cliArtifactMarker, cliArtifactMarkerSource);

      const installedReadme = join(
        installHome,
        "versions",
        CLI_VERSION,
        "factory-source",
        "templates",
        "continuity-app",
        "README.md",
      );
      chmodSync(installedReadme, 0o755);
      expect(
        (await runProcess([executable, "--version"], scratch.root, environment))
          .exitCode,
      ).toBe(1);
      chmodSync(installedReadme, 0o644);
      const readmeSource = readFileSync(installedReadme);
      unlinkSync(installedReadme);
      symlinkSync(installedConstants, installedReadme);
      expect(
        (await runProcess([executable, "--version"], scratch.root, environment))
          .exitCode,
      ).toBe(1);
      unlinkSync(installedReadme);
      writeFileSync(installedReadme, readmeSource, { mode: 0o644 });

      const hardlinkedReadme = join(scratch.root, "hardlinked-readme");
      linkSync(installedReadme, hardlinkedReadme);
      expect(
        (await runProcess([executable, "--version"], scratch.root, environment))
          .exitCode,
      ).toBe(1);
      unlinkSync(hardlinkedReadme);

      const extraFile = join(
        installHome,
        "versions",
        CLI_VERSION,
        "factory-source",
        "unexpected.js",
      );
      writeFileSync(extraFile, "unexpected\n");
      expect(
        (await runProcess([executable, "--version"], scratch.root, environment))
          .exitCode,
      ).toBe(1);
      unlinkSync(extraFile);

      const managedBun = join(
        installHome,
        "runtime",
        "bun-1.2.18",
        "bin",
        "bun",
      );
      const bunSource = readFileSync(managedBun);
      writeFileSync(managedBun, `${bunSource.toString("utf8")}\n# drift\n`);
      chmodSync(managedBun, 0o755);
      const wrapperRejectedBun = await runProcess(
        [executable, "--version"],
        scratch.root,
        environment,
      );
      expect(wrapperRejectedBun.exitCode).toBe(1);
      expect(wrapperRejectedBun.stderr).toContain("Bun binary differs");
      writeFileSync(managedBun, bunSource);
      chmodSync(managedBun, 0o755);
      expect(
        (await runProcess([executable, "--version"], scratch.root, environment))
          .stdout.trim(),
      ).toBe(CLI_VERSION);

      const pathHijackWitness = join(scratch.root, "path-hijack-witness");
      writeExecutable(
        scratch.binDirectory,
        "shasum",
        `#!/bin/sh\nprintf ran > ${JSON.stringify(pathHijackWitness)}\nexit 0\n`,
      );
      const pathHardened = await runProcess(
        [executable, "--version"],
        scratch.root,
        {
          ...environment,
          PATH: `${scratch.binDirectory}:/usr/bin:/bin`,
        },
      );
      expect(pathHardened.exitCode).toBe(0);
      expect(pathHardened.stdout.trim()).toBe(CLI_VERSION);
      expect(existsSync(pathHijackWitness)).toBe(false);

      const tamperedHome = join(scratch.root, "symlinked install");
      mkdirSync(join(tamperedHome, "versions"), { recursive: true });
      symlinkSync(
        scratch.root,
        join(tamperedHome, "versions", CLI_VERSION),
        "dir",
      );
      const tampered = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, {
        ...environment,
        TOHSENO_INSTALL_HOME: tamperedHome,
      });
      expect(tampered.exitCode).toBe(1);
      expect(tampered.stderr).toContain("managed CLI version is not a real directory");

      const rejectedHome = join(scratch.root, "rejected install");
      const rejected = await runProcess([
        "/bin/sh", installer, "--non-interactive", "--no-modify-path", "--without-cloudflared",
      ], scratch.root, {
        ...environment,
        TOHSENO_INSTALL_HOME: rejectedHome,
        TOHSENO_INSTALL_CLI_SHA256: "0".repeat(64),
      });
      expect(rejected.exitCode).toBe(1);
      expect(rejected.stderr).toContain("checksum mismatch");
      expect(existsSync(join(rejectedHome, "versions", CLI_VERSION))).toBe(false);
    });
  }, 45_000);
});
