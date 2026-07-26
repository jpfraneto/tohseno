import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { AgentAdapter } from "./agents.ts";
import { sanitizedAgentEnvironment } from "./agents.ts";
import { CliError } from "./errors.ts";
import type { AppCatalog, ResolvedComposition } from "../../skills/types.ts";
import { resolveComposition } from "../../skills/index.ts";
import { bundleIdForSlug, displayNameForSlug, slugForShotName } from "./slug.ts";

export const SHOT_PLAN_SCHEMA_VERSION = 1 as const;
export const DEFAULT_PLANNER_TIMEOUT_MS = 45_000;

export interface ShotPlan {
  schemaVersion: typeof SHOT_PLAN_SCHEMA_VERSION;
  app: {
    name: string;
    slug: string;
    bundleId: string;
  };
  summary: string;
  template: string;
  skills: Array<{
    id: string;
    reason: string;
  }>;
  data: {
    strategy: "local" | "remote" | "hybrid";
    reason: string;
  };
  identity: {
    strategy: "none" | "local-device" | "wallet" | "account";
    reason: string;
  };
  definitionOfDone: string[];
  assumptions: string[];
  questions: string[];
}

export interface ValidatedShotPlan {
  plan: ShotPlan;
  composition: ResolvedComposition;
  fallback: boolean;
  fallbackReason?: "offline" | "timeout" | "invalid-output" | "no-agent";
}

export interface PlannerInvocation {
  agent: AgentAdapter;
  cwd: string;
  environment: Record<string, string | undefined>;
  timeoutMs: number;
}

export type PlannerInvoker = (invocation: PlannerInvocation) => Promise<string>;

const PLAN_INSTRUCTION = [
  "You are the private planning phase of an iOS app factory.",
  "Read intention.md and catalog.json in the current directory.",
  "Return exactly one JSON object matching the schema described in catalog.json.",
  "Select only listed template and skill IDs. List only extra skills not already supplied by the selected template.",
  "Prefer local data and no identity for a first shot unless the intention requires otherwise.",
  "Ask questions only for architectural or externally consequential decisions that cannot be deferred.",
  "Do not write files, use the network, or include the raw intention verbatim.",
].join(" ");

function planningArguments(agent: AgentAdapter): string[] {
  if (agent.id === "codex") {
    return [
      agent.executable,
      "--sandbox",
      "read-only",
      "--ask-for-approval",
      "never",
      "exec",
      "--color",
      "never",
      PLAN_INSTRUCTION,
    ];
  }
  return [
    agent.executable,
    "--print",
    "--permission-mode",
    "plan",
    "--no-session-persistence",
    PLAN_INSTRUCTION,
  ];
}

export async function invokePlanner(
  invocation: PlannerInvocation,
): Promise<string> {
  const child = Bun.spawn(planningArguments(invocation.agent), {
    cwd: invocation.cwd,
    env: sanitizedAgentEnvironment(invocation.environment),
    stdin: "ignore",
    stdout: "pipe",
    stderr: "ignore",
  });
  let timedOut = false;
  const timeout = setTimeout(() => {
    timedOut = true;
    try {
      child.kill("SIGTERM");
    } catch {
      // The process already exited.
    }
  }, invocation.timeoutMs);
  timeout.unref?.();
  try {
    const [exitCode, stdout] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
    ]);
    if (timedOut) throw new CliError("planning timed out");
    if (exitCode !== 0) throw new CliError(`planning agent exited with status ${exitCode}`);
    return stdout;
  } finally {
    clearTimeout(timeout);
  }
}

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function exactKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
  label: string,
): void {
  const allowedSet = new Set(allowed);
  const unknown = Object.keys(value).filter((key) => !allowedSet.has(key));
  if (unknown.length > 0) {
    throw new CliError(`${label} contains unsupported field ${JSON.stringify(unknown[0])}`);
  }
}

function text(value: unknown, label: string, maximum = 240): string {
  if (
    typeof value !== "string" ||
    value.trim() === "" ||
    value.length > maximum ||
    /[\u0000-\u001f\u007f-\u009f]/u.test(value)
  ) {
    throw new CliError(`${label} must be a short non-empty string`);
  }
  return value.trim();
}

function strings(
  value: unknown,
  label: string,
  maximumItems: number,
  maximumLength = 240,
): string[] {
  if (!Array.isArray(value) || value.length > maximumItems) {
    throw new CliError(`${label} must be an array with at most ${maximumItems} items`);
  }
  return value.map((item, index) => text(item, `${label}[${index}]`, maximumLength));
}

function privateCopy(
  value: string,
  rawIntention: string,
): boolean {
  const normalizedValue = value.trim().replace(/\s+/gu, " ").toLowerCase();
  const normalizedRaw = rawIntention.trim().replace(/\s+/gu, " ").toLowerCase();
  return normalizedValue.length >= 24 && (
    normalizedRaw === normalizedValue ||
    normalizedRaw.includes(normalizedValue) ||
    normalizedValue.includes(normalizedRaw)
  );
}

export function validateShotPlan(
  value: unknown,
  catalog: AppCatalog,
  rawIntention: string,
): { plan: ShotPlan; composition: ResolvedComposition } {
  const plan = record(value);
  if (plan === null) throw new CliError("planner output must be an object");
  exactKeys(plan, [
    "schemaVersion",
    "app",
    "summary",
    "template",
    "skills",
    "data",
    "identity",
    "definitionOfDone",
    "assumptions",
    "questions",
  ], "planner output");
  if (plan.schemaVersion !== 1) throw new CliError("planner schemaVersion must be 1");
  const app = record(plan.app);
  const data = record(plan.data);
  const identity = record(plan.identity);
  if (app === null || data === null || identity === null) {
    throw new CliError("planner app, data, and identity fields must be objects");
  }
  exactKeys(app, ["name", "slug", "bundleId"], "planner app");
  exactKeys(data, ["strategy", "reason"], "planner data");
  exactKeys(identity, ["strategy", "reason"], "planner identity");

  const name = text(app.name, "planner app.name", 80);
  const slug = slugForShotName(text(app.slug, "planner app.slug", 80));
  const bundleId = text(app.bundleId, "planner app.bundleId", 180);
  if (
    !/^[A-Za-z0-9]+(?:\.[A-Za-z0-9-]+)+$/u.test(bundleId) ||
    bundleId.includes("\n")
  ) {
    throw new CliError("planner app.bundleId must be a reverse-domain identifier");
  }
  const summary = text(plan.summary, "planner summary", 320);
  const skillValues = plan.skills;
  if (!Array.isArray(skillValues) || skillValues.length > 24) {
    throw new CliError("planner skills must be an array");
  }
  const skills = skillValues.map((item, index) => {
    const skill = record(item);
    if (skill === null) throw new CliError(`planner skills[${index}] must be an object`);
    exactKeys(skill, ["id", "reason"], `planner skills[${index}]`);
    return {
      id: text(skill.id, `planner skills[${index}].id`, 80),
      reason: text(skill.reason, `planner skills[${index}].reason`, 240),
    };
  });
  const dataStrategy = data.strategy;
  if (!["local", "remote", "hybrid"].includes(String(dataStrategy))) {
    throw new CliError("planner data.strategy is unsupported");
  }
  const identityStrategy = identity.strategy;
  if (!["none", "local-device", "wallet", "account"].includes(String(identityStrategy))) {
    throw new CliError("planner identity.strategy is unsupported");
  }
  const definitionOfDone = strings(plan.definitionOfDone, "definitionOfDone", 8);
  if (definitionOfDone.length === 0) {
    throw new CliError("planner definitionOfDone must not be empty");
  }
  const assumptions = strings(plan.assumptions, "assumptions", 8);
  const questions = strings(plan.questions, "questions", 3);
  const trackedText = [
    summary,
    ...skills.map((skill) => skill.reason),
    text(data.reason, "planner data.reason"),
    text(identity.reason, "planner identity.reason"),
    ...definitionOfDone,
    ...assumptions,
    ...questions,
  ];
  if (trackedText.some((item) => privateCopy(item, rawIntention))) {
    throw new CliError("planner output copied private intention text instead of sanitizing it");
  }
  const composition = resolveComposition(catalog, {
    schemaVersion: 1,
    template: text(plan.template, "planner template", 80),
    skills: skills.map((skill) => skill.id),
  });
  return {
    plan: {
      schemaVersion: 1,
      app: { name, slug, bundleId },
      summary,
      template: composition.template.descriptor.id,
      skills: composition.skills.map((installed) => {
        const selected = skills.find((skill) => skill.id === installed.descriptor.id);
        return {
          id: installed.descriptor.id,
          reason: selected?.reason ??
            `Required by the selected ${composition.template.descriptor.title} composition.`,
        };
      }),
      data: {
        strategy: dataStrategy as ShotPlan["data"]["strategy"],
        reason: text(data.reason, "planner data.reason"),
      },
      identity: {
        strategy: identityStrategy as ShotPlan["identity"]["strategy"],
        reason: text(identity.reason, "planner identity.reason"),
      },
      definitionOfDone,
      assumptions,
      questions,
    },
    composition,
  };
}

function fallbackPlan(
  catalog: AppCatalog,
  nameValue?: string,
): { plan: ShotPlan; composition: ResolvedComposition } {
  const name = nameValue?.trim() || "New App";
  const slug = slugForShotName(name);
  const composition = resolveComposition(catalog, {
    schemaVersion: 1,
    template: "blank",
    skills: [],
  });
  return {
    plan: {
      schemaVersion: 1,
      app: {
        name: displayNameForSlug(slug),
        slug,
        bundleId: bundleIdForSlug(slug),
      },
      summary: "A native iOS app shaped from the owner’s private intention.",
      template: "blank",
      skills: [],
      data: {
        strategy: "local",
        reason: "No remote data requirement is installed in the safe fallback.",
      },
      identity: {
        strategy: "none",
        reason: "The safe fallback does not assume an account or identity system.",
      },
      definitionOfDone: [...composition.template.descriptor.definitionOfDone],
      assumptions: [
        "The coding agent will resolve product-specific behavior from private provenance and SHOT.md.",
      ],
      questions: [],
    },
    composition,
  };
}

function catalogPrompt(catalog: AppCatalog): string {
  return JSON.stringify({
    outputSchema: {
      schemaVersion: 1,
      app: { name: "string", slug: "string", bundleId: "string" },
      summary: "sanitized functional interpretation",
      template: "known template id",
      skills: [{
        id: "extra known skill id not supplied by the selected template",
        reason: "sanitized reason",
      }],
      data: { strategy: "local|remote|hybrid", reason: "string" },
      identity: {
        strategy: "none|local-device|wallet|account",
        reason: "string",
      },
      definitionOfDone: ["observable behavior"],
      assumptions: ["safe assumption"],
      questions: ["only architectural or external consequence questions"],
    },
    platform: "ios",
    templates: [...catalog.templates.values()].map((template) => ({
      id: template.descriptor.id,
      title: template.descriptor.title,
      summary: template.descriptor.summary,
      defaultSkills: template.descriptor.skills,
      definitionOfDone: template.descriptor.definitionOfDone,
    })),
    skills: [...catalog.skills.values()].map((skill) => ({
      id: skill.descriptor.id,
      title: skill.descriptor.title,
      summary: skill.descriptor.summary,
      requires: skill.descriptor.requires,
      conflicts: skill.descriptor.conflicts,
    })),
  }, null, 2);
}

function extractJson(source: string): unknown {
  const trimmed = source.trim();
  const fenced = trimmed.match(/^```(?:json)?\s*([\s\S]*?)\s*```$/u);
  const candidate = fenced?.[1] ?? trimmed;
  return JSON.parse(candidate) as unknown;
}

export async function planIntention(options: {
  intention: string;
  catalog: AppCatalog;
  agent: AgentAdapter | null;
  environment: Record<string, string | undefined>;
  invoker?: PlannerInvoker;
  timeoutMs?: number;
}): Promise<ValidatedShotPlan> {
  if (options.agent === null) {
    const fallback = fallbackPlan(options.catalog);
    return { ...fallback, fallback: true, fallbackReason: "no-agent" };
  }
  const scratch = mkdtempSync(join(tmpdir(), "tohseno-plan-"));
  try {
    writeFileSync(join(scratch, "intention.md"), options.intention, { mode: 0o600 });
    writeFileSync(join(scratch, "catalog.json"), catalogPrompt(options.catalog), { mode: 0o600 });
    let output: string;
    try {
      output = await (options.invoker ?? invokePlanner)({
        agent: options.agent,
        cwd: scratch,
        environment: options.environment,
        timeoutMs: options.timeoutMs ?? DEFAULT_PLANNER_TIMEOUT_MS,
      });
    } catch (error) {
      const fallback = fallbackPlan(options.catalog);
      const timeout = error instanceof Error && /timed out/iu.test(error.message);
      return {
        ...fallback,
        fallback: true,
        fallbackReason: timeout ? "timeout" : "offline",
      };
    }
    try {
      const validated = validateShotPlan(
        extractJson(output),
        options.catalog,
        options.intention,
      );
      return { ...validated, fallback: false };
    } catch {
      const fallback = fallbackPlan(options.catalog);
      return { ...fallback, fallback: true, fallbackReason: "invalid-output" };
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

export function blankPlan(
  catalog: AppCatalog,
  name?: string,
): ValidatedShotPlan {
  return {
    ...fallbackPlan(catalog, name),
    fallback: true,
    fallbackReason: "no-agent",
  };
}
