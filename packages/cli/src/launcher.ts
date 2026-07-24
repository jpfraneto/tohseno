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
import { ONBOARDING_VERSION } from "./constants.ts";
import { writeOnboardingVersion } from "./config.ts";
import { bunExecutable, runCaptured, sanitizedRuntimeEnvironment } from "./process.ts";
import { trustedShotToolFromCache } from "./trusted-tools.ts";

async function shotSummary(shot: DiscoveredShot, context: CommandContext): Promise<string> {
  let runtime = "development unavailable in legacy shot";
  const machine = join(shot.path, ".tohseno", "machine.ts");
  if (existsSync(machine)) {
    try {
      const trusted = trustedShotToolFromCache({
        shotRoot: shot.path,
        releasesDirectory: context.config.cacheDirectory,
        tool: "machine",
      });
      const inspected = await runCaptured([
        bunExecutable(context.environment),
        trusted.executable,
        "dev",
        "status",
        "--json",
      ], {
        cwd: trusted.root,
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
  return `${shot.name} — iOS · repository ready · ${runtime}`;
}

export async function interactiveLauncher(context: CommandContext): Promise<number> {
  if (!context.io.interactive) {
    throw new CliError(
      "the no-argument experience needs an interactive terminal; automation should use explicit create or machine commands",
      2,
    );
  }
  if ((context.config.onboardingVersion ?? 0) < ONBOARDING_VERSION) {
    context.io.out(
      "TOHSENO creates an independent native iOS repository in your shots folder,",
    );
    context.io.out(
      "keeps your raw intention and references private from Git, asks your selected",
    );
    context.io.out(
      "coding agent to build inside that repository, then verifies the result and",
    );
    context.io.out("helps you launch it in Apple Simulator.");
    context.io.out();
    context.io.out(
      "The coding agent uses its provider’s account, privacy, and retention terms.",
    );
    context.io.out();
    await context.io.prompt("Press Enter to continue: ");
    writeOnboardingVersion(context.config, ONBOARDING_VERSION);
  }
  const shots = discoverShots(context);
  if (shots.length === 0) {
    context.io.out("Your contact sheet is empty.");
    context.io.out();
    context.io.out("Let’s take your first shot.");
    context.io.out();
    let intention = "";
    while (intention === "") {
      intention = (
        await context.io.prompt(
          "Intention (one line is enough; use `tohseno create --file ...` for a document): ",
        )
      ).trim();
      if (intention === "") context.io.error("Describe what you want to make.");
    }
    context.io.out();
    return await createCommand({
      text: intention,
      noLaunch: false,
      noInteractive: false,
    }, context);
  }
  context.io.out("What would you like to do?");
  context.io.out();
  context.io.out(`  Shots here: ${shots.length}`);
  context.io.out();
  context.io.out("  1. Take another shot");
  context.io.out("  2. Continue a shot");
  const action = await chooseNumber(context.io, 2, "Choose");
  context.io.out();

  if (action === 0) {
    let intention = "";
    while (intention === "") {
      intention = (
        await context.io.prompt(
          "Intention (one line is enough; use `tohseno create --file ...` for a document): ",
        )
      ).trim();
      if (intention === "") context.io.error("Describe what you want to make.");
    }
    return await createCommand({
      text: intention,
      noLaunch: false,
      noInteractive: false,
    }, context);
  }
  context.io.out("Shots:");
  const summaries = await Promise.all(shots.map((shot) => shotSummary(shot, context)));
  summaries.forEach((summary, index) => context.io.out(`  ${index + 1}. ${summary}`));
  const selected = shots[await chooseNumber(context.io, shots.length, "Continue") ]!;
  context.io.out();
  return await continueCommand(selected.path, { noInteractive: false }, context);
}
