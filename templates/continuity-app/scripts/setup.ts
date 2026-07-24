/**
 * Machine-local owner setup: `bun run setup`
 *
 * Writes public identifiers and credential paths to gitignored files. The
 * App Store Connect private key is read only to authenticate one validation
 * request; its value is never copied, printed, or written to configuration.
 */

import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  openSync,
  readSync,
  realpathSync,
  readdirSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import {
  basename,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
} from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const TEAM_ID_PATTERN = /^[A-Z0-9]{10}$/;
const KEY_ID_PATTERN = /^[A-Z0-9]{10}$/;
const ISSUER_ID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const ASC_API_URL = "https://api.appstoreconnect.apple.com/v1/apps?limit=1";
const MAX_SETUP_FILE_BYTES = 1_048_576;
const MAX_PRIVATE_KEY_BYTES = 16_384;
const MAX_CAPTURED_OUTPUT_BYTES = 1_048_576;

function readDescriptorBounded(
  descriptor: number,
  maximumBytes: number,
  label: string,
): Buffer {
  const chunks: Buffer[] = [];
  const buffer = Buffer.allocUnsafe(65_536);
  let total = 0;
  while (true) {
    const length = readSync(descriptor, buffer, 0, buffer.length, null);
    if (length === 0) break;
    total += length;
    if (total > maximumBytes) {
      throw new Error(`${label} grew past its ${maximumBytes}-byte limit`);
    }
    chunks.push(Buffer.from(buffer.subarray(0, length)));
  }
  return Buffer.concat(chunks, total);
}

const OWNED_XCCONFIG_KEYS = [
  "APP_DISPLAY_NAME",
  "APP_BUNDLE_ID",
  "DEVELOPMENT_TEAM",
  "REVENUECAT_PUBLIC_KEY",
] as const;

const ENVIRONMENT = {
  fromManifest: "TOHSENO_FROM_MANIFEST",
  team: "TOHSENO_APPLE_TEAM_ID",
  ascKeyPath: "TOHSENO_ASC_KEY_PATH",
  ascKeyId: "TOHSENO_ASC_KEY_ID",
  ascIssuerId: "TOHSENO_ASC_ISSUER_ID",
  revenueCatPublicKey: "TOHSENO_REVENUECAT_PUBLIC_KEY",
} as const;

export interface SetupConfig {
  displayName: string;
  bundleId: string;
  teamId: string;
  appStoreConnect?: AppStoreConnectConfig;
  revenueCatPublicKey?: string;
}

export interface AppStoreConnectConfig {
  keyPath: string;
  keyId: string;
  issuerId: string;
}

interface ManifestDefaults {
  displayName: string;
  bundleId: string;
}

interface SetupOptions {
  fromManifest: boolean;
  help: boolean;
  team?: string;
  ascKeyPath?: string;
  ascKeyId?: string;
  ascIssuerId?: string;
  revenueCatPublicKey?: string;
}

interface CommandResult {
  exitCode: number;
  stdout: string;
}

type Environment = Record<string, string | undefined>;
type CaptureCommand = (command: string[]) => Promise<CommandResult>;
type Fetcher = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

interface Output {
  write(value: string): void;
  line(value?: string): void;
}

export interface SetupDependencies {
  root?: string;
  environment?: Environment;
  stdinLines?: AsyncIterator<unknown>;
  output?: Output;
  captureCommand?: CaptureCommand;
  validateAppStoreConnect?: (config: AppStoreConnectConfig) => Promise<void>;
}

interface TeamCandidate {
  teamId: string;
  source: string;
}

const defaultOutput: Output = {
  write(value) {
    process.stdout.write(value);
  },
  line(value = "") {
    console.log(value);
  },
};

class Prompter {
  private inputEnded = false;

  constructor(
    private readonly stdinLines: AsyncIterator<unknown>,
    private readonly output: Output,
  ) {}

  async ask(question: string, fallback: string, fallbackLabel = fallback): Promise<string> {
    const suffix = fallback ? ` [${fallbackLabel}]` : " (press enter to skip)";
    this.output.write(`${question}${suffix}: `);
    const line = await this.stdinLines.next();
    this.inputEnded = Boolean(line.done);
    const answer = line.done ? "" : String(line.value).trim();
    if (line.done) this.output.write("\n");
    return answer || fallback;
  }

  async askValidated(
    context: string,
    question: string,
    fallback: string,
    validate: (value: string) => string | null,
    fallbackLabel = fallback,
  ): Promise<string> {
    while (true) {
      this.output.line(context);
      const answer = await this.ask(question, fallback, fallbackLabel);
      const problem = validate(answer);
      if (problem === null) return answer;
      this.output.line(`  ${problem}`);
      if (this.inputEnded) throw new Error("Input ended before setup was complete.");
    }
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function displayNameProblem(value: string): string | null {
  return value.length > 0 &&
      value.length <= 80 &&
      !/[\r\n=$\\]/u.test(value) &&
      !value.includes("//") &&
      !value.includes("/*") &&
      !value.includes("*/")
    ? null
    : "Use 1–80 characters without line breaks or Xcode configuration syntax.";
}

function bundleIdProblem(value: string): string | null {
  return /^[A-Za-z0-9]+(\.[A-Za-z0-9-]+)+$/.test(value)
    ? null
    : "A bundle ID looks like com.yourname.yourapp.";
}

function teamIdProblem(value: string, allowEmpty: boolean): string | null {
  if (allowEmpty && value === "") return null;
  return TEAM_ID_PATTERN.test(value)
    ? null
    : "A Team ID is 10 uppercase letters/digits.";
}

function ascKeyPathProblem(
  value: string,
  environment: Environment = process.env,
): string | null {
  if (value === "") return null;
  if (!value.toLowerCase().endsWith(".p8")) return "That should be a .p8 file path.";
  const path = expandPath(value, environment);
  try {
    const details = lstatSync(path);
    if (
      details.isSymbolicLink() ||
      !details.isFile() ||
      details.nlink !== 1
    ) {
      return "The .p8 path must be a single-link regular file.";
    }
    if (details.size > MAX_PRIVATE_KEY_BYTES) {
      return "The .p8 file is unexpectedly large.";
    }
    if ((details.mode & 0o077) !== 0) {
      return "Protect the .p8 file with owner-only permissions (chmod 600).";
    }
    return null;
  } catch {
    return "No file exists at that path.";
  }
}

function revenueCatKeyProblem(value: string): string | null {
  return value === "" || /^[A-Za-z0-9_]{10,}$/.test(value)
    ? null
    : "That doesn't look like a RevenueCat public key.";
}

export function deriveBundleId(displayName: string): string {
  const slug = displayName
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "")
    .slice(0, 40);
  return `com.${slug || "myapp"}.app`;
}

function expandPath(value: string, environment: Environment = process.env): string {
  const homeDirectory = environment.HOME;
  if (value === "~" && homeDirectory) return homeDirectory;
  if (value.startsWith("~/") && homeDirectory) return join(homeDirectory, value.slice(2));
  return resolve(value);
}

function booleanEnvironmentValue(value: string | undefined): boolean {
  if (value === undefined || value === "" || value === "0" || value.toLowerCase() === "false") {
    return false;
  }
  return true;
}

function optionValue(arguments_: string[], index: number, option: string): string {
  const value = arguments_[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value.`);
  }
  return value;
}

export function parseSetupOptions(
  arguments_: string[],
  environment: Environment = process.env,
): SetupOptions {
  const values = new Map<string, string>();
  let fromManifest = booleanEnvironmentValue(environment[ENVIRONMENT.fromManifest]);
  let help = false;

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]!;
    if (argument === "--from-manifest") {
      fromManifest = true;
    } else if (argument === "--help" || argument === "-h") {
      help = true;
    } else if (
      argument === "--team" ||
      argument === "--asc-key" ||
      argument === "--asc-key-id" ||
      argument === "--asc-issuer-id" ||
      argument === "--revenuecat-key"
    ) {
      values.set(argument, optionValue(arguments_, index, argument));
      index += 1;
    } else {
      throw new Error(`Unknown setup option: ${argument}`);
    }
  }

  const options: SetupOptions = { fromManifest, help };
  const team = values.get("--team") ?? environment[ENVIRONMENT.team];
  const ascKeyPath = values.get("--asc-key") ?? environment[ENVIRONMENT.ascKeyPath];
  const ascKeyId = values.get("--asc-key-id") ?? environment[ENVIRONMENT.ascKeyId];
  const ascIssuerId = values.get("--asc-issuer-id") ?? environment[ENVIRONMENT.ascIssuerId];
  const revenueCatPublicKey =
    values.get("--revenuecat-key") ?? environment[ENVIRONMENT.revenueCatPublicKey];

  if (team !== undefined && team !== "") options.team = team;
  if (ascKeyPath !== undefined && ascKeyPath !== "") options.ascKeyPath = ascKeyPath;
  if (ascKeyId !== undefined && ascKeyId !== "") options.ascKeyId = ascKeyId;
  if (ascIssuerId !== undefined && ascIssuerId !== "") options.ascIssuerId = ascIssuerId;
  if (revenueCatPublicKey !== undefined && revenueCatPublicKey !== "") {
    options.revenueCatPublicKey = revenueCatPublicKey;
  }
  return options;
}

function printUsage(output: Output): void {
  output.line("Usage:");
  output.line("  bun run setup");
  output.line("  bun run setup --from-manifest --team <id|auto> [--asc-key <path>]");
  output.line("");
  output.line("App Store Connect options (needed when --asc-key is used):");
  output.line("  --asc-key-id <id> --asc-issuer-id <uuid>");
  output.line("");
  output.line("Environment equivalents:");
  output.line(`  ${ENVIRONMENT.fromManifest}=1`);
  output.line(`  ${ENVIRONMENT.team}=<id|auto>`);
  output.line(`  ${ENVIRONMENT.ascKeyPath}=<path>`);
  output.line(`  ${ENVIRONMENT.ascKeyId}=<id>`);
  output.line(`  ${ENVIRONMENT.ascIssuerId}=<uuid>`);
  output.line(`  ${ENVIRONMENT.revenueCatPublicKey}=<public-key>`);
}

function missingFile(error: unknown): boolean {
  return isRecord(error) && error.code === "ENOENT";
}

function assertParentInsideRoot(root: string, path: string): void {
  const rootPath = realpathSync(root);
  const parentPath = realpathSync(dirname(path));
  const difference = relative(rootPath, parentPath);
  if (
    difference === ".." ||
    difference.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`) ||
    isAbsolute(difference)
  ) {
    throw new Error(`Refusing setup path outside the workspace: ${path}`);
  }
  const parent = lstatSync(dirname(path));
  if (parent.isSymbolicLink() || !parent.isDirectory()) {
    throw new Error(`Refusing non-directory setup parent: ${dirname(path)}`);
  }
}

export function readOptionalSetupFile(
  root: string,
  path: string,
  label: string,
): string | null {
  assertParentInsideRoot(root, path);
  try {
    const details = lstatSync(path);
    if (
      details.isSymbolicLink() ||
      !details.isFile() ||
      details.nlink !== 1 ||
      details.size > MAX_SETUP_FILE_BYTES
    ) {
      throw new Error(`Refusing non-regular ${label}: ${path}`);
    }
    const descriptor = openSync(
      path,
      constants.O_RDONLY | constants.O_NOFOLLOW,
    );
    try {
      const opened = fstatSync(descriptor);
      if (
        !opened.isFile() ||
        opened.nlink !== 1 ||
        opened.dev !== details.dev ||
        opened.ino !== details.ino ||
        opened.size > MAX_SETUP_FILE_BYTES
      ) {
        throw new Error(`Refusing non-regular ${label}: ${path}`);
      }
      return new TextDecoder("utf-8", { fatal: true }).decode(
        readDescriptorBounded(descriptor, MAX_SETUP_FILE_BYTES, label),
      );
    } finally {
      closeSync(descriptor);
    }
  } catch (error) {
    if (missingFile(error)) return null;
    throw error;
  }
}

export function writePrivateSetupFile(
  root: string,
  path: string,
  content: string,
  label: string,
): void {
  assertParentInsideRoot(root, path);
  // Recheck the final target immediately before replacement. A setup file may
  // be absent or regular, but never a link, directory, device, or socket.
  readOptionalSetupFile(root, path, label);
  const temporary = join(
    dirname(path),
    `.${basename(path)}.writing-${process.pid}-${crypto.randomUUID()}`,
  );
  let descriptor: number | null = null;
  try {
    descriptor = openSync(temporary, "wx", 0o600);
    writeFileSync(descriptor, content, "utf8");
    fchmodSync(descriptor, 0o600);
    fsyncSync(descriptor);
    closeSync(descriptor);
    descriptor = null;
    renameSync(temporary, path);
  } finally {
    if (descriptor !== null) closeSync(descriptor);
    try {
      unlinkSync(temporary);
    } catch (error) {
      if (!missingFile(error)) throw error;
    }
  }
}

export async function readManifestDefaults(root: string): Promise<ManifestDefaults> {
  const manifestPath = join(root, "continuity.manifest.json");
  const source = readOptionalSetupFile(
    root,
    manifestPath,
    "continuity manifest",
  );
  if (source === null) {
    throw new Error(`Missing ${manifestPath}; setup needs the workspace manifest.`);
  }
  let manifest: unknown;
  try {
    manifest = JSON.parse(source) as unknown;
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Could not read JSON at ${manifestPath}: ${detail}`);
  }
  const application = isRecord(manifest) && isRecord(manifest.application)
    ? manifest.application
    : null;
  if (
    application === null ||
    typeof application.name !== "string" ||
    typeof application.id !== "string"
  ) {
    throw new Error("continuity.manifest.json needs application.name and application.id strings.");
  }
  return { displayName: application.name, bundleId: application.id };
}

async function readExistingConfig(root: string): Promise<Partial<SetupConfig>> {
  const configPath = join(root, "app.config.json");
  const source = readOptionalSetupFile(root, configPath, "app config");
  if (source === null) return {};
  let value: unknown;
  try {
    value = JSON.parse(source) as unknown;
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Could not read JSON at ${configPath}: ${detail}`);
  }
  if (!isRecord(value)) throw new Error("app.config.json must contain an object.");

  const result: Partial<SetupConfig> = {};
  if (typeof value.displayName === "string") result.displayName = value.displayName;
  if (typeof value.bundleId === "string") result.bundleId = value.bundleId;
  if (typeof value.teamId === "string") result.teamId = value.teamId;
  if (typeof value.revenueCatPublicKey === "string") {
    result.revenueCatPublicKey = value.revenueCatPublicKey;
  }
  if (
    isRecord(value.appStoreConnect) &&
    typeof value.appStoreConnect.keyPath === "string" &&
    typeof value.appStoreConnect.keyId === "string" &&
    typeof value.appStoreConnect.issuerId === "string"
  ) {
    result.appStoreConnect = {
      keyPath: value.appStoreConnect.keyPath,
      keyId: value.appStoreConnect.keyId,
      issuerId: value.appStoreConnect.issuerId,
    };
  }
  return result;
}

async function defaultCaptureCommand(command: string[]): Promise<CommandResult> {
  try {
    const executable = command[0] === "defaults"
      ? "/usr/bin/defaults"
      : command[0] === "security"
        ? "/usr/bin/security"
        : null;
    if (executable === null) return { exitCode: 1, stdout: "" };
    const child = Bun.spawn([executable, ...command.slice(1)], {
      env: {
        PATH: "/usr/bin:/bin",
        ...(process.env.HOME === undefined
          ? {}
          : { HOME: process.env.HOME }),
        ...(process.env.LANG === undefined
          ? {}
          : { LANG: process.env.LANG }),
      },
      stdin: "ignore",
      stdout: "pipe",
      stderr: "ignore",
    });
    const reader = child.stdout.getReader();
    const chunks: Uint8Array[] = [];
    let length = 0;
    let exceeded = false;
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      length += next.value.byteLength;
      if (length > MAX_CAPTURED_OUTPUT_BYTES) {
        exceeded = true;
        child.kill("SIGKILL");
        break;
      }
      chunks.push(next.value);
    }
    const [exitCode, stdout] = await Promise.all([
      child.exited,
      Promise.resolve(
        exceeded
          ? ""
          : Buffer.concat(chunks.map((chunk) => Buffer.from(chunk)))
            .toString("utf8"),
      ),
    ]);
    return { exitCode: exceeded ? 1 : exitCode, stdout };
  } catch {
    return { exitCode: 1, stdout: "" };
  }
}

function uniqueTeamIds(values: Iterable<string>): string[] {
  return [...new Set([...values].filter((value) => TEAM_ID_PATTERN.test(value)))];
}

function teamIdsFromSigningIdentities(output: string): string[] {
  return uniqueTeamIds([...output.matchAll(/\(([A-Z0-9]{10})\)/g)].map((match) => match[1]!));
}

function teamIdsFromProvisioningProfile(output: string): string[] {
  const match = output.match(
    /<key>TeamIdentifier<\/key>\s*<array>\s*<string>([A-Z0-9]{10})<\/string>/s,
  );
  return match?.[1] ? [match[1]] : [];
}

export async function detectAppleTeamId(
  captureCommand: CaptureCommand = defaultCaptureCommand,
  homeDirectory = process.env.HOME,
): Promise<TeamCandidate | null> {
  const selectedTeam = await captureCommand([
    "defaults",
    "read",
    "com.apple.dt.Xcode",
    "IDEProvisioningTeamManagerLastSelectedTeamID",
  ]);
  const selectedValue = selectedTeam.stdout.trim().replace(/^"|"$/g, "");
  if (selectedTeam.exitCode === 0 && TEAM_ID_PATTERN.test(selectedValue)) {
    return { teamId: selectedValue, source: "Xcode's selected account" };
  }

  const identities = await captureCommand(["security", "find-identity", "-v", "-p", "codesigning"]);
  const identityTeamIds = identities.exitCode === 0
    ? teamIdsFromSigningIdentities(identities.stdout)
    : [];
  if (identityTeamIds.length === 1) {
    return { teamId: identityTeamIds[0]!, source: "an installed code-signing identity" };
  }

  if (homeDirectory) {
    const profileDirectory = join(
      homeDirectory,
      "Library",
      "Developer",
      "Xcode",
      "UserData",
      "Provisioning Profiles",
    );
    let profileNames: string[] = [];
    try {
      profileNames = readdirSync(profileDirectory)
        .filter((name) => name.endsWith(".mobileprovision"))
        .sort()
        .slice(0, 256);
    } catch {
      profileNames = [];
    }
    const profileTeamIds: string[] = [];
    for (const profileName of profileNames) {
      const decoded = await captureCommand([
        "security",
        "cms",
        "-D",
        "-i",
        join(profileDirectory, profileName),
      ]);
      if (decoded.exitCode === 0) {
        profileTeamIds.push(...teamIdsFromProvisioningProfile(decoded.stdout));
      }
    }
    const uniqueProfileTeamIds = uniqueTeamIds(profileTeamIds);
    if (uniqueProfileTeamIds.length === 1) {
      return { teamId: uniqueProfileTeamIds[0]!, source: "Xcode's provisioning profiles" };
    }
  }

  return null;
}

function inferKeyId(keyPath: string): string {
  return basename(keyPath).match(/^AuthKey_([A-Z0-9]{10})\.p8$/i)?.[1]?.toUpperCase() ?? "";
}

function base64Url(value: string | Uint8Array): string {
  return Buffer.from(value).toString("base64url");
}

function privateKeyDer(pem: string): ArrayBuffer {
  const body = pem
    .replace("-----BEGIN PRIVATE KEY-----", "")
    .replace("-----END PRIVATE KEY-----", "")
    .replace(/\s/g, "");
  if (body === "" || !/^[A-Za-z0-9+/=]+$/.test(body)) {
    throw new Error("The selected .p8 file is not a PKCS#8 private key.");
  }
  const bytes = Uint8Array.from(Buffer.from(body, "base64"));
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

async function appStoreConnectToken(
  config: AppStoreConnectConfig,
  now = Math.floor(Date.now() / 1_000),
): Promise<string> {
  const problem = ascKeyPathProblem(config.keyPath);
  if (problem !== null) throw new Error(problem);
  const details = lstatSync(config.keyPath);
  const descriptor = openSync(
    config.keyPath,
    constants.O_RDONLY | constants.O_NOFOLLOW,
  );
  let keyPem: string;
  try {
    const opened = fstatSync(descriptor);
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      opened.dev !== details.dev ||
      opened.ino !== details.ino ||
      opened.size > MAX_PRIVATE_KEY_BYTES
    ) {
      throw new Error("The .p8 path changed while it was being opened.");
    }
    keyPem = new TextDecoder("utf-8", { fatal: true }).decode(
      readDescriptorBounded(
        descriptor,
        MAX_PRIVATE_KEY_BYTES,
        "App Store Connect private key",
      ),
    );
  } finally {
    closeSync(descriptor);
  }
  const key = await crypto.subtle.importKey(
    "pkcs8",
    privateKeyDer(keyPem),
    { name: "ECDSA", namedCurve: "P-256" },
    false,
    ["sign"],
  );
  const header = base64Url(JSON.stringify({ alg: "ES256", kid: config.keyId, typ: "JWT" }));
  const payload = base64Url(JSON.stringify({
    iss: config.issuerId,
    iat: now,
    exp: now + 19 * 60,
    aud: "appstoreconnect-v1",
  }));
  const signingInput = `${header}.${payload}`;
  const signature = await crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-256" },
    key,
    new TextEncoder().encode(signingInput),
  );
  return `${signingInput}.${base64Url(new Uint8Array(signature))}`;
}

export async function validateAppStoreConnectCredentials(
  config: AppStoreConnectConfig,
  fetcher: Fetcher = fetch,
): Promise<void> {
  let response: Response;
  try {
    const token = await appStoreConnectToken(config);
    response = await fetcher(ASC_API_URL, {
      method: "GET",
      headers: { Authorization: `Bearer ${token}` },
      signal: AbortSignal.timeout(15_000),
    });
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`App Store Connect validation could not run: ${detail}`);
  }
  if (!response.ok) {
    throw new Error(
      `App Store Connect rejected the key (HTTP ${response.status}). ` +
      "Check the Team Key, key ID, issuer ID, and App Manager role.",
    );
  }
}

export function mergeLocalXcconfig(
  existing: string,
  values: Record<(typeof OWNED_XCCONFIG_KEYS)[number], string>,
): string {
  const owned = new Set<string>(OWNED_XCCONFIG_KEYS);
  const sourceLines = existing === ""
    ? ["// Written by `bun run setup`. Gitignored: identifiers and local slots are yours."]
    : existing.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n");
  const written = new Set<string>();
  const merged: string[] = [];

  for (const line of sourceLines) {
    const key = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=/)?.[1];
    if (key && owned.has(key)) {
      if (!written.has(key)) {
        merged.push(`${key} = ${values[key as keyof typeof values]}`);
        written.add(key);
      }
    } else {
      merged.push(line);
    }
  }

  const missing = OWNED_XCCONFIG_KEYS.filter((key) => !written.has(key));
  if (missing.length > 0 && merged.length > 0 && merged.at(-1) !== "") merged.push("");
  for (const key of missing) merged.push(`${key} = ${values[key]}`);
  return `${merged.join("\n")}\n`;
}

async function chooseInteractiveTeam(
  prompter: Prompter,
  candidate: TeamCandidate | null,
): Promise<string> {
  if (candidate) {
    const confirmation = await prompter.askValidated(
      `Why: Xcode needs a signing team for a phone or TestFlight. Detected ${candidate.teamId} from ${candidate.source}; saying no lets you choose another or skip.`,
      `Use Apple Team ID ${candidate.teamId}? (yes/no)`,
      "yes",
      (value) => /^(?:y|yes|n|no)$/i.test(value) ? null : "Answer yes or no.",
    );
    if (/^(?:y|yes)$/i.test(confirmation)) return candidate.teamId;
  }

  return prompter.askValidated(
    "Why: the Team ID signs phone and TestFlight builds; skip = simulator-only until you re-run setup or select a team in Xcode.",
    "Apple Team ID",
    "",
    (value) => teamIdProblem(value, true),
  );
}

async function resolveNonInteractiveTeam(
  selection: string | undefined,
  captureCommand: CaptureCommand,
  homeDirectory: string | undefined,
): Promise<string> {
  if (!selection) {
    throw new Error("--from-manifest also needs --team <id|auto> (or TOHSENO_APPLE_TEAM_ID).");
  }
  if (selection.toLowerCase() === "auto") {
    const detected = await detectAppleTeamId(captureCommand, homeDirectory);
    if (!detected) {
      throw new Error("Could not auto-detect one Apple Team ID; pass --team with the owner-approved ID.");
    }
    return detected.teamId;
  }
  const problem = teamIdProblem(selection, false);
  if (problem) throw new Error(`Invalid --team value: ${problem}`);
  return selection;
}

async function resolveAppStoreConnect(
  options: SetupOptions,
  existing: Partial<SetupConfig>,
  prompter: Prompter | null,
  environment: Environment,
): Promise<AppStoreConnectConfig | undefined> {
  let keyPath = options.ascKeyPath ?? existing.appStoreConnect?.keyPath ?? "";
  if (prompter && options.ascKeyPath === undefined) {
    keyPath = await prompter.askValidated(
      "Path: App Store Connect → Users and Access → Integrations → App Store Connect API → Team Keys → “+” → role App Manager → download once. Why: fastlane needs the Team Key path; setup validates it read-only before writing. Skip = no TestFlight until you re-run setup.",
      "App Store Connect API key path (.p8)",
      keyPath,
      (value) => ascKeyPathProblem(value, environment),
      keyPath ? "keep configured path" : "",
    );
  }
  if (keyPath === "") return undefined;
  const keyPathProblem = ascKeyPathProblem(keyPath, environment);
  if (keyPathProblem) throw new Error(`Invalid App Store Connect key path: ${keyPathProblem}`);
  keyPath = expandPath(keyPath, environment);

  const existingMatchesPath = existing.appStoreConnect?.keyPath === keyPath;
  let keyId = options.ascKeyId ?? (existingMatchesPath ? existing.appStoreConnect?.keyId : undefined) ?? inferKeyId(keyPath);
  if (prompter && options.ascKeyId === undefined) {
    keyId = await prompter.askValidated(
      "Why: the key ID identifies the Team Key used for validation; skip = the key cannot be validated or used for TestFlight, so setup writes nothing.",
      "App Store Connect API key ID",
      keyId,
      (value) => KEY_ID_PATTERN.test(value) ? null : "A key ID is 10 uppercase letters/digits.",
    );
  }
  if (!KEY_ID_PATTERN.test(keyId)) {
    throw new Error(
      "An App Store Connect key needs --asc-key-id (or TOHSENO_ASC_KEY_ID); " +
      "AuthKey_<KEYID>.p8 filenames are inferred.",
    );
  }

  let issuerId = options.ascIssuerId ?? (existingMatchesPath ? existing.appStoreConnect?.issuerId : undefined) ?? "";
  if (prompter && options.ascIssuerId === undefined) {
    issuerId = await prompter.askValidated(
      "Why: the issuer ID scopes the Team Key to your App Store Connect account; skip = validation and TestFlight cannot run, so setup writes nothing.",
      "App Store Connect issuer ID",
      issuerId,
      (value) => ISSUER_ID_PATTERN.test(value)
        ? null
        : "The issuer ID is the UUID shown on the App Store Connect API page.",
    );
  }
  if (!ISSUER_ID_PATTERN.test(issuerId)) {
    throw new Error("An App Store Connect key needs --asc-issuer-id (or TOHSENO_ASC_ISSUER_ID).");
  }
  return { keyPath, keyId, issuerId };
}

export async function main(
  arguments_: string[] = process.argv.slice(2),
  dependencies: SetupDependencies = {},
): Promise<void> {
  const root = dependencies.root ?? ROOT;
  const environment = dependencies.environment ?? process.env;
  const output = dependencies.output ?? defaultOutput;
  const captureCommand = dependencies.captureCommand ?? defaultCaptureCommand;
  const options = parseSetupOptions(arguments_, environment);
  if (options.help) {
    printUsage(output);
    return;
  }

  const manifestDefaults = await readManifestDefaults(root);
  const existing = await readExistingConfig(root);
  const localXcconfigPath = join(root, "Config", "Local.xcconfig");
  const existingLocalXcconfig =
    readOptionalSetupFile(root, localXcconfigPath, "local Xcode config") ?? "";
  const prompter = options.fromManifest
    ? null
    : new Prompter(
      dependencies.stdinLines ?? console[Symbol.asyncIterator](),
      output,
    );

  output.line("\nTOHSENO app setup — owner-approved, machine-local configuration.");
  output.line("Setup writes identifiers and credential paths only; it never copies a private key.\n");

  let displayName: string;
  let bundleId: string;
  let teamId: string;
  if (prompter) {
    displayName = await prompter.askValidated(
      "Why: this is the name people see below the app icon; pressing Enter keeps the workspace or previous-setup default.",
      "App display name",
      existing.displayName ?? manifestDefaults.displayName,
      displayNameProblem,
    );
    bundleId = await prompter.askValidated(
      "Why: Apple uses this stable identifier for signing and installs; pressing Enter keeps the workspace or previous-setup default.",
      "Bundle ID",
      existing.bundleId ?? (displayName === manifestDefaults.displayName
        ? manifestDefaults.bundleId
        : deriveBundleId(displayName)),
      bundleIdProblem,
    );

    let candidate: TeamCandidate | null = null;
    if (options.team?.toLowerCase() === "auto") {
      candidate = await detectAppleTeamId(captureCommand, environment.HOME);
    } else if (options.team) {
      const problem = teamIdProblem(options.team, false);
      if (problem) throw new Error(`Invalid team option: ${problem}`);
      candidate = { teamId: options.team, source: "the setup option or environment" };
    } else if (existing.teamId && TEAM_ID_PATTERN.test(existing.teamId)) {
      candidate = { teamId: existing.teamId, source: "the previous setup" };
    } else {
      candidate = await detectAppleTeamId(captureCommand, environment.HOME);
    }
    teamId = await chooseInteractiveTeam(prompter, candidate);
  } else {
    displayName = manifestDefaults.displayName;
    bundleId = manifestDefaults.bundleId;
    const displayProblem = displayNameProblem(displayName);
    const bundleProblem = bundleIdProblem(bundleId);
    if (displayProblem) throw new Error(`Invalid application.name in the manifest: ${displayProblem}`);
    if (bundleProblem) throw new Error(`Invalid application.id in the manifest: ${bundleProblem}`);
    teamId = await resolveNonInteractiveTeam(options.team, captureCommand, environment.HOME);
  }

  const appStoreConnect = await resolveAppStoreConnect(options, existing, prompter, environment);
  if (appStoreConnect) {
    output.line("Validating the Team Key with a read-only App Store Connect API request before writing config…");
    const validate = dependencies.validateAppStoreConnect ?? validateAppStoreConnectCredentials;
    await validate(appStoreConnect);
    output.line("  ✓ App Store Connect accepted the Team Key.");
  }

  let revenueCatPublicKey = options.revenueCatPublicKey ?? existing.revenueCatPublicKey ?? "";
  if (prompter && options.revenueCatPublicKey === undefined) {
    revenueCatPublicKey = await prompter.askValidated(
      "Why: this public identifier connects the optional paywall module; skip = the paywall stays unavailable until you re-run setup.",
      "RevenueCat public API key",
      revenueCatPublicKey,
      revenueCatKeyProblem,
      revenueCatPublicKey ? "keep configured public key" : "",
    );
  }
  const revenueCatProblem = revenueCatKeyProblem(revenueCatPublicKey);
  if (revenueCatProblem) throw new Error(`Invalid RevenueCat key: ${revenueCatProblem}`);

  const config: SetupConfig = { displayName, bundleId, teamId };
  if (appStoreConnect) config.appStoreConnect = appStoreConnect;
  if (revenueCatPublicKey !== "") config.revenueCatPublicKey = revenueCatPublicKey;

  const configPath = join(root, "app.config.json");
  const mergedXcconfig = mergeLocalXcconfig(existingLocalXcconfig, {
    APP_DISPLAY_NAME: displayName,
    APP_BUNDLE_ID: bundleId,
    DEVELOPMENT_TEAM: teamId,
    REVENUECAT_PUBLIC_KEY: revenueCatPublicKey,
  });
  writePrivateSetupFile(
    root,
    configPath,
    `${JSON.stringify(config, null, 2)}\n`,
    "app config",
  );
  writePrivateSetupFile(
    root,
    localXcconfigPath,
    mergedXcconfig,
    "local Xcode config",
  );

  output.line(`\n  ✓ ${configPath}`);
  output.line(`  ✓ ${localXcconfigPath}`);
  output.line("\nNext:");
  output.line("  open Writing.xcodeproj        # simulator needs no keys");
  if (config.appStoreConnect) {
    output.line("  fastlane beta                 # prepared only; owner runs it when ready");
  } else {
    output.line("  (re-run setup with an App Store Connect Team Key when you want TestFlight)");
  }
  output.line();
}

if (import.meta.main) {
  try {
    await main();
    process.exit(0);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`\nSetup failed: ${message}`);
    process.exit(1);
  }
}
