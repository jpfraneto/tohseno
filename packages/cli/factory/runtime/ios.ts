import { accessSync, constants as fsConstants, existsSync, statSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { developmentStatus } from "./dev.ts";
import {
  appendStructuredLog,
  MachineError,
  runCaptured,
  readJson,
  runtimePaths,
  safeEnvironment,
} from "./shared.ts";

interface SimulatorDevice {
  name: string;
  udid: string;
  state: string;
  isAvailable: boolean;
}

interface SimctlDevices {
  devices?: Record<string, Array<Partial<SimulatorDevice>>>;
}

const BUNDLE_IDENTIFIER = /^[A-Za-z0-9]+(?:\.[A-Za-z0-9-]+)+$/u;
const XCODE_NAME = /^[A-Za-z0-9._-]+$/u;

interface AppOperations {
  project: string;
  scheme: string;
  product: string;
  requiresDevelopmentService: boolean;
}

function appOperations(root: string): AppOperations {
  const genericPath = join(root, "app.manifest.json");
  if (!existsSync(genericPath)) {
    return {
      project: "Writing.xcodeproj",
      scheme: "Writing",
      product: "Writing",
      requiresDevelopmentService: true,
    };
  }
  const manifest = readJson<{
    kind?: unknown;
    platform?: unknown;
    operations?: {
      project?: unknown;
      scheme?: unknown;
      product?: unknown;
    };
  }>(genericPath);
  const project = manifest.operations?.project;
  const scheme = manifest.operations?.scheme;
  const product = manifest.operations?.product;
  if (
    manifest.kind !== "app" ||
    manifest.platform !== "ios" ||
    typeof project !== "string" ||
    !XCODE_NAME.test(project.replace(/\.xcodeproj$/u, "")) ||
    !project.endsWith(".xcodeproj") ||
    typeof scheme !== "string" ||
    !XCODE_NAME.test(scheme) ||
    typeof product !== "string" ||
    !XCODE_NAME.test(product)
  ) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "app.manifest has invalid iOS machine operations",
    );
  }
  return {
    project,
    scheme,
    product,
    requiresDevelopmentService: false,
  };
}

function executable(name: string, pathValue = process.env.PATH ?? ""): string | null {
  for (const directory of pathValue.split(delimiter).filter(Boolean)) {
    const path = resolve(directory, name);
    try {
      accessSync(path, fsConstants.X_OK);
      if (statSync(path).isFile()) return path;
    } catch {
      // Continue.
    }
  }
  return null;
}

async function availableSimulators(root: string, xcrun: string): Promise<SimulatorDevice[]> {
  const result = await runCaptured([xcrun, "simctl", "list", "devices", "available", "--json"], {
    cwd: root,
    environment: safeEnvironment(),
  });
  if (result.exitCode !== 0) return [];
  let value: SimctlDevices;
  try {
    value = JSON.parse(result.stdout) as SimctlDevices;
  } catch {
    return [];
  }
  return Object.values(value.devices ?? {})
    .flat()
    .filter((device): device is SimulatorDevice =>
      typeof device.name === "string" &&
      device.name.startsWith("iPhone") &&
      typeof device.udid === "string" &&
      typeof device.state === "string" &&
      device.isAvailable !== false
    )
    .sort((left, right) => left.name.localeCompare(right.name));
}

export async function inspectIos(root: string): Promise<{
  implemented: true;
  xcode: { available: boolean; xcodebuild: string | null; xcrun: string | null };
  simulator: { available: boolean; devices: SimulatorDevice[] };
  development: Awaited<ReturnType<typeof developmentStatus>>;
  project: string;
  scheme: string;
  launchOperation: string;
  physicalDevice: {
    automaticLaunch: false;
    requiresSigningTeam: true;
    requiresQuickTunnelForRemoteApi: true;
    guidance: string;
  };
}> {
  const xcodebuild = executable("xcodebuild");
  const xcrun = executable("xcrun");
  const devices = xcrun ? await availableSimulators(root, xcrun) : [];
  const operations = appOperations(root);
  return {
    implemented: true,
    xcode: { available: xcodebuild !== null && xcrun !== null, xcodebuild, xcrun },
    simulator: { available: devices.length > 0, devices },
    development: await developmentStatus(root),
    project: join(root, operations.project),
    scheme: operations.scheme,
    launchOperation: "tohseno machine ios launch --json",
    physicalDevice: {
      automaticLaunch: false,
      requiresSigningTeam: true,
      requiresQuickTunnelForRemoteApi: true,
      guidance: "Start development with --tunnel, select a signing team, then run the Debug app on the connected device in Xcode.",
    },
  };
}

async function capturedToLog(
  root: string,
  log: string,
  command: readonly string[],
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const result = await runCaptured(command, { cwd: root, environment: safeEnvironment() });
  appendStructuredLog(log, {
    event: "ios_command",
    executable: command[0] ? command[0].split("/").at(-1) : "unknown",
    exitCode: result.exitCode,
    stdoutBytes: Buffer.byteLength(result.stdout),
    stderrBytes: Buffer.byteLength(result.stderr),
  });
  return result;
}

export async function resolveBuiltBundleIdentifier(
  root: string,
  appPath: string,
  plutil: string,
  capture: typeof runCaptured = runCaptured,
): Promise<string> {
  const extracted = await capture(
    [plutil, "-extract", "CFBundleIdentifier", "raw", "-o", "-", join(appPath, "Info.plist")],
    { cwd: root, environment: safeEnvironment() },
  );
  const identifier = extracted.exitCode === 0 ? extracted.stdout.trim() : "";
  if (!BUNDLE_IDENTIFIER.test(identifier)) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "the built iOS app has no valid CFBundleIdentifier",
    );
  }
  return identifier;
}

export async function launchIos(root: string, requestedUdid?: string): Promise<{
  launched: true;
  device: SimulatorDevice;
  bundleId: string;
  endpoint: string | null;
  endpointMatchesBuiltApp: boolean;
  appPath: string;
  logs: string;
}> {
  const xcodebuild = executable("xcodebuild");
  const xcrun = executable("xcrun");
  if (!xcodebuild || !xcrun) {
    throw new MachineError(
      "MISSING_DEPENDENCY",
      "Xcode command-line tools are required to launch the iOS simulator",
      { dependency: !xcodebuild ? "xcodebuild" : "xcrun" },
    );
  }
  const operations = appOperations(root);
  const development = await developmentStatus(root);
  if (
    operations.requiresDevelopmentService &&
    (!development.healthy ||
      !development.endpoint.configured ||
      !development.endpoint.url)
  ) {
    throw new MachineError(
      "UNHEALTHY_SERVICES",
      "start the shot development environment before launching iOS",
      { operation: "tohseno machine dev start --json" },
    );
  }
  const devices = await availableSimulators(root, xcrun);
  const device = requestedUdid
    ? devices.find((candidate) => candidate.udid === requestedUdid)
    : devices[0];
  if (!device) {
    throw new MachineError(
      "MISSING_DEPENDENCY",
      requestedUdid
        ? `the requested iPhone simulator is unavailable: ${requestedUdid}`
        : "no available iPhone simulator is installed; add one in Xcode Settings > Platforms",
      { dependency: "ios-simulator" },
    );
  }

  const paths = runtimePaths(root);
  const derivedData = join(paths.runtime, "DerivedData");
  const log = paths.iosLog;
  if (device.state !== "Booted") {
    const boot = await capturedToLog(root, log, [xcrun, "simctl", "boot", device.udid]);
    if (boot.exitCode !== 0 && !/current state: Booted/iu.test(`${boot.stdout}\n${boot.stderr}`)) {
      throw new MachineError("UNHEALTHY_SERVICES", "the iPhone simulator could not boot", { logs: log });
    }
  }
  const bootStatus = await capturedToLog(root, log, [xcrun, "simctl", "bootstatus", device.udid, "-b"]);
  if (bootStatus.exitCode !== 0) {
    throw new MachineError("UNHEALTHY_SERVICES", "the iPhone simulator did not finish booting", { logs: log });
  }

  const build = await capturedToLog(root, log, [
    xcodebuild,
    "-project", join(root, operations.project),
    "-scheme", operations.scheme,
    "-configuration", "Debug",
    "-destination", `platform=iOS Simulator,id=${device.udid}`,
    "-derivedDataPath", derivedData,
    "build",
  ]);
  if (build.exitCode !== 0) {
    throw new MachineError("UNHEALTHY_SERVICES", "the Debug iOS build failed", { logs: log });
  }
  const appPath = join(
    derivedData,
    "Build",
    "Products",
    "Debug-iphonesimulator",
    `${operations.product}.app`,
  );
  if (!existsSync(appPath)) {
    throw new MachineError("INTERNAL_FAILURE", "xcodebuild succeeded but the simulator app bundle is missing", { appPath, logs: log });
  }
  const plist = join(appPath, "Info.plist");
  const plutil = executable("plutil", "/usr/bin:/bin");
  if (!plutil) {
    throw new MachineError(
      "MISSING_DEPENDENCY",
      "property list tooling is required to inspect the built iOS app",
      { dependency: "plutil" },
    );
  }
  const identifier = await resolveBuiltBundleIdentifier(root, appPath, plutil);
  const install = await capturedToLog(root, log, [xcrun, "simctl", "install", device.udid, appPath]);
  if (install.exitCode !== 0) {
    throw new MachineError("UNHEALTHY_SERVICES", "the app could not be installed in the simulator", { logs: log });
  }
  const launch = await capturedToLog(root, log, [xcrun, "simctl", "launch", device.udid, identifier]);
  if (launch.exitCode !== 0) {
    throw new MachineError("UNHEALTHY_SERVICES", "the app could not be launched in the simulator", { logs: log });
  }

  let builtEndpoint: string | null = null;
  const extracted = await runCaptured([plutil, "-extract", "TohsenoAPIBaseURL", "raw", "-o", "-", plist], {
    cwd: root,
    environment: safeEnvironment(),
  });
  if (extracted.exitCode === 0) builtEndpoint = extracted.stdout.trim();
  const expectedEndpoint = development.endpoint.url;
  const endpointMatchesBuiltApp =
    !operations.requiresDevelopmentService ||
    builtEndpoint === null ||
    builtEndpoint === expectedEndpoint;
  if (!endpointMatchesBuiltApp) {
    throw new MachineError(
      "UNHEALTHY_SERVICES",
      "the built iOS app endpoint does not match the active development endpoint",
      { activeEndpoint: expectedEndpoint, builtEndpoint, logs: log },
    );
  }
  const open = executable("open", "/usr/bin:/bin");
  if (open) void runCaptured([open, "-a", "Simulator"], { cwd: root, environment: safeEnvironment() });

  return {
    launched: true,
    device,
    bundleId: identifier,
    endpoint: operations.requiresDevelopmentService ? expectedEndpoint : null,
    endpointMatchesBuiltApp,
    appPath,
    logs: log,
  };
}
