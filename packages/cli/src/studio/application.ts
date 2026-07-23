import {
  existsSync,
  lstatSync,
  readFileSync,
  realpathSync,
  statSync,
} from "node:fs";
import { fileURLToPath } from "node:url";
import { basename, join, relative, resolve, sep } from "node:path";
import { CliError } from "../errors.ts";
import {
  detectImageType,
  MAX_INTENTION_BYTES,
  MAX_REFERENCE_BYTES,
  MAX_REFERENCES,
} from "../provenance.ts";
import {
  MAX_PROGRESS_JOURNAL_BYTES,
  readProgressJournal,
  type ShotProgressEvent,
} from "../progress.ts";
import {
  discoverShotsNewestFirst,
  recognizedShotBySlug,
  type DiscoveredShot,
} from "../workspace.ts";
import {
  StudioJobManager,
  type StudioFactoryDefaults,
  type StudioShotFactory,
} from "./jobs.ts";
import {
  WorkspaceObserver,
  type WorkspaceStudioEvent,
} from "./observer.ts";
import {
  shotOperationalStatus,
  StudioOperationGate,
  type StudioHeavyOperation,
} from "./operations.ts";
import {
  StudioHttpError,
  StudioRequestSecurity,
  type StudioSecurityOptions,
  withStudioSecurityHeaders,
} from "./security.ts";
import { stageStudioCreationRequest } from "./uploads.ts";

const MAX_PROVENANCE_BYTES = 2 * 1_048_576;
const MAX_SCREENSHOT_BYTES = 32 * 1_048_576;
const HEARTBEAT_INTERVAL_MS = 15_000;
const LIVE_PREVIEW_URL =
  /^http:\/\/127\.0\.0\.1:([1-9][0-9]{0,4})\/_tohseno\/live\/([A-Za-z0-9_-]{43,128})$/u;

type StudioActionName =
  | "run"
  | "preview"
  | "stop-preview"
  | "verify"
  | "open-xcode"
  | "reveal";

const ACTION_NAMES = new Set<StudioActionName>([
  "run",
  "preview",
  "stop-preview",
  "verify",
  "open-xcode",
  "reveal",
]);

const NAMED_WORKSPACE_EVENTS = new Set([
  "shots-changed",
  "shot-created",
  "shot-changed",
  "shot-removed",
  "screenshot-captured",
  "completed",
]);

export interface StudioActionResult {
  message?: string;
  /**
   * Only preview may return a URL. The application accepts serve-sim's exact
   * loopback capability URL and never persists or logs it.
   */
  url?: string;
}

export interface StudioActionContext {
  signal: AbortSignal;
}

export type StudioShotAction = (
  shot: DiscoveredShot,
  context: StudioActionContext,
) => StudioActionResult | void | Promise<StudioActionResult | void>;

export interface StudioActions {
  run?: StudioShotAction;
  preview?: StudioShotAction;
  "stop-preview"?: StudioShotAction;
  verify?: StudioShotAction;
  "open-xcode"?: StudioShotAction;
  reveal?: StudioShotAction;
}

export interface StudioRequestLog {
  method: string;
  route: string;
  status: number;
  durationMs: number;
  code?: string;
}

export interface StudioApplicationOptions {
  creation: StudioFactoryDefaults;
  security?: StudioSecurityOptions;
  factory?: StudioShotFactory;
  actions?: StudioActions;
  observer?: WorkspaceObserver;
  assetsDirectory?: string;
  now?: () => Date;
  logger?: (record: StudioRequestLog) => void;
  dispose?: () => void | Promise<void>;
}

interface SafeReference {
  path: string;
  originalFilename: string;
  mediaType: string;
}

interface PrivateShotInput {
  intention: string | null;
  references: SafeReference[];
}

interface StaticAssets {
  html: string;
  css: Uint8Array;
  javascript: Uint8Array;
}

interface RoutedResponse {
  route: string;
  response: Response;
}

interface ErrorResponse {
  status: number;
  code: string;
  message: string;
}

function isRecord(
  value: unknown,
): value is Record<string, unknown> {
  return typeof value === "object" &&
    value !== null &&
    !Array.isArray(value);
}

function inside(root: string, candidate: string): boolean {
  const fromRoot = relative(resolve(root), resolve(candidate));
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function safeRegularFile(root: string, candidate: string): string | null {
  if (!existsSync(candidate)) return null;
  try {
    const details = lstatSync(candidate);
    if (details.isSymbolicLink() || !details.isFile()) return null;
    const canonical = realpathSync(candidate);
    return inside(root, canonical) && canonical !== root ? canonical : null;
  } catch {
    return null;
  }
}

function fileWithinLimit(path: string, maximumBytes: number): boolean {
  try {
    return statSync(path).size <= maximumBytes;
  } catch {
    return false;
  }
}

function loadStaticAssets(directoryValue?: string): StaticAssets {
  const directory = resolve(
    directoryValue ??
      fileURLToPath(new URL("./assets", import.meta.url)),
  );
  const read = (name: string): Uint8Array => {
    const candidate = join(directory, name);
    const path = safeRegularFile(directory, candidate);
    if (path === null) {
      throw new StudioHttpError(
        500,
        "missing-assets",
        "Studio's local interface assets are unavailable.",
      );
    }
    return readFileSync(path);
  };
  return {
    html: new TextDecoder().decode(read("index.html")),
    css: read("studio.css"),
    javascript: read("studio.js"),
  };
}

function jsonResponse(
  value: unknown,
  status = 200,
): Response {
  return new Response(`${JSON.stringify(value)}\n`, {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
    },
  });
}

function errorResponse(error: ErrorResponse): Response {
  return jsonResponse({
    error: error.code,
    message: error.message,
  }, error.status);
}

function escapeHtmlText(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function agentPrivacyNotice(agent: StudioFactoryDefaults["agent"]): string {
  if (agent === null) {
    return "No coding agent is available, so contact-sheet viewing remains available but creation requires Codex or Claude Code on PATH.";
  }
  return `Studio will use ${agent.label}, the local coding agent selected at startup, under that agent's own provider and privacy settings.`;
}

function safeError(error: unknown): ErrorResponse {
  if (error instanceof StudioHttpError) {
    return { status: error.status, code: error.code, message: error.message };
  }
  if (error instanceof CliError) {
    return {
      status: error.exitCode === 2 ? 400 : 422,
      code: "factory-error",
      message: error.message,
    };
  }
  if (error instanceof Error && error.name === "SimulatorError") {
    const record = error as Error & { code?: unknown };
    const code = typeof record.code === "string"
      ? record.code.toLowerCase().replaceAll("_", "-")
      : "simulator-unavailable";
    return {
      status: record.code === "LIVE_PREVIEW_BUSY" ? 409 : 503,
      code,
      message: error.message,
    };
  }
  return {
    status: 500,
    code: "studio-error",
    message: "Studio could not complete the local request.",
  };
}

function pathSegments(pathname: string): string[] {
  try {
    return pathname.split("/").filter(Boolean).map((value) =>
      decodeURIComponent(value)
    );
  } catch {
    throw new StudioHttpError(400, "invalid-path", "Studio received an invalid path.");
  }
}

function safeShot(
  shotsDirectory: string,
  slug: string,
): DiscoveredShot {
  try {
    return recognizedShotBySlug(shotsDirectory, slug);
  } catch {
    throw new StudioHttpError(
      404,
      "shot-not-found",
      "That shot was not found in this local workspace.",
    );
  }
}

function screenshotPath(shot: DiscoveredShot): string | null {
  const candidate = join(
    shot.path,
    ".tohseno",
    "artifacts",
    "screenshot.png",
  );
  const path = safeRegularFile(shot.path, candidate);
  if (path === null) return null;
  try {
    if (!fileWithinLimit(path, MAX_SCREENSHOT_BYTES)) return null;
    return detectImageType(readFileSync(path))?.mediaType === "image/png"
      ? path
      : null;
  } catch {
    return null;
  }
}

function screenshotUrl(shot: DiscoveredShot): string | null {
  const path = screenshotPath(shot);
  if (path === null) return null;
  try {
    const version = Math.floor(statSync(path).mtimeMs);
    return `/api/shots/${encodeURIComponent(shot.metadata.slug)}/screenshot?v=${version}`;
  } catch {
    return null;
  }
}

function contactShot(shot: DiscoveredShot): Record<string, unknown> {
  const journal = safeRegularFile(
    shot.path,
    join(shot.path, ".tohseno", "provenance", "events.jsonl"),
  );
  let status = "READY";
  if (
    journal !== null &&
    fileWithinLimit(journal, MAX_PROGRESS_JOURNAL_BYTES)
  ) {
    try {
      const events = readProgressJournal(journal).filter(
        (event) => event.slug === shot.metadata.slug,
      );
      status = shotOperationalStatus(events);
    } catch {
      // Legacy or concurrently replaced journals fall back to a usable shot.
    }
  }
  return {
    slug: shot.metadata.slug,
    name: shot.name,
    createdAt: shot.metadata.createdAt,
    sequence: shot.metadata.sequence ?? null,
    status,
    screenshotUrl: screenshotUrl(shot),
  };
}

function provenanceRecord(shot: DiscoveredShot): Record<string, unknown> | null {
  const candidate = join(
    shot.path,
    ".tohseno",
    "provenance",
    "provenance.json",
  );
  const path = safeRegularFile(shot.path, candidate);
  if (path === null) return null;
  try {
    if (!fileWithinLimit(path, MAX_PROVENANCE_BYTES)) return null;
    const value = JSON.parse(readFileSync(path, "utf8")) as unknown;
    return isRecord(value) ? value : null;
  } catch {
    return null;
  }
}

function privateShotInput(shot: DiscoveredShot): PrivateShotInput {
  const provenance = provenanceRecord(shot);
  if (provenance === null) {
    return { intention: null, references: [] };
  }
  let intention: string | null = null;
  const intentionRecord = provenance.intention;
  if (
    isRecord(intentionRecord) &&
    intentionRecord.path === "intention.md"
  ) {
    const path = safeRegularFile(
      shot.path,
      join(shot.path, ".tohseno", "provenance", "intention.md"),
    );
    if (path !== null && fileWithinLimit(path, MAX_INTENTION_BYTES)) {
      try {
        intention = readFileSync(path, "utf8");
      } catch {
        intention = null;
      }
    }
  }

  const records = Array.isArray(provenance.references)
    ? provenance.references.slice(0, MAX_REFERENCES)
    : [];
  const references: SafeReference[] = [];
  for (const value of records) {
    if (!isRecord(value)) continue;
    const path = value.path;
    const originalName = value.originalName;
    const mediaType = value.mediaType;
    if (
      typeof path !== "string" ||
      !/^references\/reference-[0-9]{3}\.(?:png|jpg|webp|gif|heic|avif)$/u.test(path) ||
      typeof originalName !== "string" ||
      typeof mediaType !== "string"
    ) {
      continue;
    }
    const candidate = safeRegularFile(
      shot.path,
      join(shot.path, ".tohseno", "provenance", path),
    );
    if (candidate === null || !fileWithinLimit(candidate, MAX_REFERENCE_BYTES)) {
      continue;
    }
    references.push({
      path: candidate,
      originalFilename: basename(originalName),
      mediaType,
    });
  }
  return { intention, references };
}

function detailShot(shot: DiscoveredShot): Record<string, unknown> {
  const input = privateShotInput(shot);
  return {
    ...contactShot(shot),
    intention: input.intention,
    references: input.references.map((reference, index) => ({
      originalFilename: reference.originalFilename,
      mediaType: reference.mediaType,
      url:
        `/api/shots/${encodeURIComponent(shot.metadata.slug)}/references/${index}`,
      imageUrl:
        `/api/shots/${encodeURIComponent(shot.metadata.slug)}/references/${index}`,
    })),
    creation: shot.metadata.creation === undefined
      ? null
      : {
        door: shot.metadata.creation.door,
        inputDigest: shot.metadata.creation.inputDigest,
        referenceCount: shot.metadata.creation.referenceCount,
        options: shot.metadata.creation.options,
      },
    factory: {
      releaseId: shot.metadata.factory.releaseId,
      cliVersion: shot.metadata.factory.cliVersion,
      templateVersion: shot.metadata.factory.templateVersion,
    },
  };
}

function imageResponse(
  path: string,
  mediaType: string,
  filename: string,
): Response {
  return new Response(readFileSync(path), {
    headers: {
      "Content-Disposition": `inline; filename="${filename}"`,
      "Content-Type": mediaType,
    },
  });
}

function responseBody(bytes: Uint8Array): ArrayBuffer {
  return Uint8Array.from(bytes).buffer;
}

function sseLine(
  value: unknown,
  options: { id?: number; event?: string } = {},
): Uint8Array {
  const parts = [
    ...(options.id === undefined ? [] : [`id: ${options.id}`]),
    ...(options.event === undefined ? [] : [`event: ${options.event}`]),
    `data: ${JSON.stringify(value)}`,
    "",
    "",
  ];
  return new TextEncoder().encode(parts.join("\n"));
}

function safeLastEventId(request: Request): number {
  const value = request.headers.get("last-event-id");
  if (value === null || value === "") return 0;
  if (!/^[0-9]{1,12}$/u.test(value)) return 0;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : 0;
}

function jobEventStream(
  request: Request,
  manager: StudioJobManager,
  jobId: string,
  registerCloser: (closer: () => void) => () => void,
): Response {
  // Resolve a missing job before committing an SSE response.
  manager.events(jobId);
  const afterId = safeLastEventId(request);
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      let closed = false;
      let unsubscribe = (): void => {};
      let unregister = (): void => {};
      let heartbeat: ReturnType<typeof setInterval> | null = null;
      const onAbort = (): void => {
        close();
      };
      const close = (): void => {
        if (closed) return;
        closed = true;
        unsubscribe();
        unregister();
        request.signal.removeEventListener("abort", onAbort);
        if (heartbeat !== null) clearInterval(heartbeat);
        try {
          controller.close();
        } catch {
          // The browser may already have closed its local stream.
        }
      };
      unregister = registerCloser(close);
      const send = (record: { id: number; event: ShotProgressEvent }): void => {
        if (closed) return;
        try {
          controller.enqueue(sseLine(record.event, { id: record.id }));
        } catch {
          close();
          return;
        }
        if (
          record.event.type === "completed" ||
          record.event.type === "failed" ||
          record.event.type === "interrupted"
        ) {
          queueMicrotask(close);
        }
      };
      unsubscribe = manager.subscribe(jobId, afterId, send);
      if (closed) {
        unsubscribe();
        return;
      }
      heartbeat = setInterval(() => {
        if (closed) return;
        try {
          controller.enqueue(new TextEncoder().encode(": keep-alive\n\n"));
        } catch {
          close();
        }
      }, HEARTBEAT_INTERVAL_MS);
      heartbeat.unref?.();
      request.signal.addEventListener("abort", onAbort, { once: true });
      if (manager.isTerminal(jobId)) queueMicrotask(close);
    },
  });
  return new Response(stream, {
    headers: {
      "Connection": "keep-alive",
      "Content-Type": "text/event-stream; charset=utf-8",
      "X-Accel-Buffering": "no",
    },
  });
}

function workspaceEventStream(
  request: Request,
  observer: WorkspaceObserver,
  registerCloser: (closer: () => void) => () => void,
): Response {
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      let closed = false;
      let unsubscribe = (): void => {};
      let unregister = (): void => {};
      let heartbeat: ReturnType<typeof setInterval> | null = null;
      const onAbort = (): void => {
        close();
      };
      const close = (): void => {
        if (closed) return;
        closed = true;
        unsubscribe();
        unregister();
        request.signal.removeEventListener("abort", onAbort);
        if (heartbeat !== null) clearInterval(heartbeat);
        try {
          controller.close();
        } catch {
          // The browser may already have closed its local stream.
        }
      };
      unregister = registerCloser(close);
      unsubscribe = observer.subscribe((event) => {
        if (closed) return;
        const eventName = NAMED_WORKSPACE_EVENTS.has(event.type)
          ? event.type
          : undefined;
        try {
          controller.enqueue(sseLine(event, {
            id: event.id,
            ...(eventName === undefined ? {} : { event: eventName }),
          }));
        } catch {
          close();
        }
      });
      if (closed) {
        unsubscribe();
        return;
      }
      heartbeat = setInterval(() => {
        if (closed) return;
        try {
          controller.enqueue(new TextEncoder().encode(": keep-alive\n\n"));
        } catch {
          close();
        }
      }, HEARTBEAT_INTERVAL_MS);
      heartbeat.unref?.();
      request.signal.addEventListener("abort", onAbort, { once: true });
    },
  });
  return new Response(stream, {
    headers: {
      "Connection": "keep-alive",
      "Content-Type": "text/event-stream; charset=utf-8",
      "X-Accel-Buffering": "no",
    },
  });
}

function validLivePreviewUrl(value: string): boolean {
  const match = LIVE_PREVIEW_URL.exec(value);
  if (match === null) return false;
  const port = Number(match[1]);
  return Number.isSafeInteger(port) && port >= 1 && port <= 65_535;
}

function publicActionResult(
  action: StudioActionName,
  value: StudioActionResult | void,
): StudioActionResult {
  const message =
    typeof value?.message === "string" && value.message.length <= 500
      ? value.message
      : undefined;
  if (action !== "preview") {
    return message === undefined ? {} : { message };
  }
  const url = value?.url;
  if (typeof url !== "string" || !validLivePreviewUrl(url)) {
    throw new StudioHttpError(
      502,
      "unsafe-preview-url",
      "The live preview helper returned an invalid local URL.",
    );
  }
  return {
    ...(message === undefined ? {} : { message }),
    url,
  };
}

function routeLabel(method: string, segments: readonly string[]): string {
  if (segments.length === 0) return "studio-shell";
  if (segments.length === 1 && ["studio.css", "studio.js"].includes(segments[0]!)) {
    return "studio-asset";
  }
  if (segments[0] === "shots") return "studio-shot-shell";
  if (segments[0] !== "api") return "not-found";
  if (segments[1] === "events") return "workspace-events";
  if (segments[1] === "jobs") return "job-events";
  if (segments[1] !== "shots") return "api-not-found";
  if (segments.length === 2) {
    return method === "POST" ? "create-shot" : "list-shots";
  }
  if (segments.length === 3) return "shot-detail";
  if (segments[3] === "screenshot") return "shot-screenshot";
  if (segments[3] === "references") return "shot-reference";
  return "shot-action";
}

export class StudioApplication {
  readonly security: StudioRequestSecurity;
  readonly #creation: StudioFactoryDefaults;
  readonly #factory: StudioShotFactory | undefined;
  readonly #actions: StudioActions;
  readonly #observer: WorkspaceObserver;
  readonly #assets: StaticAssets;
  readonly #now: () => Date;
  readonly #logger: ((record: StudioRequestLog) => void) | undefined;
  readonly #dispose: (() => void | Promise<void>) | undefined;
  readonly #streamClosers = new Set<() => void>();
  readonly #operations: StudioOperationGate;
  readonly #jobs: StudioJobManager;
  #closed = false;

  constructor(options: StudioApplicationOptions) {
    this.#creation = options.creation;
    this.#factory = options.factory;
    this.#actions = options.actions ?? {};
    this.#assets = loadStaticAssets(options.assetsDirectory);
    this.#now = options.now ?? (() => new Date());
    this.#logger = options.logger;
    this.#dispose = options.dispose;
    this.security = new StudioRequestSecurity(options.security);
    this.#operations = new StudioOperationGate();
    this.#observer = options.observer ?? new WorkspaceObserver({
      shotsDirectory: options.creation.config.shotsDirectory,
      now: this.#now,
    });
    this.#jobs = new StudioJobManager({
      creation: this.#creation,
      operations: this.#operations,
      ...(this.#factory === undefined ? {} : { factory: this.#factory }),
      now: this.#now,
      onActivity: () => {
        this.#observer.requestScan();
      },
    });
    this.#observer.start();
  }

  readonly fetch = async (request: Request): Promise<Response> => {
    const started = performance.now();
    let route = "unmatched";
    let status = 500;
    let code: string | undefined;
    try {
      if (this.#closed) {
        throw new StudioHttpError(
          503,
          "studio-stopping",
          "Studio is shutting down.",
        );
      }
      this.security.assertHost(request);
      const url = new URL(request.url);
      const segments = pathSegments(url.pathname);
      route = routeLabel(request.method, segments);
      const routed = await this.#route(request, segments);
      route = routed.route;
      status = routed.response.status;
      return withStudioSecurityHeaders(
        routed.response,
        route === "studio-shell" || route === "studio-shot-shell"
          ? { html: true }
          : {},
      );
    } catch (error) {
      const safe = safeError(error);
      status = safe.status;
      code = safe.code;
      return withStudioSecurityHeaders(errorResponse(safe));
    } finally {
      try {
        this.#logger?.({
          method: request.method,
          route,
          status,
          durationMs: Math.max(0, Math.round(performance.now() - started)),
          ...(code === undefined ? {} : { code }),
        });
      } catch {
        // Logging is presentation-only and may never break a local request.
      }
    }
  };

  setPort(port: number): void {
    this.security.setPort(port);
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#observer.close();
    for (const close of [...this.#streamClosers]) close();
    this.#streamClosers.clear();
    await this.#jobs.close();
    await this.#operations.close();
    await this.#dispose?.();
  }

  async #route(
    request: Request,
    segments: readonly string[],
  ): Promise<RoutedResponse> {
    if (request.method !== "GET" && request.method !== "POST") {
      throw new StudioHttpError(
        405,
        "method-not-allowed",
        "Studio does not support that request method.",
      );
    }
    if (request.method === "GET" && segments.length === 0) {
      return { route: "studio-shell", response: this.#shellResponse() };
    }
    if (
      request.method === "GET" &&
      segments.length === 1 &&
      segments[0] === "studio.css"
    ) {
      return {
        route: "studio-asset",
        response: new Response(responseBody(this.#assets.css), {
          headers: { "Content-Type": "text/css; charset=utf-8" },
        }),
      };
    }
    if (
      request.method === "GET" &&
      segments.length === 1 &&
      segments[0] === "studio.js"
    ) {
      return {
        route: "studio-asset",
        response: new Response(responseBody(this.#assets.javascript), {
          headers: {
            "Content-Type": "text/javascript; charset=utf-8",
          },
        }),
      };
    }
    if (
      request.method === "GET" &&
      segments[0] === "shots" &&
      (segments.length === 2 ||
        (segments.length === 3 && segments[2] === "live"))
    ) {
      safeShot(this.#creation.config.shotsDirectory, segments[1]!);
      return { route: "studio-shot-shell", response: this.#shellResponse() };
    }
    if (segments[0] !== "api") {
      throw new StudioHttpError(404, "not-found", "Studio route not found.");
    }
    if (
      request.method === "GET" &&
      segments.length === 2 &&
      segments[1] === "events"
    ) {
      return {
        route: "workspace-events",
        response: workspaceEventStream(
          request,
          this.#observer,
          (closer) => this.#registerStreamCloser(closer),
        ),
      };
    }
    if (
      request.method === "GET" &&
      segments.length === 4 &&
      segments[1] === "jobs" &&
      segments[3] === "events"
    ) {
      return {
        route: "job-events",
        response: jobEventStream(
          request,
          this.#jobs,
          segments[2]!,
          (closer) => this.#registerStreamCloser(closer),
        ),
      };
    }
    if (segments[1] !== "shots") {
      throw new StudioHttpError(404, "not-found", "Studio API route not found.");
    }
    if (request.method === "GET" && segments.length === 2) {
      const shots = discoverShotsNewestFirst(
        this.#creation.config.shotsDirectory,
      ).map(contactShot);
      return {
        route: "list-shots",
        response: jsonResponse({ count: shots.length, shots }),
      };
    }
    if (request.method === "POST" && segments.length === 2) {
      this.security.assertMutation(request);
      this.#operations.assertAvailable();
      const staged = await stageStudioCreationRequest({
        request,
        factoryHome: this.#creation.config.factoryHome,
      });
      const job = this.#jobs.start(staged);
      return {
        route: "create-shot",
        response: jsonResponse(job, 202),
      };
    }
    if (segments.length < 3) {
      throw new StudioHttpError(404, "not-found", "Studio API route not found.");
    }
    const shot = safeShot(
      this.#creation.config.shotsDirectory,
      segments[2]!,
    );
    if (request.method === "GET" && segments.length === 3) {
      return {
        route: "shot-detail",
        response: jsonResponse(detailShot(shot)),
      };
    }
    if (
      request.method === "GET" &&
      segments.length === 4 &&
      segments[3] === "screenshot"
    ) {
      const path = screenshotPath(shot);
      if (path === null) {
        throw new StudioHttpError(
          404,
          "screenshot-not-found",
          "This shot does not have a Simulator capture yet.",
        );
      }
      return {
        route: "shot-screenshot",
        response: imageResponse(path, "image/png", "screenshot.png"),
      };
    }
    if (
      request.method === "GET" &&
      segments.length === 5 &&
      segments[3] === "references" &&
      /^[0-9]{1,2}$/u.test(segments[4]!)
    ) {
      const input = privateShotInput(shot);
      const index = Number(segments[4]);
      const reference = input.references[index];
      if (reference === undefined) {
        throw new StudioHttpError(
          404,
          "reference-not-found",
          "That local reference image was not found.",
        );
      }
      const bytes = readFileSync(reference.path);
      const detected = detectImageType(bytes);
      if (
        detected === null ||
        detected.mediaType !== reference.mediaType
      ) {
        throw new StudioHttpError(
          404,
          "reference-not-found",
          "That local reference image was not found.",
        );
      }
      return {
        route: "shot-reference",
        response: imageResponse(
          reference.path,
          detected.mediaType,
          `reference-${String(index + 1).padStart(3, "0")}${detected.extension}`,
        ),
      };
    }
    if (
      request.method === "POST" &&
      segments.length === 4 &&
      ACTION_NAMES.has(segments[3] as StudioActionName)
    ) {
      this.security.assertMutation(request);
      const action = segments[3] as StudioActionName;
      const callback = this.#actions[action];
      if (callback === undefined) {
        throw new StudioHttpError(
          501,
          "action-unavailable",
          "That shot action is not available in this Studio build.",
        );
      }
      const invoke = async (
        operationSignal: AbortSignal = request.signal,
      ): Promise<StudioActionResult | void> => {
        const controller = new AbortController();
        const abort = (): void => controller.abort();
        request.signal.addEventListener("abort", abort, { once: true });
        if (operationSignal !== request.signal) {
          operationSignal.addEventListener("abort", abort, { once: true });
        }
        if (request.signal.aborted || operationSignal.aborted) abort();
        try {
          return await callback(shot, {
            signal: controller.signal,
          });
        } finally {
          request.signal.removeEventListener("abort", abort);
          if (operationSignal !== request.signal) {
            operationSignal.removeEventListener("abort", abort);
          }
        }
      };
      const heavyOperation: StudioHeavyOperation | null =
        action === "run" || action === "preview" || action === "verify"
          ? action
          : null;
      const value = heavyOperation === null
        ? await invoke()
        : await this.#operations.run(
            heavyOperation,
            async (signal) => await invoke(signal),
          );
      return {
        route: "shot-action",
        response: jsonResponse(publicActionResult(action, value)),
      };
    }
    throw new StudioHttpError(404, "not-found", "Studio API route not found.");
  }

  #shellResponse(): Response {
    const count = discoverShotsNewestFirst(
      this.#creation.config.shotsDirectory,
    ).length;
    const html = this.#assets.html
      .replaceAll("{{SESSION_TOKEN}}", this.security.sessionToken)
      .replaceAll("{{INITIAL_COUNT}}", String(count))
      .replaceAll(
        "{{AGENT_PRIVACY_NOTICE}}",
        escapeHtmlText(agentPrivacyNotice(this.#creation.agent)),
      );
    return new Response(html, {
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
  }

  #registerStreamCloser(closer: () => void): () => void {
    this.#streamClosers.add(closer);
    return () => {
      this.#streamClosers.delete(closer);
    };
  }
}

export function createStudioApplication(
  options: StudioApplicationOptions,
): StudioApplication {
  return new StudioApplication(options);
}

export type { WorkspaceStudioEvent };
