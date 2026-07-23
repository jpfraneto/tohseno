import {
  existsSync,
  type FSWatcher,
  lstatSync,
  readdirSync,
  statSync,
  watch,
} from "node:fs";
import { join } from "node:path";
import {
  existingProgressJournalDirectory,
  MAX_PROGRESS_JOURNAL_BYTES,
  readProgressJournal,
  type ShotProgressEvent,
} from "../progress.ts";
import { discoverShotsNewestFirst } from "../workspace.ts";

const DEFAULT_POLL_INTERVAL_MS = 500;

interface ShotSnapshot {
  signature: string;
  screenshot: string | null;
}

export interface WorkspaceStudioEvent {
  id: number;
  type:
    | "ready"
    | "shots-changed"
    | "shot-created"
    | "shot-changed"
    | "shot-removed"
    | ShotProgressEvent["type"];
  at: string;
  slug?: string;
  progress?: ShotProgressEvent;
}

export interface WorkspaceObserverOptions {
  shotsDirectory: string;
  pollIntervalMs?: number;
  now?: () => Date;
}

function safeStatSignature(path: string): string {
  try {
    const details = statSync(path);
    return `${details.mtimeMs}:${details.size}`;
  } catch {
    return "missing";
  }
}

function shotSnapshot(
  path: string,
  createdAt: string,
  sequence: number | undefined,
): ShotSnapshot {
  const screenshotPath = join(path, ".tohseno", "artifacts", "screenshot.png");
  const screenshot = existsSync(screenshotPath)
    ? safeStatSignature(screenshotPath)
    : null;
  return {
    signature: [
      createdAt,
      sequence ?? 0,
      safeStatSignature(join(path, ".tohseno", "shot.json")),
      safeStatSignature(
        join(path, ".tohseno", "provenance", "events.jsonl"),
      ),
      safeStatSignature(join(path, "continuity.manifest.json")),
    ].join(":"),
    screenshot,
  };
}

export class WorkspaceObserver {
  readonly #shotsDirectory: string;
  readonly #pollIntervalMs: number;
  readonly #now: () => Date;
  readonly #subscribers = new Set<(event: WorkspaceStudioEvent) => void>();
  readonly #journalOffsets = new Map<string, number>();
  #shots: Map<string, ShotSnapshot> | null = null;
  #watcher: FSWatcher | null = null;
  #timer: ReturnType<typeof setInterval> | null = null;
  #eventId = 0;
  #scanning = false;
  #scanAgain = false;
  #closed = false;

  constructor(options: WorkspaceObserverOptions) {
    this.#shotsDirectory = options.shotsDirectory;
    this.#pollIntervalMs =
      options.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS;
    this.#now = options.now ?? (() => new Date());
  }

  start(): void {
    if (this.#closed || this.#timer !== null) return;
    try {
      this.#watcher = watch(
        this.#shotsDirectory,
        { persistent: false },
        () => {
          this.requestScan();
        },
      );
      this.#watcher.on("error", () => {
        this.#watcher?.close();
        this.#watcher = null;
      });
    } catch {
      // Polling below is the portable fallback and remains authoritative.
    }
    this.#timer = setInterval(() => {
      this.requestScan();
    }, this.#pollIntervalMs);
    this.#timer.unref?.();
    this.requestScan();
  }

  subscribe(
    subscriber: (event: WorkspaceStudioEvent) => void,
  ): () => void {
    this.#subscribers.add(subscriber);
    subscriber({
      id: ++this.#eventId,
      type: "ready",
      at: this.#now().toISOString(),
    });
    return () => {
      this.#subscribers.delete(subscriber);
    };
  }

  requestScan(): void {
    if (this.#closed) return;
    if (this.#scanning) {
      this.#scanAgain = true;
      return;
    }
    this.#scanning = true;
    try {
      do {
        this.#scanAgain = false;
        this.#scan();
      } while (this.#scanAgain && !this.#closed);
    } finally {
      this.#scanning = false;
    }
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#watcher?.close();
    this.#watcher = null;
    if (this.#timer !== null) clearInterval(this.#timer);
    this.#timer = null;
    this.#subscribers.clear();
  }

  #scan(): void {
    const next = new Map<string, ShotSnapshot>();
    for (const shot of discoverShotsNewestFirst(this.#shotsDirectory)) {
      next.set(
        shot.metadata.slug,
        shotSnapshot(
          shot.path,
          shot.metadata.createdAt,
          shot.metadata.sequence,
        ),
      );
    }
    const previous = this.#shots;
    this.#shots = next;
    if (previous !== null) {
      let changed = false;
      for (const [slug, current] of next) {
        const earlier = previous.get(slug);
        if (earlier === undefined) {
          changed = true;
          this.#emit({ type: "shot-created", slug });
          continue;
        }
        if (earlier.signature !== current.signature) {
          changed = true;
          this.#emit({ type: "shot-changed", slug });
        }
        if (earlier.screenshot !== current.screenshot) {
          changed = true;
          this.#emit({
            type: current.screenshot === null
              ? "shot-changed"
              : "screenshot-captured",
            slug,
          });
        }
      }
      for (const slug of previous.keys()) {
        if (!next.has(slug)) {
          changed = true;
          this.#emit({ type: "shot-removed", slug });
        }
      }
      if (changed) this.#emit({ type: "shots-changed" });
    }
    this.#scanJournals(previous === null);
  }

  #scanJournals(initial: boolean): void {
    const directory = existingProgressJournalDirectory(this.#shotsDirectory);
    if (directory === null) return;
    let entries;
    try {
      entries = readdirSync(directory, { withFileTypes: true });
    } catch {
      return;
    }
    const present = new Set<string>();
    for (const entry of entries) {
      if (
        !entry.isFile() ||
        !/^[A-Za-z0-9][A-Za-z0-9-]{7,79}\.jsonl$/u.test(entry.name)
      ) {
        continue;
      }
      const path = join(directory, entry.name);
      try {
        const details = lstatSync(path);
        if (
          details.isSymbolicLink() ||
          !details.isFile() ||
          details.size > MAX_PROGRESS_JOURNAL_BYTES
        ) {
          continue;
        }
      } catch {
        continue;
      }
      present.add(entry.name);
      let events: ShotProgressEvent[];
      try {
        events = readProgressJournal(path);
      } catch {
        continue;
      }
      const earlier = this.#journalOffsets.get(entry.name);
      const offset = earlier === undefined
        ? (initial ? events.length : 0)
        : Math.min(earlier, events.length);
      this.#journalOffsets.set(entry.name, events.length);
      for (const progress of events.slice(offset)) {
        this.#emit({
          type: progress.type,
          ...(progress.slug === undefined ? {} : { slug: progress.slug }),
          progress,
        });
      }
    }
    for (const name of this.#journalOffsets.keys()) {
      if (!present.has(name)) this.#journalOffsets.delete(name);
    }
  }

  #emit(
    input: Omit<WorkspaceStudioEvent, "id" | "at">,
  ): void {
    const event: WorkspaceStudioEvent = {
      id: ++this.#eventId,
      at: this.#now().toISOString(),
      ...input,
    };
    for (const subscriber of this.#subscribers) {
      try {
        subscriber(event);
      } catch {
        // A browser disconnect must never affect workspace observation.
      }
    }
  }
}
