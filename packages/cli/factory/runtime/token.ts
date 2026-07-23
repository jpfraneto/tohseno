import { accessSync, appendFileSync, constants as fsConstants, existsSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join, resolve } from "node:path";
import {
  atomicWrite,
  ensureRuntimeDirectories,
  MachineError,
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

const LOGIN_HINT = "npx @bankr/cli login email";
const RATE_LIMIT_HINT =
  "Bankr limits: one launch per minute per wallet, 50 per day; gas is sponsored for the first 3 launches per day";

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
  if (bankr) return [bankr];
  const npx = executable("npx");
  if (npx) return [npx, "@bankr/cli"];
  return null;
}

function bankrAuthenticated(): boolean {
  return existsSync(join(homedir(), ".bankr", "config.json")) ||
    Boolean(process.env.BANKR_API_KEY);
}

function manifestToken(root: string): { raw: string; manifest: Record<string, unknown>; token: TokenRecord | null } {
  const path = join(root, "continuity.manifest.json");
  let raw: string;
  try {
    raw = readFileSync(path, "utf8");
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
    "Fixed economics: 100B supply; 85% liquidity pool, 15% creator vesting over 1 year with a 30-day cliff; 0.7% swap fee split 95% creator / 5% protocol.",
    "IRREVERSIBLE: the launch is a permanent on-chain action under your own Bankr account. The vesting recipient is locked forever; the fee beneficiary can only ever be transferred all-or-nothing, permanently.",
    RATE_LIMIT_HINT + ".",
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
  if (options.symbol.length < 1 || options.symbol.length > 10 || !/\S/u.test(options.symbol)) {
    throw new MachineError("INVALID_CONFIGURATION", "--symbol must be 1-10 characters");
  }
  for (const [flag, value] of [["--image", options.image], ["--website", options.website]] as const) {
    if (value === undefined) continue;
    let url: URL;
    try {
      url = new URL(value);
    } catch {
      throw new MachineError("INVALID_CONFIGURATION", `${flag} must be a valid https URL`);
    }
    if (url.protocol !== "https:") {
      throw new MachineError("INVALID_CONFIGURATION", `${flag} must use https`);
    }
  }
}

function scrub(value: string): string {
  const key = process.env.BANKR_API_KEY;
  return key ? value.split(key).join("[redacted]") : value;
}

function appendTokenLog(log: string, record: Record<string, unknown>, output: string): void {
  appendFileSync(
    log,
    [JSON.stringify({ at: new Date().toISOString(), ...record }), scrub(output)].filter(Boolean).join("\n") + "\n",
    { mode: 0o600 },
  );
}

function parseLaunchOutput(stdout: string): { address?: string; txHash?: string; parsed: boolean } {
  try {
    const value = JSON.parse(stdout) as Record<string, unknown>;
    const address = typeof value.address === "string" ? value.address : typeof value.tokenAddress === "string" ? value.tokenAddress : undefined;
    const txHash = typeof value.txHash === "string" ? value.txHash : typeof value.transactionHash === "string" ? value.transactionHash : undefined;
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
      `the Bankr CLI is required; install Node and run: ${LOGIN_HINT}`,
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
  const environment = {
    ...safeEnvironment(),
    ...(process.env.BANKR_API_KEY ? { BANKR_API_KEY: process.env.BANKR_API_KEY } : {}),
    BANKR_NOT_INTERACTIVE: "1",
  };
  const result = await runCaptured([
    ...command,
    "launch",
    "--name", options.name,
    "--symbol", options.symbol,
    "--chain", options.chain,
    ...(options.image === undefined ? [] : ["--image", options.image]),
    ...(options.website === undefined ? [] : ["--website", options.website]),
    ...(options.feeRecipient === undefined ? [] : ["--fee", options.feeRecipient]),
    ...(options.feeType === undefined ? [] : ["--fee-type", options.feeType]),
    "--yes",
  ], { cwd: root, environment });
  appendTokenLog(
    paths.tokenLog,
    { event: "bankr_launch", chain: options.chain, symbol: options.symbol, exitCode: result.exitCode },
    [result.stdout, result.stderr].filter(Boolean).join("\n"),
  );
  if (result.exitCode !== 0) {
    throw new MachineError(
      "INTERNAL_FAILURE",
      scrub(`bankr launch failed: ${(result.stderr.trim() || result.stdout.trim() || `exit ${result.exitCode}`)}`),
      { hint: RATE_LIMIT_HINT, logs: paths.tokenLog },
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
  return { launched: true, token, parsed: parsed.parsed, logs: paths.tokenLog };
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
      `the Bankr CLI is required; install Node and run: ${LOGIN_HINT}`,
      { dependency: "bankr" },
    );
  }
  const result = await runCaptured(
    [...command, "fees", token.address, "--json"],
    { cwd: root, environment: safeEnvironment() },
  );
  if (result.exitCode !== 0) {
    throw new MachineError(
      "INTERNAL_FAILURE",
      scrub(`bankr fees failed: ${(result.stderr.trim() || result.stdout.trim() || `exit ${result.exitCode}`)}`),
    );
  }
  let fees: unknown;
  try {
    fees = JSON.parse(result.stdout);
  } catch {
    fees = result.stdout.trim();
  }
  return { address: token.address, fees };
}
