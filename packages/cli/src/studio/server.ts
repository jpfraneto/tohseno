import {
  existsSync,
  lstatSync,
  realpathSync,
} from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import {
  detectInstalledAgents,
  sanitizedAgentEnvironment,
  type AgentAdapter,
} from "../agents.ts";
import type { ResolvedConfig } from "../config.ts";
import { createShot } from "../creation.ts";
import { CliError } from "../errors.ts";
import {
  executeCommand,
  SimulatorService,
  type LivePreviewHandle,
} from "../simulator.ts";
import { trustedShotToolFromCache } from "../trusted-tools.ts";
import { canonicalShotsDirectory } from "../workspace.ts";
import {
  createStudioApplication,
  type StudioApplication,
  type StudioRequestLog,
} from "./application.ts";
import { MAX_STUDIO_UPLOAD_BYTES } from "./uploads.ts";

export const DEFAULT_STUDIO_PORT = 4747;
export const STUDIO_HOST = "127.0.0.1" as const;

export interface StudioServerOptions {
  config: ResolvedConfig;
  cwd: string;
  environment: Record<string, string | undefined>;
  sourceRoot?: string;
  port?: number;
  simulator?: SimulatorService;
  logger?: (record: StudioRequestLog) => void;
  openUrl?: (url: string) => Promise<void>;
}

export interface StudioServerHandle {
  readonly url: string;
  readonly port: number;
  readonly application: StudioApplication;
  readonly selectedAgent: AgentAdapter | null;
  open(): Promise<void>;
  stop(): Promise<void>;
}

function isInside(root: string, candidate: string): boolean {
  const fromRoot = relative(resolve(root), resolve(candidate));
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function selectedStudioAgent(
  config: ResolvedConfig,
  environment: Record<string, string | undefined>,
  cwd: string,
): AgentAdapter | null {
  const installed = detectInstalledAgents(environment.PATH ?? "", cwd);
  return installed.find((agent) => agent.id === config.defaultAgent) ??
    installed[0] ??
    null;
}

async function requireSuccessfulAction(
  argv: readonly string[],
  cwd: string,
  environment: Record<string, string | undefined>,
  message: string,
  signal?: AbortSignal,
): Promise<void> {
  const result = await executeCommand(argv, {
    cwd,
    environment: sanitizedAgentEnvironment(environment),
    ...(signal === undefined ? {} : { signal }),
  });
  if (result.exitCode !== 0) throw new CliError(message);
}

function xcodeProject(shotRoot: string): string {
  const project = join(shotRoot, "Writing.xcodeproj");
  if (!existsSync(project)) {
    throw new CliError("the shot is missing its generated Xcode project");
  }
  const details = lstatSync(project);
  const canonical = realpathSync(project);
  if (
    details.isSymbolicLink() ||
    !details.isDirectory() ||
    !isInside(shotRoot, canonical)
  ) {
    throw new CliError("the shot has an unsafe Xcode project path");
  }
  return canonical;
}

async function defaultOpenUrl(
  urlValue: string,
  environment: Record<string, string | undefined>,
  cwd: string,
): Promise<void> {
  const url = new URL(urlValue);
  if (
    url.protocol !== "http:" ||
    (url.hostname !== "127.0.0.1" && url.hostname !== "localhost")
  ) {
    throw new CliError("Studio refused to open a non-local URL");
  }
  const executable = process.platform === "darwin"
    ? "/usr/bin/open"
    : Bun.which("xdg-open");
  if (executable === null || !existsSync(executable)) {
    throw new CliError(
      `a browser launcher is unavailable; open ${url.origin} manually`,
    );
  }
  await requireSuccessfulAction(
    [executable, url.href],
    cwd,
    environment,
    "the local Studio URL could not be opened",
  );
}

function requestedPort(value: number | undefined): number {
  const port = value ?? DEFAULT_STUDIO_PORT;
  if (!Number.isSafeInteger(port) || port < 0 || port > 65_535) {
    throw new CliError("Studio port must be an integer from 0 to 65535", 2);
  }
  return port;
}

export function startStudioServer(
  options: StudioServerOptions,
): StudioServerHandle {
  const shotsDirectory = canonicalShotsDirectory(
    options.config.shotsDirectory,
  );
  const config: ResolvedConfig = {
    ...options.config,
    shotsDirectory,
  };
  const selectedAgent = selectedStudioAgent(
    config,
    options.environment,
    options.cwd,
  );
  const simulator = options.simulator ?? new SimulatorService({
    environment: options.environment,
    cwd: options.cwd,
    releasesDirectory: config.cacheDirectory,
  });
  let activePreview: LivePreviewHandle | null = null;
  let previewOperation: AbortController | null = null;
  const application = createStudioApplication({
    creation: {
      config,
      cwd: options.cwd,
      environment: options.environment,
      ...(options.sourceRoot === undefined
        ? {}
        : { sourceRoot: options.sourceRoot }),
      agent: selectedAgent,
      runner: simulator.creationRunner(),
    },
    security: {
      hostname: STUDIO_HOST,
      // Port 0 is resolved immediately after Bun binds below.
      port: requestedPort(options.port) || DEFAULT_STUDIO_PORT,
    },
    factory: async (request) => {
      if (request.agent === null) {
        throw new CliError(
          "Studio needs Codex or Claude Code on PATH to turn an intention into an app; contact-sheet viewing remains available",
          3,
        );
      }
      return await createShot(request);
    },
    actions: {
      verify: async (shot, context) => {
        const trusted = trustedShotToolFromCache({
          shotRoot: shot.path,
          releasesDirectory: config.cacheDirectory,
          tool: "verify",
        });
        await requireSuccessfulAction(
          [process.execPath, trusted.executable],
          trusted.root,
          sanitizedAgentEnvironment(options.environment),
          "shot verification failed",
          context.signal,
        );
        return { message: "SHOT VERIFIED." };
      },
      run: async (shot, context) => {
        await simulator.runShot({
          shotRoot: shot.path,
          environment: options.environment,
          signal: context.signal,
        });
        return { message: "SHOT IS RUNNING IN APPLE SIMULATOR." };
      },
      preview: async (shot, context) => {
        if (simulator.livePreview.status().active) {
          await activePreview?.stop();
          await simulator.livePreview.stop();
          activePreview = null;
        }
        const controller = new AbortController();
        const forwardAbort = (): void => controller.abort();
        context.signal.addEventListener("abort", forwardAbort, { once: true });
        if (context.signal.aborted) forwardAbort();
        previewOperation = controller;
        try {
          const result = await simulator.runAndPreview({
            shotRoot: shot.path,
            environment: options.environment,
            signal: controller.signal,
          });
          activePreview = result.preview;
          return { url: result.preview.iframeUrl() };
        } finally {
          context.signal.removeEventListener("abort", forwardAbort);
          if (previewOperation === controller) previewOperation = null;
        }
      },
      "stop-preview": async () => {
        previewOperation?.abort();
        await activePreview?.stop();
        await simulator.livePreview.stop();
        activePreview = null;
        return { message: "LIVE PREVIEW STOPPED." };
      },
      "open-xcode": async (shot) => {
        await requireSuccessfulAction(
          ["/usr/bin/open", "-a", "Xcode", xcodeProject(shot.path)],
          shot.path,
          options.environment,
          "Xcode could not be opened for this shot",
        );
        return { message: "XCODE OPENED." };
      },
      reveal: async (shot) => {
        await requireSuccessfulAction(
          ["/usr/bin/open", "-R", shot.path],
          shot.path,
          options.environment,
          "the shot folder could not be revealed",
        );
        return { message: "SHOT FOLDER REVEALED." };
      },
    },
    ...(options.logger === undefined ? {} : { logger: options.logger }),
    dispose: async () => {
      previewOperation?.abort();
      previewOperation = null;
      await activePreview?.stop();
      activePreview = null;
      await simulator.dispose();
    },
  });

  const port = requestedPort(options.port);
  let server: Bun.Server<undefined>;
  try {
    server = Bun.serve({
      hostname: STUDIO_HOST,
      port,
      maxRequestBodySize: MAX_STUDIO_UPLOAD_BYTES,
      fetch: application.fetch,
    });
  } catch {
    void application.close();
    throw new CliError(
      port === 0
        ? "Studio could not bind a local port"
        : `Studio could not bind http://${STUDIO_HOST}:${port}; choose another port with --port`,
      3,
    );
  }
  const boundPort = server.port;
  if (boundPort === undefined) {
    void application.close();
    void server.stop(true);
    throw new CliError("Studio did not receive a local port", 3);
  }
  application.setPort(boundPort);
  const url = `http://${STUDIO_HOST}:${boundPort}`;
  let stopping: Promise<void> | null = null;
  const stop = async (): Promise<void> => {
    stopping ??= (async () => {
      try {
        await application.close();
      } finally {
        await server.stop(true);
      }
    })();
    return await stopping;
  };
  return {
    url,
    port: boundPort,
    application,
    selectedAgent,
    open: async () => {
      await (options.openUrl ?? (async (localUrl) =>
        await defaultOpenUrl(
          localUrl,
          options.environment,
          options.cwd,
        )))(url);
    },
    stop,
  };
}

export async function waitForStudioSignal(): Promise<"SIGINT" | "SIGTERM"> {
  return await new Promise((resolveSignal) => {
    const interrupt = (): void => {
      cleanup();
      resolveSignal("SIGINT");
    };
    const terminate = (): void => {
      cleanup();
      resolveSignal("SIGTERM");
    };
    const cleanup = (): void => {
      process.off("SIGINT", interrupt);
      process.off("SIGTERM", terminate);
    };
    process.once("SIGINT", interrupt);
    process.once("SIGTERM", terminate);
  });
}
