import type { CliIo } from "./io.ts";

export type HandoffState =
  | "completed"
  | "action-needed"
  | "blocked"
  | "not-attempted"
  | "informational";

export interface HandoffEvidence {
  id:
    | "repository"
    | "source"
    | "skills"
    | "build"
    | "simulator"
    | "capture"
    | "agent";
  label: string;
  state: HandoffState;
  detail: string;
}

export interface ShotHandoff {
  schemaVersion: 1;
  shot: {
    name: string;
    slug: string;
    sequence?: number;
    path: string;
  };
  evidence: HandoffEvidence[];
  next: {
    action: string;
    command?: string;
    why: string;
  };
  later: Array<{
    label: string;
    command: string;
  }>;
}

const SYMBOLS: Record<HandoffState, string> = {
  completed: "✅",
  "action-needed": "🟡",
  blocked: "🔴",
  "not-attempted": "⚪",
  informational: "🔵",
};

const COLORS: Record<HandoffState, string> = {
  completed: "\u001b[32m",
  "action-needed": "\u001b[33m",
  blocked: "\u001b[31m",
  "not-attempted": "\u001b[37m",
  informational: "\u001b[34m",
};

function colorEnabled(
  io: CliIo,
  environment: Record<string, string | undefined>,
): boolean {
  return (
    io.interactive &&
    environment.NO_COLOR === undefined &&
    environment.TERM !== "dumb"
  );
}

function statusLine(
  evidence: HandoffEvidence,
  colored: boolean,
): string {
  const content = `${SYMBOLS[evidence.state]} ${evidence.label.padEnd(11)} ${evidence.detail}`;
  return colored
    ? `${COLORS[evidence.state]}${content}\u001b[0m`
    : content;
}

export function renderHandoff(
  handoff: ShotHandoff,
  io: CliIo,
  environment: Record<string, string | undefined> = process.env,
): void {
  const colored = colorEnabled(io, environment);
  io.out();
  io.out(
    `TOHSENO / ${handoff.shot.name.toUpperCase()}${
      handoff.shot.sequence === undefined
        ? ""
        : ` / SHOT ${String(handoff.shot.sequence).padStart(3, "0")}`
    }`,
  );
  io.out();
  for (const evidence of handoff.evidence) {
    io.out(statusLine(evidence, colored));
  }
  io.out();
  io.out("APP");
  io.out(handoff.shot.path);
  io.out();
  io.out("NEXT");
  io.out(handoff.next.action);
  if (handoff.next.command !== undefined) {
    io.out();
    io.out(`    ${handoff.next.command}`);
  }
  io.out();
  io.out("WHY");
  io.out(handoff.next.why);
  if (handoff.later.length > 0) {
    io.out();
    io.out("LATER");
    for (const item of handoff.later) {
      io.out(`${`${item.label}:`.padEnd(20)} ${item.command}`);
    }
  }
}

export function handoffForShot(options: {
  name: string;
  slug: string;
  path: string;
  sequence?: number;
  skillCount: number;
  verificationPassed: boolean;
  agentExitCode: number | null;
  buildState: "completed" | "failed" | "not-attempted";
  simulatorState: "completed" | "failed" | "not-attempted";
  captureState: "completed" | "failed" | "not-attempted";
  simulatorReason?: string;
}): ShotHandoff {
  const quote = (value: string): string =>
    `'${value.replaceAll("'", `'\"'\"'`)}'`;
  const shotTarget = options.verificationPassed
    ? options.slug
    : quote(options.path);
  const evidence: HandoffEvidence[] = [
    {
      id: "repository",
      label: "REPOSITORY",
      state: options.verificationPassed ? "completed" : "blocked",
      detail: options.verificationPassed
        ? "privacy rules and pinned factory rails verified"
        : "privacy or pinned integrity verification failed",
    },
    {
      id: "source",
      label: "SOURCE",
      state: options.verificationPassed ? "completed" : "blocked",
      detail: options.verificationPassed
        ? "manifest and exact composition lock verified"
        : "not marked ready",
    },
    {
      id: "skills",
      label: "SKILLS",
      state: options.verificationPassed ? "completed" : "blocked",
      detail: options.skillCount === 0
        ? "neutral kernel; no app skills installed"
        : `${options.skillCount}/${options.skillCount} acceptance sets passed`,
    },
    {
      id: "build",
      label: "BUILD",
      state: options.buildState === "completed"
        ? "completed"
        : options.buildState === "failed"
          ? "blocked"
          : "not-attempted",
      detail: options.buildState === "completed"
        ? "native iOS build succeeded"
        : options.buildState === "failed"
          ? "native iOS build failed"
          : "not attempted in this flow",
    },
    {
      id: "simulator",
      label: "SIMULATOR",
      state: options.simulatorState === "completed"
        ? "completed"
        : options.simulatorState === "failed"
          ? "action-needed"
          : "not-attempted",
      detail: options.simulatorState === "completed"
        ? "launched on an iPhone Simulator"
        : options.simulatorState === "failed"
          ? `not launched — ${options.simulatorReason ?? "the Apple toolchain is unavailable"}`
          : "not requested in this flow",
    },
    {
      id: "capture",
      label: "CAPTURE",
      state: options.captureState === "completed"
        ? "completed"
        : options.captureState === "failed"
          ? "action-needed"
          : "not-attempted",
      detail: options.captureState === "completed"
        ? "Simulator screenshot saved"
        : options.captureState === "failed"
          ? "screenshot unavailable after launch"
          : "not attempted",
    },
  ];
  if (options.agentExitCode !== null) {
    evidence.unshift({
      id: "agent",
      label: "AGENT",
      state: options.agentExitCode === 0 ? "completed" : "blocked",
      detail: options.agentExitCode === 0
        ? "coding agent exited successfully"
        : `coding agent exited with status ${options.agentExitCode}`,
    });
  }

  const blocked = !options.verificationPassed || (
    options.agentExitCode !== null && options.agentExitCode !== 0
  );
  const launched = options.simulatorState === "completed";
  return {
    schemaVersion: 1,
    shot: {
      name: options.name,
      slug: options.slug,
      path: options.path,
      ...(options.sequence === undefined ? {} : { sequence: options.sequence }),
    },
    evidence,
    next: blocked
      ? {
          action: "Continue the shot from any folder.",
          command: options.verificationPassed
            ? `tohseno ${options.slug}`
            : `tohseno continue ${shotTarget}`,
          why: !options.verificationPassed
            ? "The repository must pass its pinned privacy and integrity gate before TOHSENO presents it as ready."
            : "The coding agent stopped before the accepted shot contract was completed.",
        }
      : launched
        ? {
            action: "Use the app in Simulator and decide what the next shot needs.",
            command: `tohseno ${options.slug}`,
            why: "A shot becomes useful through contact with the real app, not through terminal output alone.",
          }
        : {
            action: "Build and launch the app from any folder.",
            command: `tohseno run ${options.slug}`,
            why: options.simulatorReason ??
              "The repository is verified; the native Apple build and Simulator still need direct evidence.",
          },
    later: [
      {
        label: "Continue building",
        command: options.verificationPassed
          ? `tohseno ${options.slug}`
          : `tohseno continue ${shotTarget}`,
      },
      { label: "Verify again", command: `tohseno verify ${shotTarget}` },
      { label: "Open Studio", command: "tohseno studio" },
      { label: "Take another shot", command: "tohseno" },
    ],
  };
}
