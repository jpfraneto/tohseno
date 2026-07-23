import { randomUUID } from "node:crypto";
import {
  createShot,
  type CreateShotRequest,
  type CreateShotResult,
} from "../creation.ts";
import { CliError } from "../errors.ts";
import {
  SHOT_PROGRESS_SCHEMA_VERSION,
  type ShotProgressEvent,
} from "../progress.ts";
import {
  type StudioOperationGate,
  type StudioOperationLease,
} from "./operations.ts";
import { StudioHttpError } from "./security.ts";
import type { StagedStudioInput } from "./uploads.ts";

const MAX_RETAINED_JOBS = 20;

export type StudioFactoryDefaults = Omit<
  CreateShotRequest,
  | "door"
  | "input"
  | "io"
  | "jobId"
  | "name"
  | "noLaunch"
  | "onProgress"
  | "signal"
  | "slug"
>;

export type StudioShotFactory = (
  request: CreateShotRequest,
) => Promise<CreateShotResult>;

export interface StudioJobManagerOptions {
  creation: StudioFactoryDefaults;
  operations: StudioOperationGate;
  factory?: StudioShotFactory;
  now?: () => Date;
  onActivity?: () => void;
}

export interface StudioJobEvent {
  id: number;
  event: ShotProgressEvent;
}

interface StudioJob {
  id: string;
  controller: AbortController;
  events: StudioJobEvent[];
  subscribers: Set<(event: StudioJobEvent) => void>;
  terminal: boolean;
  cleanup: () => void;
  promise: Promise<void>;
}

function terminalType(type: ShotProgressEvent["type"]): boolean {
  return type === "completed" || type === "failed" || type === "interrupted";
}

function publicFailureMessage(error: unknown): string {
  if (error instanceof CliError) return error.message;
  if (
    error instanceof Error &&
    error.name === "SimulatorError" &&
    error.message.length <= 500
  ) {
    return error.message;
  }
  return "Shot creation stopped before completion.";
}

export class StudioJobManager {
  readonly #creation: StudioFactoryDefaults;
  readonly #factory: StudioShotFactory;
  readonly #operations: StudioOperationGate;
  readonly #now: () => Date;
  readonly #onActivity: (() => void) | undefined;
  readonly #jobs = new Map<string, StudioJob>();
  #activeJobId: string | null = null;
  #closed = false;

  constructor(options: StudioJobManagerOptions) {
    this.#creation = options.creation;
    this.#factory = options.factory ?? createShot;
    this.#operations = options.operations;
    this.#now = options.now ?? (() => new Date());
    this.#onActivity = options.onActivity;
  }

  get activeJobId(): string | null {
    return this.#activeJobId;
  }

  start(staged: StagedStudioInput): { jobId: string } {
    if (this.#closed) {
      staged.cleanup();
      throw new StudioHttpError(
        503,
        "studio-stopping",
        "Studio is shutting down.",
      );
    }
    let operation: StudioOperationLease;
    try {
      operation = this.#operations.acquire("create");
    } catch (error) {
      staged.cleanup();
      throw error;
    }
    const jobId = randomUUID();
    const controller = new AbortController();
    const job: StudioJob = {
      id: jobId,
      controller,
      events: [],
      subscribers: new Set(),
      terminal: false,
      cleanup: staged.cleanup,
      promise: Promise.resolve(),
    };
    this.#jobs.set(jobId, job);
    this.#activeJobId = jobId;
    this.#pruneJobs();
    job.promise = Promise.resolve().then(async () => {
      try {
        const request: CreateShotRequest = {
          ...this.#creation,
          door: "studio",
          input: staged.input,
          ...(staged.name === undefined ? {} : { name: staged.name }),
          noLaunch: false,
          jobId,
          signal: controller.signal,
          onProgress: (event) => {
            this.#publish(job, event);
          },
        };
        const result = await this.#factory(request);
        if (!job.terminal) {
          this.#publish(job, {
            schemaVersion: SHOT_PROGRESS_SCHEMA_VERSION,
            jobId,
            at: this.#now().toISOString(),
            type: "completed",
            door: "studio",
            slug: result.metadata.slug,
            sequence: result.sequence,
          });
        }
      } catch (error) {
        if (!job.terminal) {
          this.#publish(job, {
            schemaVersion: SHOT_PROGRESS_SCHEMA_VERSION,
            jobId,
            at: this.#now().toISOString(),
            type: controller.signal.aborted ? "interrupted" : "failed",
            door: "studio",
            message: controller.signal.aborted
              ? "Creation stopped safely."
              : publicFailureMessage(error),
          });
        }
      } finally {
        try {
          job.cleanup();
        } finally {
          operation.release();
          if (this.#activeJobId === jobId) this.#activeJobId = null;
          this.#onActivity?.();
        }
      }
    });
    return { jobId };
  }

  has(jobId: string): boolean {
    return this.#jobs.has(jobId);
  }

  events(jobId: string, afterId = 0): StudioJobEvent[] {
    const job = this.#jobs.get(jobId);
    if (!job) {
      throw new StudioHttpError(
        404,
        "job-not-found",
        "That local creation job is no longer available.",
      );
    }
    return job.events.filter((record) => record.id > afterId);
  }

  isTerminal(jobId: string): boolean {
    const job = this.#jobs.get(jobId);
    if (!job) {
      throw new StudioHttpError(
        404,
        "job-not-found",
        "That local creation job is no longer available.",
      );
    }
    return job.terminal;
  }

  subscribe(
    jobId: string,
    afterId: number,
    subscriber: (event: StudioJobEvent) => void,
  ): () => void {
    const job = this.#jobs.get(jobId);
    if (!job) {
      throw new StudioHttpError(
        404,
        "job-not-found",
        "That local creation job is no longer available.",
      );
    }
    for (const record of job.events) {
      if (record.id > afterId) subscriber(record);
    }
    if (!job.terminal) job.subscribers.add(subscriber);
    return () => {
      job.subscribers.delete(subscriber);
    };
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    const running = [...this.#jobs.values()]
      .filter((job) => !job.terminal)
      .map((job) => {
        job.controller.abort();
        return job.promise;
      });
    await Promise.allSettled(running);
    for (const job of this.#jobs.values()) {
      job.cleanup();
      job.subscribers.clear();
    }
    this.#activeJobId = null;
  }

  #publish(job: StudioJob, event: ShotProgressEvent): void {
    if (job.terminal) return;
    const record = { id: job.events.length + 1, event };
    job.events.push(record);
    if (terminalType(event.type)) job.terminal = true;
    for (const subscriber of job.subscribers) {
      try {
        subscriber(record);
      } catch {
        // A disconnected Studio browser must not affect factory progress.
      }
    }
    if (job.terminal) job.subscribers.clear();
    this.#onActivity?.();
  }

  #pruneJobs(): void {
    if (this.#jobs.size <= MAX_RETAINED_JOBS) return;
    for (const [id, job] of this.#jobs) {
      if (this.#jobs.size <= MAX_RETAINED_JOBS) break;
      if (!job.terminal || id === this.#activeJobId) continue;
      this.#jobs.delete(id);
    }
  }
}
