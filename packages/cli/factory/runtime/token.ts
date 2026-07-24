import {
  accessSync,
  constants as fsConstants,
  existsSync,
  statSync,
} from "node:fs";
import { homedir } from "node:os";
import { delimiter, join, resolve } from "node:path";
import {
  atomicWrite,
  appendStructuredLog,
  ensureRuntimeDirectories,
  MachineError,
  readBoundedUtf8,
  runCaptured,
  runtimePaths,
  safeEnvironment,
} from "./shared.ts";

export interface TokenRecord {
  provider: "bankr";
  chain: "base" | "robinhood";
  name: string;
  symbol: string;
  feeRecipient?: string;
  address?: string;
  txHash?: string;
  launchedAt: string;
}

export interface TokenLaunchOptions {
  name: string;
  symbol: string;
  chain: "base" | "robinhood";
  image?: string;
  website?: string;
  feeRecipient?: string;
  feeType?: "x" | "farcaster" | "ens" | "wallet";
  yes: boolean;
  json: boolean;
}

const INSTALL_HINT = "npm install -g @bankr/cli";
const LOGIN_HINT = "bankr login";
const PROVIDER_TERMS_HINT =
  "Review Bankr's current launch terms and limits at https://docs.bankr.bot/token-launching/overview/";
const MAX_MANIFEST_BYTES = 1_048_576;

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

function bankrCommand(): string[] | null {
  const bankr = executable("bankr");
  return bankr ? [bankr] : null;
}

function bankrAuthenticated(): boolean {
  return existsSync(join(homedir(), ".bankr", "config.json")) ||
    Boolean(process.env.BANKR_API_KEY);
}

function manifestToken(root: string): { raw: string; manifest: Record<string, unknown>; token: TokenRecord | null } {
  const path = join(root, "continuity.manifest.json");
  let raw: string;
  try {
    raw = readBoundedUtf8(path, MAX_MANIFEST_BYTES, "continuity.manifest.json");
  } catch (error) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `cannot read ${path}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  let manifest: Record<string, unknown>;
  try {
    manifest = JSON.parse(raw) as Record<string, unknown>;
  } catch (error) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `continuity.manifest.json is not valid JSON: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const token = manifest.token;
  return {
    raw,
    manifest,
    token: typeof token === "object" && token !== null ? token as TokenRecord : null,
  };
}

export async function tokenStatus(root: string): Promise<{
  bankrCliAvailable: boolean;
  bankrCli: string | null;
  authenticated: boolean;
  token: TokenRecord | null;
}> {
  const command = bankrCommand();
  return {
    bankrCliAvailable: command !== null,
    bankrCli: command === null ? null : command.join(" "),
    authenticated: bankrAuthenticated(),
    token: manifestToken(root).token,
  };
}

export function launchSummary(options: TokenLaunchOptions): string {
  return [
    `Token launch: ${options.name} (${options.symbol}) on ${options.chain}`,
    `Fee recipient: ${options.feeRecipient ?? "your Bankr wallet (default)"}`,
    "IRREVERSIBLE: this asks your installed Bankr CLI to broadcast a permanent on-chain launch under your Bankr account.",
    "Provider economics, vesting, fees, limits, and beneficiary rules can change. TOHSENO does not independently quote or guarantee them.",
    PROVIDER_TERMS_HINT,
  ].join("\n");
}

function requireLaunchApproval(options: TokenLaunchOptions): Promise<void> | void {
  if (options.yes) return;
  const summary = launchSummary(options);
  if (options.json || !process.stdin.isTTY || !process.stderr.isTTY) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `a token launch requires explicit human approval; review the summary and rerun with --yes\n${summary}`,
      { approval: "--yes", summary },
    );
  }
  return confirmOnTty(summary);
}

async function confirmOnTty(summary: string): Promise<void> {
  process.stderr.write(`${summary}\n`);
  const readline = await import("node:readline/promises");
  const prompts = readline.createInterface({ input: process.stdin, output: process.stderr });
  try {
    const answer = (await prompts.question("Type yes to launch: ")).trim().toLowerCase();
    if (answer !== "yes") {
      throw new MachineError("INVALID_CONFIGURATION", "token launch cancelled by the owner");
    }
  } finally {
    prompts.close();
  }
}

function validateLaunchOptions(options: TokenLaunchOptions): void {
  if (
    options.name !== options.name.trim() ||
    [...options.name].length < 1 ||
    [...options.name].length > 80 ||
    /[\u0000-\u001f\u007f]/u.test(options.name)
  ) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "--name must be 1-80 characters with no surrounding whitespace or control characters",
    );
  }
  if (
    options.symbol !== options.symbol.trim() ||
    [...options.symbol].length < 1 ||
    [...options.symbol].length > 10 ||
    /\s|[\u0000-\u001f\u007f]/u.test(options.symbol)
  ) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "--symbol must be 1-10 non-whitespace characters with no control characters",
    );
  }
  for (const [flag, value] of [["--image", options.image], ["--website", options.website]] as const) {
    if (value === undefined) continue;
    if (
      value.length > 2_048 ||
      /[\u0000-\u001f\u007f]/u.test(value)
    ) {
      throw new MachineError("INVALID_CONFIGURATION", `${flag} must be a valid https URL`);
    }
    let url: URL;
    try {
      url = new URL(value);
    } catch {
      throw new MachineError("INVALID_CONFIGURATION", `${flag} must be a valid https URL`);
    }
    if (
      url.protocol !== "https:" ||
      url.username !== "" ||
      url.password !== "" ||
      url.hostname === ""
    ) {
      throw new MachineError("INVALID_CONFIGURATION", `${flag} must use https`);
    }
  }
  if (
    options.feeRecipient !== undefined &&
    (
      options.feeRecipient !== options.feeRecipient.trim() ||
      options.feeRecipient.length < 1 ||
      options.feeRecipient.length > 200 ||
      /\s|[\u0000-\u001f\u007f]/u.test(options.feeRecipient)
    )
  ) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "--fee-recipient must be 1-200 non-whitespace characters with no control characters",
    );
  }
  if (options.feeType !== undefined && options.feeRecipient === undefined) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "--fee-type requires --fee-recipient",
    );
  }
}

function parseLaunchOutput(stdout: string): { address?: string; txHash?: string; parsed: boolean } {
  const validAddress = (value: unknown): string | undefined =>
    typeof value === "string" && /^0x[a-fA-F0-9]{40}$/u.test(value)
      ? value
      : undefined;
  const validTransaction = (value: unknown): string | undefined =>
    typeof value === "string" && /^0x[a-fA-F0-9]{64}$/u.test(value)
      ? value
      : undefined;
  try {
    const value = JSON.parse(stdout) as Record<string, unknown>;
    const address = validAddress(value.address) ??
      validAddress(value.tokenAddress);
    const txHash = validTransaction(value.txHash) ??
      validTransaction(value.transactionHash);
    if (address || txHash) {
      return {
        ...(address === undefined ? {} : { address }),
        ...(txHash === undefined ? {} : { txHash }),
        parsed: true,
      };
    }
  } catch {
    // Not JSON; fall through to the text scan.
  }
  const txHash = stdout.match(/0x[a-fA-F0-9]{64}/u)?.[0];
  const address = stdout.replace(/0x[a-fA-F0-9]{64}/gu, "").match(/0x[a-fA-F0-9]{40}/u)?.[0];
  return {
    ...(address === undefined ? {} : { address }),
    ...(txHash === undefined ? {} : { txHash }),
    parsed: address !== undefined || txHash !== undefined,
  };
}

function bankrEnvironment(): Record<string, string> {
  return {
    ...safeEnvironment(),
    ...(process.env.BANKR_API_KEY
      ? { BANKR_API_KEY: process.env.BANKR_API_KEY }
      : {}),
    BANKR_NOT_INTERACTIVE: "1",
  };
}

function launchArguments(
  command: readonly string[],
  options: TokenLaunchOptions,
  simulate: boolean,
): string[] {
  return [
    ...command,
    "launch",
    "--name", options.name,
    "--symbol", options.symbol,
    "--chain", options.chain,
    ...(options.image === undefined ? [] : ["--image", options.image]),
    ...(options.website === undefined ? [] : ["--website", options.website]),
    ...(options.feeRecipient === undefined ? [] : ["--fee", options.feeRecipient]),
    ...(options.feeType === undefined ? [] : ["--fee-type", options.feeType]),
    ...(simulate ? ["--simulate"] : []),
    "--yes",
  ];
}

async function writeTokenRecord(root: string, raw: string, manifest: Record<string, unknown>, token: TokenRecord): Promise<void> {
  const path = join(root, "continuity.manifest.json");
  atomicWrite(path, `${JSON.stringify({ ...manifest, token }, null, 2)}\n`, 0o644);
  const validation = await runCaptured(
    [process.execPath, join(root, ".tohseno", "manifest", "cli.ts"), path],
    { cwd: root, environment: safeEnvironment() },
  );
  if (validation.exitCode !== 0) {
    atomicWrite(path, raw, 0o644);
    throw new MachineError(
      "INTERNAL_FAILURE",
      `the launch succeeded but the token record failed manifest validation; the manifest was restored\n${validation.stdout}${validation.stderr}`.trim(),
      { token },
    );
  }
}

export async function launchToken(root: string, options: TokenLaunchOptions): Promise<{
  launched: true;
  simulated: true;
  token: TokenRecord;
  parsed: boolean;
  logs: string;
}> {
  const state = manifestToken(root);
  if (state.token !== null) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "a token is already recorded for this shot; one token per shot, and relaunching is a human decision made outside tohseno",
      { token: state.token },
    );
  }
  validateLaunchOptions(options);
  const command = bankrCommand();
  if (command === null) {
    throw new MachineError(
      "MISSING_DEPENDENCY",
      `an explicitly installed Bankr CLI is required; run: ${INSTALL_HINT}`,
      { dependency: "bankr" },
    );
  }
  if (!bankrAuthenticated()) {
    throw new MachineError(
      "MISSING_DEPENDENCY",
      `no Bankr credentials found (~/.bankr/config.json or BANKR_API_KEY); run: ${LOGIN_HINT}`,
      { dependency: "bankr-auth" },
    );
  }
  await requireLaunchApproval(options);

  const paths = runtimePaths(root);
  ensureRuntimeDirectories(paths);
  const environment = bankrEnvironment();
  const simulation = await runCaptured(
    launchArguments(command, options, true),
    { cwd: root, environment },
  );
  if (simulation.exitCode !== 0) {
    appendStructuredLog(paths.tokenLog, {
      event: "bankr_launch_simulation",
      chain: options.chain,
      exitCode: simulation.exitCode,
    });
    throw new MachineError(
      "INTERNAL_FAILURE",
      `Bankr launch simulation failed with exit code ${simulation.exitCode}; provider output was not retained`,
      { hint: PROVIDER_TERMS_HINT, logs: paths.tokenLog },
    );
  }

  const result = await runCaptured(
    launchArguments(command, options, false),
    { cwd: root, environment },
  );
  appendStructuredLog(
    paths.tokenLog,
    {
      event: "bankr_launch",
      chain: options.chain,
      simulated: true,
      exitCode: result.exitCode,
    },
  );
  if (result.exitCode !== 0) {
    throw new MachineError(
      "INTERNAL_FAILURE",
      `Bankr launch failed with exit code ${result.exitCode}; provider output was not retained`,
      { hint: PROVIDER_TERMS_HINT, logs: paths.tokenLog },
    );
  }

  const parsed = parseLaunchOutput(result.stdout);
  const token: TokenRecord = {
    provider: "bankr",
    chain: options.chain,
    name: options.name,
    symbol: options.symbol,
    ...(options.feeRecipient === undefined ? {} : { feeRecipient: options.feeRecipient }),
    ...(parsed.address === undefined ? {} : { address: parsed.address }),
    ...(parsed.txHash === undefined ? {} : { txHash: parsed.txHash }),
    launchedAt: new Date().toISOString(),
  };
  await writeTokenRecord(root, state.raw, state.manifest, token);
  return {
    launched: true,
    simulated: true,
    token,
    parsed: parsed.parsed,
    logs: paths.tokenLog,
  };
}

export async function tokenFees(root: string): Promise<{
  address: string;
  fees: unknown;
}> {
  const token = manifestToken(root).token;
  if (token === null || typeof token.address !== "string") {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "no launched token with a recorded address; run tohseno machine token launch first",
    );
  }
  const command = bankrCommand();
  if (command === null) {
    throw new MachineError(
      "MISSING_DEPENDENCY",
      `an explicitly installed Bankr CLI is required; run: ${INSTALL_HINT}`,
      { dependency: "bankr" },
    );
  }
  const result = await runCaptured(
    [...command, "fees", token.address, "--json"],
    { cwd: root, environment: bankrEnvironment() },
  );
  if (result.exitCode !== 0) {
    throw new MachineError(
      "INTERNAL_FAILURE",
      `Bankr fees lookup failed with exit code ${result.exitCode}; provider output was not retained`,
    );
  }
  let fees: unknown;
  try {
    fees = JSON.parse(result.stdout);
  } catch {
    throw new MachineError(
      "INTERNAL_FAILURE",
      "Bankr fees returned an invalid JSON response; provider output was not retained",
    );
  }
  return { address: token.address, fees };
}
