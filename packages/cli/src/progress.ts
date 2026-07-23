import {
  appendFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
} from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import { CliError } from "./errors.ts";

export const SHOT_PROGRESS_SCHEMA_VERSION = 1 as const;
export const MAX_PROGRESS_JOURNAL_BYTES = 2 * 1_048_576;

export type CreationDoor = "cli" | "studio";

export type ShotProgressType =
  | "allocated"
  | "preparing-release"
  | "preparing-shot"
  | "provenance-written"
  | "manifest-validated"
  | "baseline-committed"
  | "published"
  | "agent-started"
  | "agent-completed"
  | "verifying"
  | "building"
  | "simulator-launching"
  | "screenshot-captured"
  | "preview-unavailable"
  | "completed"
  | "interrupted"
  | "failed";

export interface ShotProgressEvent {
  schemaVersion: typeof SHOT_PROGRESS_SCHEMA_VERSION;
  jobId: string;
  at: string;
  type: ShotProgressType;
  door: CreationDoor;
  slug?: string;
  sequence?: number;
  message?: string;
}

export type ShotProgressInput = Omit<
  ShotProgressEvent,
  "schemaVersion" | "jobId" | "at" | "door"
>;

export type ShotProgressSink = (
  event: ShotProgressEvent,
) => void | Promise<void>;

const SAFE_JOB_ID = /^[A-Za-z0-9][A-Za-z0-9-]{7,79}$/u;

function requireSafeJobId(jobId: string): string {
  if (!SAFE_JOB_ID.test(jobId)) {
    throw new CliError("creation job id has an unsafe format");
  }
  return jobId;
}

export function progressJournalDirectory(shotsDirectory: string): string {
  return join(resolve(shotsDirectory), ".tohseno", "events");
}

export function progressJournalPath(
  shotsDirectory: string,
  jobId: string,
): string {
  return join(progressJournalDirectory(shotsDirectory), `${requireSafeJobId(jobId)}.jsonl`);
}

function inside(root: string, candidate: string): boolean {
  const fromRoot = relative(root, candidate);
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function privateDirectory(
  path: string,
  boundary: string,
  label: string,
): string {
  const details = lstatSync(path);
  if (details.isSymbolicLink() || !details.isDirectory()) {
    throw new CliError(`${label} is not a private directory: ${path}`);
  }
  const canonical = realpathSync(path);
  if (canonical === boundary || !inside(boundary, canonical)) {
    throw new CliError(`${label} leaves the shots workspace: ${path}`);
  }
  return canonical;
}

function createPrivateDirectory(path: string): void {
  try {
    mkdirSync(path, { mode: 0o700 });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
  }
}

export function existingProgressJournalDirectory(
  shotsDirectory: string,
): string | null {
  const requested = resolve(shotsDirectory);
  if (!existsSync(requested)) return null;
  try {
    const root = realpathSync(requested);
    const rootDetails = lstatSync(root);
    if (!rootDetails.isDirectory()) return null;
    const controlPath = join(root, ".tohseno");
    if (!existsSync(controlPath)) return null;
    const control = privateDirectory(
      controlPath,
      root,
      "workspace control path",
    );
    const eventsPath = join(control, "events");
    if (!existsSync(eventsPath)) return null;
    return privateDirectory(
      eventsPath,
      control,
      "progress journal path",
    );
  } catch {
    return null;
  }
}

function ensureJournalDirectory(shotsDirectory: string): string {
  const requested = resolve(shotsDirectory);
  mkdirSync(requested, { recursive: true, mode: 0o700 });
  const root = realpathSync(requested);
  const rootDetails = lstatSync(root);
  if (!rootDetails.isDirectory()) {
    throw new CliError(`shots workspace is not a directory: ${requested}`);
  }
  const controlPath = join(root, ".tohseno");
  createPrivateDirectory(controlPath);
  const control = privateDirectory(
    controlPath,
    root,
    "workspace control path",
  );
  const directoryPath = join(control, "events");
  createPrivateDirectory(directoryPath);
  return privateDirectory(
    directoryPath,
    control,
    "progress journal path",
  );
}

export class ShotProgressReporter {
  readonly jobId: string;
  readonly door: CreationDoor;
  readonly journalPath: string;
  readonly #now: () => Date;
  readonly #sinks: readonly ShotProgressSink[];

  constructor(options: {
    shotsDirectory: string;
    jobId: string;
    door: CreationDoor;
    now?: () => Date;
    sinks?: readonly ShotProgressSink[];
  }) {
    this.jobId = requireSafeJobId(options.jobId);
    this.door = options.door;
    this.#now = options.now ?? (() => new Date());
    this.#sinks = options.sinks ?? [];
    ensureJournalDirectory(options.shotsDirectory);
    this.journalPath = progressJournalPath(options.shotsDirectory, this.jobId);
  }

  async emit(input: ShotProgressInput): Promise<ShotProgressEvent> {
    const event: ShotProgressEvent = {
      schemaVersion: SHOT_PROGRESS_SCHEMA_VERSION,
      jobId: this.jobId,
      at: this.#now().toISOString(),
      type: input.type,
      door: this.door,
      ...(input.slug === undefined ? {} : { slug: input.slug }),
      ...(input.sequence === undefined ? {} : { sequence: input.sequence }),
      ...(input.message === undefined ? {} : { message: input.message }),
    };
    appendFileSync(this.journalPath, `${JSON.stringify(event)}\n`, {
      encoding: "utf8",
      mode: 0o600,
    });
    await Promise.all(this.#sinks.map(async (sink) => {
      try {
        await sink(event);
      } catch {
        // The workspace journal is authoritative. A disconnected browser or
        // another presentation-layer failure must not corrupt factory work.
      }
    }));
    return event;
  }
}

function isProgressEvent(value: unknown): value is ShotProgressEvent {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }
  const event = value as Partial<ShotProgressEvent>;
  return event.schemaVersion === SHOT_PROGRESS_SCHEMA_VERSION &&
    typeof event.jobId === "string" &&
    SAFE_JOB_ID.test(event.jobId) &&
    typeof event.at === "string" &&
    typeof event.type === "string" &&
    (event.door === "cli" || event.door === "studio");
}

export function readProgressJournal(path: string): ShotProgressEvent[] {
  if (!existsSync(path)) return [];
  const details = lstatSync(path);
  if (details.isSymbolicLink() || !details.isFile()) return [];
  const events: ShotProgressEvent[] = [];
  for (const line of readFileSync(path, "utf8").split("\n")) {
    if (line.trim() === "") continue;
    try {
      const value = JSON.parse(line) as unknown;
      if (isProgressEvent(value)) events.push(value);
    } catch {
      // A process may have stopped between append bytes. Earlier complete
      // events remain useful; an incomplete tail is ignored.
    }
  }
  return events;
}
