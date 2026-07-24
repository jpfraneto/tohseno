import { describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  realpathSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import {
  createCommandCancellation,
  previewCommand,
  runCommand,
  type CommandCancellation,
} from "../src/commands.ts";
import { resolveConfig } from "../src/config.ts";
import { sanitizedRuntimeEnvironment } from "../src/process.ts";
import { resolveBuiltBundleIdentifier } from "../factory/runtime/ios.ts";
import {
  LivePreviewManager,
  SERVE_SIM_VERSION,
  SimulatorService,
  executeCommand,
  runShotInSimulator,
  simulatorDiagnostics,
  type CommandExecutor,
  type LivePreviewHandle,
  type ProcessSpawner,
  type RunShotOptions,
  type ShotRunResult,
  type SimulatorProgressEvent,
  type SpawnedProcessExit,
} from "../src/simulator.ts";
import { safeEnvironment } from "../factory/runtime/shared.ts";
import {
  createMemoryIo,
  type ScratchEnvironment,
  withScratchEnvironment,
} from "./helpers.ts";

const DEVICE_UDID = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE";
const ONE_PIXEL_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z1iQAAAAASUVORK5CYII=",
  "base64",
);

function writeShotFixture(
  scratch: ScratchEnvironment,
  slug = "simulator-shot",
): { shot: string; local: string; app: string } {
  const shot = join(scratch.shotsDirectory, slug);
  const local = join(shot, ".tohseno");
  const app = join(shot, "build", "SimulatorShot.app");
  mkdirSync(local, { recursive: true });
  mkdirSync(app, { recursive: true });
  writeFileSync(join(app, "Info.plist"), "test plist fixture\n");
  writeFileSync(join(local, "machine.ts"), "// pinned machine fixture\n");
  writeFileSync(join(local, "shot.json"), `${JSON.stringify({
    schemaVersion: 1,
    slug,
    platform: "ios",
    adopted: false,
    createdAt: "2026-07-23T00:00:00.000Z",
    selectedAgent: null,
    baselineAuthor: "factory",
    factory: {
      releaseId: `content-${"a".repeat(32)}`,
      cliVersion: "0.3.1",
      templateVersion: "0.4.0",
      manifestSchemaVersion: "0.4.0",
      sourceCommit: null,
      sourceDirty: false,
      bundleDigest: "a".repeat(64),
    },
  }, null, 2)}\n`);
  writeFileSync(join(shot, "continuity.manifest.json"), `${JSON.stringify({
    application: { id: `com.tohseno.${slug}` },
  })}\n`);
  return { shot, local, app };
}

function successfulMachineResponse(
  argv: readonly string[],
  fixture: ReturnType<typeof writeShotFixture>,
  bundleId = `com.tohseno.${fixture.shot.split("/").at(-1) ?? ""}`,
): { exitCode: number; stdout: string; stderr: string } | null {
  if (argv.includes("dev") && argv.includes("start")) {
    return {
      exitCode: 0,
      stdout: JSON.stringify({
        schemaVersion: 1,
        ok: true,
        operation: "dev.start",
        shot: fixture.shot,
        result: { state: "running" },
      }),
      stderr: "",
    };
  }
  if (argv.includes("ios") && argv.includes("launch")) {
    return {
      exitCode: 0,
      stdout: JSON.stringify({
        schemaVersion: 1,
        ok: true,
        operation: "ios.launch",
        shot: fixture.shot,
        result: {
          launched: true,
          device: {
            name: "iPhone Test",
            udid: DEVICE_UDID,
            state: "Booted",
            runtime: "com.apple.CoreSimulator.SimRuntime.iOS-26-0",
            available: true,
          },
          bundleId,
          appPath: fixture.app,
        },
      }),
      stderr: "",
    };
  }
  return null;
}

function builtBundleIdentifierResponse(
  argv: readonly string[],
  bundleId: string,
): { exitCode: number; stdout: string; stderr: string } | null {
  if (argv[1] === "-extract" && argv[2] === "CFBundleIdentifier") {
    return { exitCode: 0, stdout: `${bundleId}\n`, stderr: "" };
  }
  return null;
}

function simulatorExecutable(name: string): string | null {
  if (name === "xcrun") return "/usr/bin/xcrun";
  if (name === "plutil" || name === "/usr/bin/plutil") return "/usr/bin/plutil";
  return null;
}

function compatibleNodeResponse(
  argv: readonly string[],
): { exitCode: number; stdout: string; stderr: string } {
  return {
    exitCode: 0,
    stdout: argv[1] === "--print" ? "arm64\n" : "v22.0.0\n",
    stderr: "",
  };
}

async function waitForFile(path: string, attempts = 100): Promise<boolean> {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (existsSync(path)) return true;
    await new Promise<void>((resolveWait) => setTimeout(resolveWait, 10));
  }
  return existsSync(path);
}

describe("Simulator service boundary", () => {
  test("preserves the selected Xcode developer directory in sanitized runtimes", () => {
    const environment = {
      PATH: "/usr/bin",
      DEVELOPER_DIR: "/Applications/Xcode-beta.app/Contents/Developer",
      BANKR_API_KEY: "bankr-secret-must-not-cross",
      PRIVATE_VALUE: "must-not-cross-the-boundary",
    };
    expect(sanitizedRuntimeEnvironment(environment)).toEqual({
      PATH: "/usr/bin",
      DEVELOPER_DIR: "/Applications/Xcode-beta.app/Contents/Developer",
    });
    expect(safeEnvironment(environment)).toEqual({
      PATH: "/usr/bin",
      DEVELOPER_DIR: "/Applications/Xcode-beta.app/Contents/Developer",
    });
  });

  test("cancellation terminates the owned machine process group", async () => {
    if (process.platform === "win32") return;
    await withScratchEnvironment(async (scratch) => {
      const descendantReady = join(scratch.root, "descendant-ready");
      const descendantStopped = join(scratch.root, "descendant-stopped");
      const descendantSource = [
        "const { writeFileSync } = require('node:fs');",
        `writeFileSync(${JSON.stringify(descendantReady)}, 'ready');`,
        "process.on('SIGTERM', () => {",
        `  writeFileSync(${JSON.stringify(descendantStopped)}, 'stopped');`,
        "  process.exit(0);",
        "});",
        "setInterval(() => {}, 1000);",
      ].join("\n");
      const parentSource = [
        "const { spawn } = require('node:child_process');",
        `spawn(process.execPath, ['-e', ${JSON.stringify(descendantSource)}], { stdio: 'ignore' });`,
        "setInterval(() => {}, 1000);",
      ].join("\n");
      const controller = new AbortController();
      const operation = executeCommand(
        [process.execPath, "-e", parentSource],
        {
          cwd: scratch.root,
          environment: sanitizedRuntimeEnvironment(scratch.environment),
          signal: controller.signal,
          timeoutMs: 5_000,
        },
      );
      try {
        expect(await waitForFile(descendantReady)).toBe(true);
        controller.abort();
        await expect(operation).rejects.toMatchObject({ code: "ABORTED" });
        expect(await waitForFile(descendantStopped)).toBe(true);
      } finally {
        controller.abort();
      }
    });
  });

  test("reports unsupported hosts and missing Xcode without touching Simulator", async () => {
    const unsupported = await simulatorDiagnostics({
      platform: "linux",
      architecture: "x64",
      environment: {},
      findExecutable: () => null,
      resolveServeSim: () => null,
      executor: async () => {
        throw new Error("diagnostics should not spawn");
      },
    });
    expect(unsupported.previewReady).toBe(false);
    expect(unsupported.blockers.map((blocker) => blocker.code)).toEqual(
      expect.arrayContaining([
        "macos-required",
        "apple-silicon-required",
        "xcode-tools-required",
        "serve-sim-required",
      ]),
    );

    const missingXcode = await simulatorDiagnostics({
      platform: "darwin",
      architecture: "arm64",
      environment: { PATH: "/test/bin" },
      findExecutable: (name) => name === "node" ? "/test/bin/node" : null,
      resolveServeSim: () => ({
        packageJsonPath: "/test/serve-sim/package.json",
        middlewarePath: "/test/serve-sim/middleware.js",
        version: SERVE_SIM_VERSION,
        middlewareExport: true,
      }),
      executor: async (argv) => compatibleNodeResponse(argv),
    });
    expect(missingXcode.previewReady).toBe(false);
    expect(missingXcode.blockers.map((blocker) => blocker.code))
      .toContain("xcode-tools-required");
  });

  test("reports an x64 selected Node as incompatible on an arm64 host", async () => {
    const diagnostics = await simulatorDiagnostics({
      platform: "darwin",
      architecture: "arm64",
      environment: { PATH: "/test/bin" },
      findExecutable: (name) => `/test/bin/${name}`,
      resolveServeSim: () => ({
        packageJsonPath: "/test/serve-sim/package.json",
        middlewarePath: "/test/serve-sim/middleware.js",
        version: SERVE_SIM_VERSION,
        middlewareExport: true,
      }),
      executor: async (argv) => {
        if (argv[0] === "/test/bin/node") {
          return {
            exitCode: 0,
            stdout: argv[1] === "--print" ? "x64\n" : "v22.0.0\n",
            stderr: "",
          };
        }
        if (argv[0] === "/test/bin/xcodebuild") {
          return { exitCode: 0, stdout: "Xcode 26.0\n", stderr: "" };
        }
        return {
          exitCode: 0,
          stdout: JSON.stringify({
            devices: {
              "com.apple.CoreSimulator.SimRuntime.iOS-26-0": [{
                name: "iPhone Test",
                udid: DEVICE_UDID,
                state: "Booted",
                isAvailable: true,
              }],
            },
          }),
          stderr: "",
        };
      },
    });

    expect(diagnostics.node).toMatchObject({
      version: "22.0.0",
      architecture: "x64",
      supported: true,
      compatible: false,
    });
    expect(diagnostics.previewReady).toBe(false);
    expect(diagnostics.blockers).toContainEqual({
      code: "node-arm64-required",
      message: "Live preview requires an arm64 Node.js binary on Apple Silicon.",
    });
  });

  test("orchestrates the pinned shot machine and screenshot with argv arrays", async () => {
    await withScratchEnvironment(async (scratch) => {
      const fixture = writeShotFixture(scratch);
      const commands: string[][] = [];
      const executor: CommandExecutor = async (argv) => {
        commands.push([...argv]);
        const machine = successfulMachineResponse(argv, fixture);
        if (machine !== null) return machine;
        const plist = builtBundleIdentifierResponse(
          argv,
          "com.tohseno.simulator-shot",
        );
        if (plist !== null) return plist;
        if (argv[1] === "simctl" && argv[2] === "io") {
          const destination = argv.at(-1);
          if (!destination) throw new Error("missing screenshot destination");
          writeFileSync(destination, ONE_PIXEL_PNG);
          return { exitCode: 0, stdout: "", stderr: "" };
        }
        throw new Error(`unexpected argv: ${JSON.stringify(argv)}`);
      };
      const events: SimulatorProgressEvent[] = [];
      const result = await runShotInSimulator({
        shotRoot: fixture.shot,
        environment: scratch.environment,
        onProgress: (event) => {
          events.push(event);
        },
      }, {
        executor,
        findExecutable: simulatorExecutable,
        resolveMachine: () => ({
          root: fixture.shot,
          machine: join(fixture.local, "machine.ts"),
        }),
      });

      expect(result).toMatchObject({
        bundleId: "com.tohseno.simulator-shot",
        device: { udid: DEVICE_UDID },
      });
      expect(result.shotRoot).toBe(realpathSync(fixture.shot));
      expect(result.screenshotPath).not.toBeNull();
      if (result.screenshotPath !== null) {
        expect(existsSync(result.screenshotPath)).toBe(true);
      }
      expect(events.map((event) => event.type)).toEqual([
        "development-starting",
        "development-ready",
        "building",
        "simulator-launching",
        "simulator-launched",
        "screenshot-capturing",
        "screenshot-captured",
        "completed",
      ]);
      expect(commands.every((argv) => Array.isArray(argv) && argv.length > 1)).toBe(true);
      expect(commands.some((argv) => argv.some((part) => part.includes(";")))).toBe(false);
      expect(commands.at(-1)).toEqual([
        "/usr/bin/xcrun",
        "simctl",
        "io",
        DEVICE_UDID,
        "screenshot",
        "--type=png",
        expect.stringContaining(".screenshot-"),
      ]);
    });
  });

  test("uses an APP_BUNDLE_ID override resolved from the built app", async () => {
    await withScratchEnvironment(async (scratch) => {
      const fixture = writeShotFixture(scratch, "bundle-override");
      const configuredBundleId = "com.owner.custom-app";
      const config = join(fixture.shot, "Config");
      mkdirSync(config, { recursive: true });
      writeFileSync(
        join(config, "Local.xcconfig"),
        `APP_BUNDLE_ID = ${configuredBundleId}\n`,
      );
      const factoryCommands: string[][] = [];
      const resolved = await resolveBuiltBundleIdentifier(
        fixture.shot,
        fixture.app,
        "/usr/bin/plutil",
        async (argv) => {
          factoryCommands.push([...argv]);
          return {
            exitCode: 0,
            stdout: `${configuredBundleId}\n`,
            stderr: "",
          };
        },
      );
      expect(resolved).toBe(configuredBundleId);
      expect(factoryCommands).toEqual([[
        "/usr/bin/plutil",
        "-extract",
        "CFBundleIdentifier",
        "raw",
        "-o",
        "-",
        join(fixture.app, "Info.plist"),
      ]]);

      const commands: string[][] = [];
      const result = await runShotInSimulator({
        shotRoot: fixture.shot,
        environment: scratch.environment,
      }, {
        executor: async (argv) => {
          commands.push([...argv]);
          const machine = successfulMachineResponse(
            argv,
            fixture,
            configuredBundleId,
          );
          if (machine !== null) return machine;
          const plist = builtBundleIdentifierResponse(argv, configuredBundleId);
          if (plist !== null) return plist;
          if (argv[1] === "simctl" && argv[2] === "io") {
            return { exitCode: 1, stdout: "", stderr: "" };
          }
          throw new Error(`unexpected argv: ${JSON.stringify(argv)}`);
        },
        findExecutable: simulatorExecutable,
        resolveMachine: () => ({
          root: fixture.shot,
          machine: join(fixture.local, "machine.ts"),
        }),
      });

      expect(result.bundleId).toBe(configuredBundleId);
      expect(commands).toContainEqual([
        "/usr/bin/plutil",
        "-extract",
        "CFBundleIdentifier",
        "raw",
        "-o",
        "-",
        join(realpathSync(fixture.app), "Info.plist"),
      ]);
    });
  });

  test("rejects a launch bundle ID that does not match the resolved app", async () => {
    await withScratchEnvironment(async (scratch) => {
      const fixture = writeShotFixture(scratch, "bundle-mismatch");
      const operation = runShotInSimulator({
        shotRoot: fixture.shot,
        environment: scratch.environment,
      }, {
        executor: async (argv) => {
          const machine = successfulMachineResponse(
            argv,
            fixture,
            "com.owner.claimed",
          );
          if (machine !== null) return machine;
          const plist = builtBundleIdentifierResponse(argv, "com.owner.actual");
          if (plist !== null) return plist;
          throw new Error(`unexpected argv: ${JSON.stringify(argv)}`);
        },
        findExecutable: simulatorExecutable,
        resolveMachine: () => ({
          root: fixture.shot,
          machine: join(fixture.local, "machine.ts"),
        }),
      });

      await expect(operation).rejects.toMatchObject({
        code: "INVALID_MACHINE_RESPONSE",
        message: "The pinned shot machine returned an invalid launch result.",
      });
    });
  });

  test("keeps a launched shot running when contact-sheet capture fails", async () => {
    await withScratchEnvironment(async (scratch) => {
      const fixture = writeShotFixture(scratch, "capture-optional");
      const events: SimulatorProgressEvent[] = [];
      const result = await runShotInSimulator({
        shotRoot: fixture.shot,
        environment: scratch.environment,
        onProgress: (event) => {
          events.push(event);
        },
      }, {
        executor: async (argv) => {
          const machine = successfulMachineResponse(argv, fixture);
          if (machine !== null) return machine;
          const plist = builtBundleIdentifierResponse(
            argv,
            "com.tohseno.capture-optional",
          );
          if (plist !== null) return plist;
          if (argv[1] === "simctl" && argv[2] === "io") {
            return { exitCode: 1, stdout: "", stderr: "private simulator output" };
          }
          throw new Error(`unexpected argv: ${JSON.stringify(argv)}`);
        },
        findExecutable: simulatorExecutable,
        resolveMachine: () => ({
          root: fixture.shot,
          machine: join(fixture.local, "machine.ts"),
        }),
      });

      expect(result.screenshotPath).toBeNull();
      expect(events.at(-2)).toEqual({
        type: "screenshot-unavailable",
        code: "SCREENSHOT_FAILED",
        message: "The Simulator screenshot could not be captured.",
      });
      expect(events.at(-1)).toEqual({ type: "completed" });
      expect(events.some((event) => event.type === "failed")).toBe(false);
      expect(JSON.stringify(events)).not.toContain("private simulator output");
    });
  });

  test("turns pinned machine dependency failures into actionable diagnostics", async () => {
    await withScratchEnvironment(async (scratch) => {
      const fixture = writeShotFixture(scratch, "missing-runtime");
      const events: SimulatorProgressEvent[] = [];
      const operation = runShotInSimulator({
        shotRoot: fixture.shot,
        environment: scratch.environment,
        onProgress: (event) => {
          events.push(event);
        },
      }, {
        executor: async () => ({
          exitCode: 3,
          stdout: JSON.stringify({
            schemaVersion: 1,
            ok: false,
            operation: "dev.start",
            shot: fixture.shot,
            error: {
              code: "MISSING_DEPENDENCY",
              message: "private machine detail",
            },
          }),
          stderr: "private build output",
        }),
        resolveMachine: () => ({
          root: fixture.shot,
          machine: join(fixture.local, "machine.ts"),
        }),
      });

      await expect(operation).rejects.toMatchObject({
        code: "MACHINE_FAILED",
        message: expect.stringContaining("Run `tohseno doctor`"),
        details: {
          operation: "dev.start",
          machineCode: "MISSING_DEPENDENCY",
          exitCode: 3,
        },
      });
      expect(events.at(-1)).toMatchObject({
        type: "failed",
        code: "MACHINE_FAILED",
        message: expect.stringContaining("Run `tohseno doctor`"),
      });
      expect(JSON.stringify(events)).not.toContain("private");
    });
  });

  test("does not start live preview with an x64 selected Node", async () => {
    await withScratchEnvironment(async (scratch) => {
      const sidecar = join(scratch.root, "architecture-sidecar.mjs");
      writeFileSync(sidecar, "// test sidecar\n");
      let spawnCount = 0;
      const manager = new LivePreviewManager({
        platform: "darwin",
        architecture: "arm64",
        environment: { PATH: "/test/bin" },
        nodeExecutable: "/test/bin/node",
        sidecarPath: sidecar,
        temporaryRoot: scratch.root,
        executor: async (argv) => ({
          exitCode: 0,
          stdout: argv[1] === "--print" ? "x64\n" : "v22.0.0\n",
          stderr: "",
        }),
        spawner: async () => {
          spawnCount += 1;
          throw new Error("the helper must not start");
        },
      });

      await expect(manager.start({ deviceUdid: DEVICE_UDID })).rejects.toMatchObject({
        code: "UNSUPPORTED_NODE",
        message: "Live preview requires an arm64 Node.js binary on Apple Silicon.",
      });
      expect(spawnCount).toBe(0);
    });
  });

  test("starts one capability-scoped helper and tears down its exact process", async () => {
    await withScratchEnvironment(async (scratch) => {
      const sidecar = join(scratch.root, "sidecar.mjs");
      writeFileSync(sidecar, "// test sidecar\n");
      let resolveExit!: (value: SpawnedProcessExit) => void;
      const exited = new Promise<SpawnedProcessExit>((resolveValue) => {
        resolveExit = resolveValue;
      });
      const killed: NodeJS.Signals[] = [];
      const spawned: string[][] = [];
      const spawner: ProcessSpawner = async (argv) => {
        spawned.push([...argv]);
        return {
          pid: 4242,
          stdout: (async function* () {
            yield `${JSON.stringify({
              schemaVersion: 1,
              event: "ready",
              host: "127.0.0.1",
              port: 32123,
              device: DEVICE_UDID,
            })}\n`;
          })(),
          stderr: (async function* () {})(),
          exited,
          kill: (signal) => {
            killed.push(signal);
            resolveExit({ exitCode: 0, signal });
            return true;
          },
        };
      };
      const manager = new LivePreviewManager({
        platform: "darwin",
        architecture: "arm64",
        environment: { PATH: "/test/bin" },
        nodeExecutable: "/test/bin/node",
        sidecarPath: sidecar,
        temporaryRoot: scratch.root,
        randomCapability: () => "A".repeat(43),
        resolveServeSim: () => ({
          packageJsonPath: "/test/serve-sim/package.json",
          middlewarePath: "/test/serve-sim/middleware.js",
          version: SERVE_SIM_VERSION,
          middlewareExport: true,
        }),
        executor: async (argv) => compatibleNodeResponse(argv),
        spawner,
      });

      const handle = await manager.start({ deviceUdid: DEVICE_UDID });
      expect(handle.iframeUrl()).toBe(
        `http://127.0.0.1:32123/_tohseno/live/${"A".repeat(43)}`,
      );
      expect(JSON.stringify(handle)).not.toContain("A".repeat(43));
      expect(manager.status()).toMatchObject({
        active: true,
        deviceUdid: DEVICE_UDID,
        pid: 4242,
      });
      await expect(manager.start({ deviceUdid: DEVICE_UDID }))
        .rejects.toThrow("Only one Studio live preview");
      expect(spawned).toEqual([["/test/bin/node", sidecar]]);
      const service = new SimulatorService({ livePreview: manager });
      await expect(service.runShot({ shotRoot: join(scratch.root, "other-shot") }))
        .rejects.toMatchObject({
          code: "LIVE_PREVIEW_BUSY",
          message: "Stop the active Studio live preview before running another shot.",
        });

      await handle.stop();
      expect(killed).toEqual(["SIGTERM"]);
      expect(manager.status().active).toBe(false);
      expect(
        readdirSync(scratch.root).some((entry) =>
          entry.startsWith("tohseno-studio-sim-")),
      ).toBe(false);
    });
  });

  test("stop and dispose cancel and await a helper that is still starting", async () => {
    await withScratchEnvironment(async (scratch) => {
      const sidecar = join(scratch.root, "starting-sidecar.mjs");
      writeFileSync(sidecar, "// test sidecar\n");
      let reportSpawned!: () => void;
      const spawned = new Promise<void>((resolveSpawned) => {
        reportSpawned = resolveSpawned;
      });
      let resolveExit!: (value: SpawnedProcessExit) => void;
      const exited = new Promise<SpawnedProcessExit>((resolveValue) => {
        resolveExit = resolveValue;
      });
      const killed: NodeJS.Signals[] = [];
      const manager = new LivePreviewManager({
        platform: "darwin",
        architecture: "arm64",
        environment: { PATH: "/test/bin" },
        nodeExecutable: "/test/bin/node",
        sidecarPath: sidecar,
        temporaryRoot: scratch.root,
        randomCapability: () => "B".repeat(43),
        resolveServeSim: () => ({
          packageJsonPath: "/test/serve-sim/package.json",
          middlewarePath: "/test/serve-sim/middleware.js",
          version: SERVE_SIM_VERSION,
          middlewareExport: true,
        }),
        executor: async (argv) => compatibleNodeResponse(argv),
        spawner: async () => {
          reportSpawned();
          return {
            pid: 4343,
            stdout: (async function* () {
              await new Promise<void>(() => {});
            })(),
            stderr: (async function* () {})(),
            exited,
            kill: (signal) => {
              killed.push(signal);
              resolveExit({ exitCode: 0, signal });
              return true;
            },
          };
        },
      });

      const start = manager.start({ deviceUdid: DEVICE_UDID });
      await spawned;
      const service = new SimulatorService({ livePreview: manager });
      await expect(service.runShot({ shotRoot: join(scratch.root, "other-shot") }))
        .rejects.toMatchObject({ code: "LIVE_PREVIEW_BUSY" });
      const stop = manager.stop();
      await expect(start).rejects.toMatchObject({ code: "ABORTED" });
      await stop;

      expect(killed).toEqual(["SIGTERM"]);
      expect(manager.status().active).toBe(false);
      expect(
        readdirSync(scratch.root).some((entry) =>
          entry.startsWith("tohseno-studio-sim-")),
      ).toBe(false);
      await manager.dispose();
      await expect(manager.start({ deviceUdid: DEVICE_UDID }))
        .rejects.toThrow("already shut down");
    });
  });

  test("surfaces a structured serve-sim helper startup failure", async () => {
    await withScratchEnvironment(async (scratch) => {
      const sidecar = join(scratch.root, "failing-sidecar.mjs");
      writeFileSync(sidecar, "// test sidecar\n");
      const manager = new LivePreviewManager({
        platform: "darwin",
        architecture: "arm64",
        environment: { PATH: "/test/bin" },
        nodeExecutable: "/test/bin/node",
        sidecarPath: sidecar,
        temporaryRoot: scratch.root,
        randomCapability: () => "C".repeat(43),
        resolveServeSim: () => ({
          packageJsonPath: "/test/serve-sim/package.json",
          middlewarePath: "/test/serve-sim/middleware.js",
          version: SERVE_SIM_VERSION,
          middlewareExport: true,
        }),
        executor: async (argv) => compatibleNodeResponse(argv),
        spawner: async () => ({
          pid: 4444,
          stdout: (async function* () {})(),
          stderr: (async function* () {
            yield `${JSON.stringify({
              schemaVersion: 1,
              event: "failed",
              code: "SERVE_SIM_IMPORT",
            })}\n`;
          })(),
          exited: Promise.resolve({ exitCode: 1, signal: null }),
          kill: () => true,
        }),
      });

      await expect(manager.start({ deviceUdid: DEVICE_UDID })).rejects.toMatchObject({
        code: "SERVE_SIM_UNAVAILABLE",
        message: expect.stringContaining("Reinstall Tohseno"),
        details: { helperCode: "SERVE_SIM_IMPORT" },
      });
      expect(
        readdirSync(scratch.root).some((entry) =>
          entry.startsWith("tohseno-studio-sim-")),
      ).toBe(false);
    });
  });

  test("CLI Simulator doors install cancellation before invoking their service", async () => {
    await withScratchEnvironment(async (scratch) => {
      const fixture = writeShotFixture(scratch, "cli-signal");
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const runIo = createMemoryIo();
      const runController = new AbortController();
      const runOrder: string[] = [];
      const runCancellation: CommandCancellation = {
        signal: runController.signal,
        close: () => {
          runOrder.push("run-close");
        },
      };
      const runResult: ShotRunResult = {
        shotRoot: fixture.shot,
        device: {
          name: "iPhone Test",
          udid: DEVICE_UDID,
          state: "Booted",
          runtime: "test",
          available: true,
        },
        bundleId: "com.tohseno.cli-signal",
        appPath: fixture.app,
        screenshotPath: null,
      };
      const runService = new class extends SimulatorService {
        override async runShot(options: RunShotOptions): Promise<ShotRunResult> {
          runOrder.push("run-service");
          expect(options.signal).toBe(runController.signal);
          return runResult;
        }
      }();

      expect(await runCommand("cli-signal", {
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        io: runIo,
      }, runService, {
        cancellation: () => {
          runOrder.push("run-cancellation");
          return runCancellation;
        },
      })).toBe(0);
      expect(runOrder).toEqual([
        "run-cancellation",
        "run-service",
        "run-close",
      ]);
      expect(runIo.stdout.join("\n")).not.toContain("Screenshot: null");

      const previewIo = createMemoryIo();
      const previewController = new AbortController();
      const previewOrder: string[] = [];
      const previewHandle: LivePreviewHandle = {
        deviceUdid: DEVICE_UDID,
        host: "127.0.0.1",
        port: 4748,
        iframeUrl: () => "http://127.0.0.1:4748/_tohseno/live/test",
        stop: async () => {
          previewOrder.push("preview-stop");
        },
        toJSON: () => ({
          active: true,
          deviceUdid: DEVICE_UDID,
          host: "127.0.0.1",
          port: 4748,
          pid: 4545,
        }),
      };
      const previewService = new class extends SimulatorService {
        override async runAndPreview(
          options: RunShotOptions,
        ): Promise<{ run: ShotRunResult; preview: LivePreviewHandle }> {
          previewOrder.push("preview-service");
          expect(options.signal).toBe(previewController.signal);
          return { run: runResult, preview: previewHandle };
        }

        override async dispose(): Promise<void> {
          previewOrder.push("preview-dispose");
        }
      }();

      expect(await previewCommand("cli-signal", {
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        io: previewIo,
      }, {
        service: previewService,
        cancellation: () => {
          previewOrder.push("preview-cancellation");
          return {
            signal: previewController.signal,
            close: () => {
              previewOrder.push("preview-close");
            },
          };
        },
        openUrl: async () => {
          previewOrder.push("preview-open");
        },
        wait: async (_service, signal) => {
          previewOrder.push("preview-wait");
          expect(signal).toBe(previewController.signal);
        },
      })).toBe(0);
      expect(previewOrder).toEqual([
        "preview-cancellation",
        "preview-service",
        "preview-open",
        "preview-wait",
        "preview-stop",
        "preview-dispose",
        "preview-close",
      ]);
    });
  });

  test("process cancellation listeners are removable", () => {
    const sigint = process.listenerCount("SIGINT");
    const sigterm = process.listenerCount("SIGTERM");
    const cancellation = createCommandCancellation();
    expect(process.listenerCount("SIGINT")).toBe(sigint + 1);
    expect(process.listenerCount("SIGTERM")).toBe(sigterm + 1);
    cancellation.close();
    expect(process.listenerCount("SIGINT")).toBe(sigint);
    expect(process.listenerCount("SIGTERM")).toBe(sigterm);
  });
});
