import type { ShotProgressEvent } from "../progress.ts";
import { StudioHttpError } from "./security.ts";

export type StudioHeavyOperation = "create" | "preview" | "run" | "verify";

export type StudioShotOperationalStatus =
  | "CREATING"
  | "INTERRUPTED"
  | "READY";

export interface StudioOperationLease {
  readonly operation: StudioHeavyOperation;
  release(): void;
}

const OPERATION_ACTIVITY: Record<StudioHeavyOperation, string> = {
  create: "creating a shot",
  preview: "opening a live preview",
  run: "running a shot",
  verify: "verifying a shot",
};

export function shotOperationalStatus(
  events: readonly ShotProgressEvent[],
): StudioShotOperationalStatus {
  const latest = events.at(-1);
  if (latest === undefined || latest.type === "completed") return "READY";
  if (latest.type === "failed" || latest.type === "interrupted") {
    return "INTERRUPTED";
  }
  return "CREATING";
}

export class StudioOperationGate {
  #active: {
    operation: StudioHeavyOperation;
    token: symbol;
  } | null = null;
  #running: {
    controller: AbortController;
    promise: Promise<unknown>;
  } | null = null;
  #closed = false;

  get activeOperation(): StudioHeavyOperation | null {
    return this.#active?.operation ?? null;
  }

  assertAvailable(): void {
    if (this.#closed) {
      throw new StudioHttpError(
        503,
        "studio-stopping",
        "Studio is shutting down.",
      );
    }
    if (this.#active === null) return;
    throw new StudioHttpError(
      409,
      "operation-busy",
      `Studio is already ${OPERATION_ACTIVITY[this.#active.operation]}. Wait for it to finish.`,
    );
  }

  acquire(operation: StudioHeavyOperation): StudioOperationLease {
    this.assertAvailable();
    const token = Symbol(operation);
    this.#active = { operation, token };
    let released = false;
    return {
      operation,
      release: () => {
        if (released) return;
        released = true;
        if (this.#active?.token === token) this.#active = null;
      },
    };
  }

  async run<T>(
    operation: StudioHeavyOperation,
    callback: (signal: AbortSignal) => T | Promise<T>,
  ): Promise<T> {
    const lease = this.acquire(operation);
    const controller = new AbortController();
    const promise = Promise.resolve().then(
      async () => await callback(controller.signal),
    );
    this.#running = { controller, promise };
    try {
      return await promise;
    } finally {
      if (this.#running?.promise === promise) this.#running = null;
      lease.release();
    }
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    const running = this.#running;
    running?.controller.abort();
    if (running !== null) await Promise.allSettled([running.promise]);
  }
}
