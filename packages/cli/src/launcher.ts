import {
  chooseNumber,
  evolveCommand,
  createCommand,
  discoverShots,
  type CommandContext,
  type DiscoveredShot,
} from "./commands.ts";
import { CliError } from "./errors.ts";
import { ONBOARDING_VERSION } from "./constants.ts";
import { writeOnboardingVersion } from "./config.ts";
import { readLocalShotProtocolState } from "./protocol-state.ts";

async function shotSummary(shot: DiscoveredShot): Promise<string> {
  const state = readLocalShotProtocolState(shot.path);
  if (state === null) {
    throw new CliError(
      "pre-release compatibility is unsupported; create a fresh Shot with `tohseno`",
      2,
    );
  }
  return `${shot.name} — iOS · ${state.lifecycle} · Evolution ${state.evolution}`;
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
  context.io.out("  2. Evolve a shot");
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
  const summaries = await Promise.all(shots.map((shot) => shotSummary(shot)));
  summaries.forEach((summary, index) => context.io.out(`  ${index + 1}. ${summary}`));
  const selected = shots[await chooseNumber(context.io, shots.length, "Evolve") ]!;
  context.io.out();
  return await evolveCommand(selected.path, { noInteractive: false }, context);
}
