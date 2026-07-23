import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import type { CliIo } from "../src/io.ts";
import { removeTreeEvenIfReadOnly } from "../src/files.ts";

export const REPOSITORY_ROOT = resolve(import.meta.dir, "../../..");
export const IOS_TEMPLATE_ROOT = join(REPOSITORY_ROOT, "templates", "continuity-app");

const REAL_GIT = Bun.which("git") ?? (() => {
  throw new Error("CLI tests require Git");
})();

export interface ScratchEnvironment {
  root: string;
  home: string;
  factoryHome: string;
  shotsDirectory: string;
  binDirectory: string;
  environment: Record<string, string | undefined>;
}

export interface MemoryIo extends CliIo {
  stdout: string[];
  stderr: string[];
  questions: string[];
}

export interface ProcessResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export function createScratchEnvironment(): ScratchEnvironment {
  const root = mkdtempSync(join(tmpdir(), "tohseno cli test with spaces-"));
  const home = join(root, "isolated home");
  const factoryHome = join(root, "factory home");
  const shotsDirectory = join(root, "shots with spaces");
  const binDirectory = join(root, "fake bin");
  mkdirSync(home, { recursive: true });
  mkdirSync(binDirectory, { recursive: true });
  writeExecutable(
    binDirectory,
    "bun",
    `#!/bin/sh\nexec ${JSON.stringify(process.execPath)} \"$@\"`,
  );

  const environment: Record<string, string | undefined> = { ...process.env };
  for (const key of Object.keys(environment)) {
    if (
      key.startsWith("GIT_AUTHOR_") ||
      key.startsWith("GIT_COMMITTER_") ||
      key.startsWith("GIT_CONFIG_")
    ) {
      delete environment[key];
    }
  }
  environment.HOME = home;
  environment.TOHSENO_HOME = factoryHome;
  environment.TOHSENO_SHOTS_DIR = shotsDirectory;
  environment.TOHSENO_SOURCE_ROOT = REPOSITORY_ROOT;
  environment.GIT_CONFIG_NOSYSTEM = "1";
  environment.GIT_CONFIG_GLOBAL = "/dev/null";
  environment.GIT_TERMINAL_PROMPT = "0";
  environment.PATH = [binDirectory, dirname(REAL_GIT), "/usr/bin", "/bin"].join(":");

  return { root, home, factoryHome, shotsDirectory, binDirectory, environment };
}

export async function withScratchEnvironment(
  action: (scratch: ScratchEnvironment) => void | Promise<void>,
): Promise<void> {
  const scratch = createScratchEnvironment();
  try {
    await action(scratch);
  } finally {
    if (existsSync(scratch.root)) removeTreeEvenIfReadOnly(scratch.root);
  }
}

export function createMemoryIo(
  interactive = false,
  answers: readonly string[] = [],
): MemoryIo {
  const remaining = [...answers];
  const stdout: string[] = [];
  const stderr: string[] = [];
  const questions: string[] = [];
  return {
    interactive,
    stdout,
    stderr,
    questions,
    out(line = "") {
      stdout.push(line);
    },
    error(line = "") {
      stderr.push(line);
    },
    async prompt(question: string): Promise<string> {
      questions.push(question);
      const answer = remaining.shift();
      if (answer === undefined) throw new Error(`unexpected prompt: ${question}`);
      return answer;
    },
  };
}

export function writeExecutable(directory: string, name: string, source: string): string {
  mkdirSync(directory, { recursive: true });
  const path = join(directory, name);
  writeFileSync(path, source.endsWith("\n") ? source : `${source}\n`, { mode: 0o755 });
  chmodSync(path, 0o755);
  return path;
}

export function installFakeAgent(scratch: ScratchEnvironment, name: "codex" | "claude"): string {
  return writeExecutable(scratch.binDirectory, name, [
    "#!/bin/sh",
    "record=\"$HOME/.tohseno-test-agent-record\"",
    "printf '%s\\n%s\\n%s\\n%s\\n%s\\n%s\\n' \"$0\" \"$PWD\" \"$#\" \"$1\" \"$2\" \"${OPENAI_API_KEY:-}${ANTHROPIC_API_KEY:-}${DEV_SECRET:-}\" > \"$record\"",
    "exit_file=\"$HOME/.tohseno-test-agent-exit\"",
    "if [ -f \"$exit_file\" ]; then exit \"$(sed -n '1p' \"$exit_file\")\"; fi",
    "exit 0",
  ].join("\n"));
}

export function fakeAgentRecordPath(scratch: ScratchEnvironment): string {
  return join(scratch.home, ".tohseno-test-agent-record");
}

export function setFakeAgentExit(scratch: ScratchEnvironment, exitCode: number): void {
  writeFileSync(join(scratch.home, ".tohseno-test-agent-exit"), `${exitCode}\n`);
}

export function installCommitFailingGit(scratch: ScratchEnvironment, exitCode = 42): string {
  return writeExecutable(scratch.binDirectory, "git", [
    "#!/bin/sh",
    "for argument in \"$@\"; do",
    "  if [ \"$argument\" = \"commit\" ]; then",
    `    exit ${exitCode}`,
    "  fi",
    "done",
    `exec ${JSON.stringify(REAL_GIT)} \"$@\"`,
  ].join("\n"));
}

export async function runProcess(
  command: readonly string[],
  cwd: string,
  environment: Record<string, string | undefined>,
): Promise<ProcessResult> {
  const child = Bun.spawn([...command], {
    cwd,
    env: environment,
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  });
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ]);
  return { exitCode, stdout, stderr };
}

export async function runGit(
  arguments_: readonly string[],
  cwd: string,
  environment: Record<string, string | undefined>,
): Promise<ProcessResult> {
  return runProcess([REAL_GIT, ...arguments_], cwd, environment);
}

export async function initializeCompatibleProject(
  scratch: ScratchEnvironment,
  directoryName = "legacy-app",
): Promise<string> {
  const root = join(scratch.root, directoryName);
  cpSync(IOS_TEMPLATE_ROOT, root, { recursive: true });
  const init = await runGit(
    ["-c", "init.templateDir=", "init", "--quiet", "--initial-branch=main"],
    root,
    scratch.environment,
  );
  if (init.exitCode !== 0) throw new Error(init.stderr);
  const add = await runGit(["add", "-A"], root, scratch.environment);
  if (add.exitCode !== 0) throw new Error(add.stderr);
  const commit = await runGit([
    "-c", "commit.gpgSign=false",
    "-c", "user.name=CLI Test",
    "-c", "user.email=cli-test@tohseno.local",
    "commit", "--quiet", "--no-verify", "-m", "test fixture",
  ], root, scratch.environment);
  if (commit.exitCode !== 0) throw new Error(commit.stderr);
  return root;
}

export function listTree(root: string, relative = ""): string[] {
  const directory = join(root, relative);
  const paths: string[] = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = relative === "" ? entry.name : `${relative}/${entry.name}`;
    paths.push(path);
    if (entry.isDirectory()) paths.push(...listTree(root, path));
  }
  return paths.sort();
}

export function textFilesOutsideGit(root: string): Array<{ path: string; source: string }> {
  return listTree(root)
    .filter((path) => path !== ".git" && !path.startsWith(".git/"))
    .filter((path) => statSync(join(root, path)).isFile())
    .map((path) => ({ path, source: readFileSync(join(root, path), "utf8") }));
}
