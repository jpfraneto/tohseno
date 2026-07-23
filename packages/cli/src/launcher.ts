import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  chooseNumber,
  continueCommand,
  createCommand,
  discoverShots,
  type CommandContext,
  type DiscoveredShot,
} from "./commands.ts";
import { CliError } from "./errors.ts";
import { bunExecutable, runCaptured, sanitizedRuntimeEnvironment } from "./process.ts";
import { slugForShotName } from "./slug.ts";

async function shotSummary(shot: DiscoveredShot, context: CommandContext): Promise<string> {
  const git = await runCaptured(["git", "status", "--porcelain"], {
    cwd: shot.path,
    env: context.environment,
  });
  const worktree = git.exitCode === 0 ? (git.stdout === "" ? "clean" : "changes") : "Git unavailable";
  let runtime = "development unavailable in legacy shot";
  const machine = join(shot.path, ".tohseno", "machine.ts");
  if (existsSync(machine)) {
    try {
      const inspected = await runCaptured([
        bunExecutable(context.environment), machine, "dev", "status", "--json",
      ], {
        cwd: shot.path,
        env: sanitizedRuntimeEnvironment(context.environment),
      });
      const envelope = JSON.parse(inspected.stdout) as {
        result?: { state?: unknown };
        error?: { details?: { status?: { state?: unknown } } };
      };
      const state = envelope.result?.state ?? envelope.error?.details?.status?.state;
      runtime = state === "running"
        ? "development running"
        : state === "starting"
          ? "development starting"
          : state === "unhealthy"
            ? "development unhealthy"
            : "development stopped";
    } catch {
      const statePath = join(shot.path, ".tohseno", "run", "state.json");
      runtime = existsSync(statePath) ? "development state unreadable" : "development stopped";
    }
  }
  return `${shot.name} — iOS · ${worktree} · ${runtime}`;
}

export async function interactiveLauncher(context: CommandContext): Promise<number> {
  if (!context.io.interactive) {
    throw new CliError(
      "the no-argument experience needs an interactive terminal; automation should use explicit create or machine commands",
      2,
    );
  }
  const shots = discoverShots(context);
  context.io.out("What would you like to do?");
  context.io.out();
  if (shots.length > 0) {
    context.io.out(`  Shots here: ${shots.length}`);
    context.io.out();
  }
  context.io.out(`  1. ${shots.length === 0 ? "Take your first shot" : "Take another shot"}`);
  context.io.out("  2. Continue a shot");
  const action = await chooseNumber(context.io, 2, "Choose");
  context.io.out();

  if (action === 0) {
    const name = (await context.io.prompt("Shot name: ")).trim();
    const slug = slugForShotName(name);
    if (slug !== name) context.io.out(`Using filesystem name: ${slug}`);
    return await createCommand({
      slug,
      noLaunch: false,
      noInteractive: false,
    }, context);
  }

  if (shots.length === 0) {
    throw new CliError(`no shots exist in ${context.config.shotsDirectory}; choose Take your first shot`, 2);
  }
  context.io.out("Shots:");
  const summaries = await Promise.all(shots.map((shot) => shotSummary(shot, context)));
  summaries.forEach((summary, index) => context.io.out(`  ${index + 1}. ${summary}`));
  const selected = shots[await chooseNumber(context.io, shots.length, "Continue") ]!;
  context.io.out();
  return await continueCommand(selected.path, { noInteractive: false }, context);
}
