import { createHash, randomUUID } from "node:crypto";
import {
  constants as fsConstants,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import {
  sanitizedAgentEnvironment,
  type AgentAdapter,
} from "./agents.ts";
import type { ResolvedConfig } from "./config.ts";
import { CliError, errorMessage } from "./errors.ts";
import type { CliIo } from "./io.ts";
import {
  listRegularFiles,
  readBoundedRegularFile,
  readBoundedUtf8,
} from "./files.ts";
import { runCaptured, runInherited } from "./process.ts";
import {
  normalizeCreationInput,
  type CreationInput,
  type NormalizedCreationInput,
} from "./provenance.ts";
import {
  ShotProgressReporter,
  type CreationDoor,
  type ShotProgressEvent,
  type ShotProgressSink,
} from "./progress.ts";
import {
  prepareFactoryRelease,
  useActiveCachedRelease,
  type PreparedRelease,
} from "./release.ts";
import {
  materializeShot,
  readShotMetadata,
  type CreatedShot,
  type ShotMetadata,
} from "./shot.ts";
import { slugForShotName, validateShotSlug } from "./slug.ts";
import { locateFactorySourceRoot } from "./source.ts";
import { trustedShotToolFromRelease } from "./trusted-tools.ts";
import {
  allocateShotSequence,
  canonicalShotsDirectory,
} from "./workspace.ts";

export const AUTOMATED_AGENT_INSTRUCTION = [
  "Read the local AGENTS.md, skills/continuity-app/SKILL.md, and .tohseno/OPERATIONS.md.",
  "The owner's private normalized creation input is in .tohseno/provenance/intention.md and any reference images are in .tohseno/provenance/references/.",
  "Build the requested continuity app completely from that input, choosing sensible defaults without asking questions.",
  "Use the private creation input only in this selected coding-agent session; never quote it in output, log it, commit it, or send it to any other destination.",
  "Run the shot's required verification and simulator checks when available.",
  "Do not deploy, publish, spend money, alter DNS, submit to an app store, or perform another externally consequential action.",
].join(" ");

export interface CreationRunnerResult {
  screenshotPath: string | null;
  previewAvailable: boolean;
  message?: string;
}

export interface CreationRunner {
  runShot(
    shotRoot: string,
    options: {
      signal?: AbortSignal;
      onProgress?: (type: "building" | "simulator-launching") => void | Promise<void>;
    },
  ): Promise<CreationRunnerResult>;
}

export interface CreateShotRequest {
  config: ResolvedConfig;
  cwd: string;
  environment: Record<string, string | undefined>;
  sourceRoot?: string;
  slug?: string;
  name?: string;
  door: CreationDoor;
  input?: CreationInput;
  agent: AgentAdapter | null;
  noLaunch: boolean;
  verifyAfterAgent?: boolean;
  runAfterCreate?: boolean;
  jobId?: string;
  now?: () => Date;
  signal?: AbortSignal;
  onProgress?: ShotProgressSink;
  io?: CliIo;
  runner?: CreationRunner;
}

export interface CreateShotResult extends CreatedShot {
  jobId: string;
  release: PreparedRelease;
  input: NormalizedCreationInput;
  sequence: number;
  agentMode: "interactive" | "automated" | "none";
  screenshotPath: string | null;
  previewAvailable: boolean;
}

export async function factoryReleaseFor(
  request: Pick<CreateShotRequest, "config" | "environment" | "sourceRoot">,
): Promise<PreparedRelease> {
  let sourceRoot: string;
  try {
    sourceRoot = request.sourceRoot ??
      locateFactorySourceRoot(request.environment);
  } catch (sourceError) {
    try {
      return useActiveCachedRelease(request.config.cacheDirectory);
    } catch (cacheError) {
      throw new CliError(
        `factory source is unavailable (${errorMessage(sourceError)}) and cached fallback failed: ${errorMessage(cacheError)}`,
      );
    }
  }
  return await prepareFactoryRelease(
    sourceRoot,
    request.config.cacheDirectory,
  );
}

async function requireGit(request: CreateShotRequest): Promise<void> {
  try {
    const result = await runCaptured(["git", "--version"], {
      cwd: request.cwd,
      env: request.environment,
    });
    if (result.exitCode !== 0) throw new Error("Git returned a failure");
  } catch {
    throw new CliError(
      "Git is required to create an independent shot; install Git and retry",
      3,
    );
  }
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw new CliError("shot creation was interrupted");
}

function automatedAgentArguments(agent: AgentAdapter): string[] {
  if (agent.id === "codex") {
    return [
      agent.executable,
      "--sandbox", "workspace-write",
      "--ask-for-approval", "never",
      "exec",
      "--color", "never",
      AUTOMATED_AGENT_INSTRUCTION,
    ];
  }
  return [
    agent.executable,
    "--print",
    "--permission-mode", "acceptEdits",
    "--no-session-persistence",
    AUTOMATED_AGENT_INSTRUCTION,
  ];
}

async function runAutomatedAgent(
  agent: AgentAdapter,
  shotRoot: string,
  environment: Record<string, string | undefined>,
  signal?: AbortSignal,
): Promise<number> {
  const child = Bun.spawn(automatedAgentArguments(agent), {
    cwd: shotRoot,
    env: sanitizedAgentEnvironment(environment),
    stdin: "ignore",
    stdout: "ignore",
    stderr: "ignore",
  });
  let exited = false;
  let escalation: ReturnType<typeof setTimeout> | undefined;
  const abort = (): void => {
    try {
      child.kill("SIGTERM");
    } catch {
      // The process may already have exited.
    }
    escalation ??= setTimeout(() => {
      if (exited) return;
      try {
        child.kill("SIGKILL");
      } catch {
        // The process may already have exited.
      }
    }, 5_000);
    escalation.unref?.();
  };
  signal?.addEventListener("abort", abort, { once: true });
  if (signal?.aborted) abort();
  try {
    const exitCode = await child.exited;
    exited = true;
    return exitCode;
  } finally {
    exited = true;
    if (escalation !== undefined) clearTimeout(escalation);
    signal?.removeEventListener("abort", abort);
  }
}

async function verifyPublishedShot(
  shotRoot: string,
  environment: Record<string, string | undefined>,
  release: PreparedRelease,
): Promise<void> {
  const trusted = trustedShotToolFromRelease({
    shotRoot,
    release,
    tool: "verify",
  });
  const result = await runCaptured(
    [process.execPath, trusted.executable],
    { cwd: trusted.root, env: sanitizedAgentEnvironment(environment) },
  );
  if (result.exitCode !== 0) {
    throw new CliError(
      `shot verification failed after the coding agent exited (status ${result.exitCode})`,
    );
  }
}

function assertImmutableShotMetadata(
  shotRoot: string,
  expected: ShotMetadata,
): void {
  const path = join(shotRoot, ".tohseno", "shot.json");
  if (!existsSync(path)) {
    throw new CliError(
      "the coding agent changed immutable .tohseno/shot.json creation provenance",
    );
  }
  let details;
  try {
    details = lstatSync(path);
  } catch {
    throw new CliError(
      "the coding agent changed immutable .tohseno/shot.json creation provenance",
    );
  }
  if (details.isSymbolicLink() || !details.isFile()) {
    throw new CliError(
      "the coding agent changed immutable .tohseno/shot.json creation provenance",
    );
  }
  const current = readShotMetadata(shotRoot);
  if (
    current === undefined ||
    JSON.stringify(current) !== JSON.stringify(expected)
  ) {
    throw new CliError(
      "the coding agent changed immutable .tohseno/shot.json creation provenance",
    );
  }
}

function privateCreationSnapshot(shotRoot: string): string {
  const root = join(shotRoot, ".tohseno", "provenance");
  const details = lstatSync(root);
  const canonicalShot = realpathSync(shotRoot);
  const canonicalRoot = realpathSync(root);
  if (
    details.isSymbolicLink() ||
    !details.isDirectory() ||
    canonicalRoot === canonicalShot ||
    !inside(canonicalShot, canonicalRoot)
  ) {
    throw new CliError(
      "the coding agent changed immutable private creation provenance",
    );
  }
  const references = join(root, "references");
  if (!existsSync(references)) {
    throw new CliError(
      "the coding agent changed immutable private creation provenance",
    );
  }
  const referenceDetails = lstatSync(references);
  const canonicalReferences = realpathSync(references);
  if (
    (details.mode & 0o777) !== 0o700 ||
    referenceDetails.isSymbolicLink() ||
    !referenceDetails.isDirectory() ||
    (referenceDetails.mode & 0o777) !== 0o700 ||
    canonicalReferences === canonicalRoot ||
    !inside(canonicalRoot, canonicalReferences) ||
    readdirSync(root, { withFileTypes: true }).some(
      (entry) => entry.isDirectory() && entry.name !== "references",
    ) ||
    readdirSync(references, { withFileTypes: true }).some(
      (entry) => entry.isDirectory(),
    )
  ) {
    throw new CliError(
      "the coding agent changed immutable private creation provenance",
    );
  }
  const hash = createHash("sha256");
  for (const file of listRegularFiles(root)) {
    if (file.relativePath === "events.jsonl") continue;
    const fileDetails = lstatSync(file.absolutePath);
    hash.update(file.relativePath);
    hash.update("\0");
    hash.update(
      readBoundedRegularFile(
        file.absolutePath,
        12 * 1_048_576,
        "private creation provenance input",
      ),
    );
    hash.update("\0");
    hash.update(String(fileDetails.mode & 0o777));
    hash.update("\0");
  }
  return hash.digest("hex");
}

function assertPrivateCreationUnchanged(
  shotRoot: string,
  expectedSnapshot: string,
): void {
  let currentSnapshot: string;
  try {
    currentSnapshot = privateCreationSnapshot(shotRoot);
  } catch {
    throw new CliError(
      "the coding agent changed immutable private creation provenance",
    );
  }
  if (currentSnapshot !== expectedSnapshot) {
    throw new CliError(
      "the coding agent changed immutable private creation provenance",
    );
  }
}

interface ProtectedCreationState {
  metadataSource: string;
  provenanceSource: string;
  input: NormalizedCreationInput;
}

function captureProtectedCreationState(
  shotRoot: string,
  input: NormalizedCreationInput,
): ProtectedCreationState {
  return {
    metadataSource: readBoundedUtf8(
      join(shotRoot, ".tohseno", "shot.json"),
      65_536,
      "shot metadata",
    ),
    provenanceSource: readBoundedUtf8(
      join(shotRoot, ".tohseno", "provenance", "provenance.json"),
      1_048_576,
      "private creation provenance record",
    ),
    input,
  };
}

function restoreProtectedCreationState(
  shotRoot: string,
  state: ProtectedCreationState,
): void {
  const canonicalShot = realpathSync(shotRoot);
  const local = join(canonicalShot, ".tohseno");
  const localDetails = lstatSync(local);
  const canonicalLocal = realpathSync(local);
  if (
    localDetails.isSymbolicLink() ||
    !localDetails.isDirectory() ||
    canonicalLocal === canonicalShot ||
    !inside(canonicalShot, canonicalLocal)
  ) {
    throw new CliError("the shot-local metadata directory became unsafe");
  }

  const temporary = join(
    canonicalLocal,
    `.provenance-restoring-${process.pid}-${randomUUID()}`,
  );
  const references = join(temporary, "references");
  mkdirSync(references, { recursive: true, mode: 0o700 });
  try {
    if (state.input.intention !== null) {
      writeFileSync(join(temporary, "intention.md"), state.input.intention, {
        mode: 0o600,
      });
    }
    for (const [index, reference] of state.input.references.entries()) {
      const internalName =
        `reference-${String(index + 1).padStart(3, "0")}${reference.extension}`;
      writeFileSync(join(references, internalName), reference.bytes, {
        mode: 0o600,
      });
    }
    writeFileSync(
      join(temporary, "provenance.json"),
      state.provenanceSource,
      { mode: 0o600 },
    );

    const provenance = join(canonicalLocal, "provenance");
    rmSync(provenance, { recursive: true, force: true });
    renameSync(temporary, provenance);

    const metadataTemporary = join(
      canonicalLocal,
      `.shot-restoring-${process.pid}-${randomUUID()}.json`,
    );
    try {
      writeFileSync(metadataTemporary, state.metadataSource, {
        flag: "wx",
        mode: 0o644,
      });
      renameSync(metadataTemporary, join(canonicalLocal, "shot.json"));
    } finally {
      if (existsSync(metadataTemporary)) unlinkSync(metadataTemporary);
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

function isolateUnsafePublishedShot(options: {
  shotRoot: string;
  shotsDirectory: string;
  slug: string;
  jobId: string;
}): string {
  const root = realpathSync(options.shotsDirectory);
  const details = lstatSync(options.shotRoot);
  const canonicalShot = realpathSync(options.shotRoot);
  if (
    details.isSymbolicLink() ||
    !details.isDirectory() ||
    canonicalShot === root ||
    !inside(root, canonicalShot)
  ) {
    throw new CliError("the unsafe published shot could not be isolated safely");
  }
  const isolated = join(
    root,
    `.${options.slug}.unsafe-${options.jobId}-${randomUUID()}`,
  );
  renameSync(canonicalShot, isolated);
  return isolated;
}

async function verifyAgentResultOrIsolate(options: {
  shotRoot: string;
  shotsDirectory: string;
  slug: string;
  jobId: string;
  environment: Record<string, string | undefined>;
  release: PreparedRelease;
}): Promise<void> {
  try {
    await verifyPublishedShot(
      options.shotRoot,
      options.environment,
      options.release,
    );
  } catch (verificationError) {
    let isolated: string;
    try {
      isolated = isolateUnsafePublishedShot(options);
    } catch (isolationError) {
      throw new CliError(
        `post-agent verification failed and the unsafe shot could not be isolated: ${
          errorMessage(verificationError)
        }; ${errorMessage(isolationError)}`,
      );
    }
    throw new CliError(
      `post-agent verification failed; the unsafe shot was isolated at ${isolated}: ${
        errorMessage(verificationError)
      }`,
    );
  }
}

function enforceProtectedCreationState(options: {
  shotRoot: string;
  shotsDirectory: string;
  slug: string;
  jobId: string;
  expectedMetadata: ShotMetadata;
  expectedSnapshot: string;
  state: ProtectedCreationState;
}): void {
  let violation: unknown;
  try {
    assertImmutableShotMetadata(options.shotRoot, options.expectedMetadata);
    assertPrivateCreationUnchanged(
      options.shotRoot,
      options.expectedSnapshot,
    );
    return;
  } catch (error) {
    violation = error;
  }

  try {
    restoreProtectedCreationState(options.shotRoot, options.state);
    assertImmutableShotMetadata(options.shotRoot, options.expectedMetadata);
    assertPrivateCreationUnchanged(
      options.shotRoot,
      options.expectedSnapshot,
    );
  } catch (repairError) {
    try {
      const isolated = isolateUnsafePublishedShot({
        shotRoot: options.shotRoot,
        shotsDirectory: options.shotsDirectory,
        slug: options.slug,
        jobId: options.jobId,
      });
      throw new CliError(
        `${errorMessage(violation)}; automatic repair failed and the unsafe shot was isolated at ${isolated}: ${errorMessage(repairError)}`,
      );
    } catch (isolationError) {
      if (isolationError instanceof CliError &&
        isolationError.message.includes("was isolated at")) {
        throw isolationError;
      }
      throw new CliError(
        `${errorMessage(violation)}; automatic repair and safe isolation failed: ${errorMessage(repairError)}; ${errorMessage(isolationError)}`,
      );
    }
  }
  throw new CliError(
    `${errorMessage(violation)}; the factory restored the protected creation provenance`,
  );
}

function portableEventsPath(shotRoot: string): string {
  return join(shotRoot, ".tohseno", "provenance", "events.jsonl");
}

function inside(root: string, candidate: string): boolean {
  const fromRoot = relative(resolve(root), resolve(candidate));
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function copyPortableEvents(
  shotRoot: string | null,
  reporter: ShotProgressReporter,
): void {
  if (shotRoot === null || !existsSync(shotRoot)) return;
  const canonicalShot = realpathSync(shotRoot);
  const sourceDetails = lstatSync(reporter.journalPath);
  if (sourceDetails.isSymbolicLink() || !sourceDetails.isFile()) {
    throw new CliError("the workspace progress journal became unsafe");
  }
  const local = join(canonicalShot, ".tohseno");
  const provenance = join(local, "provenance");
  for (const [path, label, boundary] of [
    [local, "shot-local metadata directory", canonicalShot],
    [provenance, "private provenance directory", local],
  ] as const) {
    if (!existsSync(path)) throw new CliError(`${label} is missing`);
    const details = lstatSync(path);
    const canonical = realpathSync(path);
    if (
      details.isSymbolicLink() ||
      !details.isDirectory() ||
      !inside(boundary, canonical) ||
      canonical === boundary
    ) {
      throw new CliError(`${label} became unsafe`);
    }
  }
  const destination = portableEventsPath(canonicalShot);
  const temporary = join(
    provenance,
    `.events-${process.pid}-${randomUUID()}.jsonl`,
  );
  try {
    copyFileSync(
      reporter.journalPath,
      temporary,
      fsConstants.COPYFILE_EXCL,
    );
    renameSync(temporary, destination);
  } finally {
    if (existsSync(temporary)) {
      const details = lstatSync(temporary);
      if (!details.isSymbolicLink() && details.isFile()) unlinkSync(temporary);
    }
  }
}

function requestedSlug(
  request: CreateShotRequest,
  sequence: number,
): string {
  if (request.slug !== undefined) return validateShotSlug(request.slug);
  if (request.name !== undefined && request.name.trim() !== "") {
    return slugForShotName(request.name.trim());
  }
  return `shot-${String(sequence).padStart(3, "0")}`;
}

function agentMode(
  request: CreateShotRequest,
  input: NormalizedCreationInput,
): "interactive" | "automated" | "none" {
  if (request.noLaunch || request.agent === null) return "none";
  return input.intention === null ? "interactive" : "automated";
}

export function terminalProgressSink(io: CliIo): ShotProgressSink {
  const labels: Partial<Record<ShotProgressEvent["type"], string>> = {
    allocated: "Shot allocated.",
    "preparing-release": "Preparing the pinned factory release…",
    "preparing-shot": "Preparing the shot atomically…",
    "provenance-written": "Private input provenance saved locally.",
    "manifest-validated": "Manifest valid.",
    "baseline-committed": "Baseline committed.",
    published: "Shot published.",
    "agent-started": "Coding agent started.",
    "agent-completed": "Coding agent completed.",
    verifying: "Verifying the shot…",
    building: "Building for iOS Simulator…",
    "simulator-launching": "Launching the iOS Simulator…",
    "screenshot-captured": "Simulator screenshot captured.",
    "preview-unavailable": "Interactive preview is unavailable on this machine.",
    completed: "Creation complete.",
    interrupted: "Creation interrupted.",
  };
  return (event) => {
    const label = labels[event.type];
    if (label !== undefined) io.out(label);
  };
}

export async function createShot(
  request: CreateShotRequest,
): Promise<CreateShotResult> {
  const normalizedInput = normalizeCreationInput(request.input);
  const shotsDirectory = canonicalShotsDirectory(
    request.config.shotsDirectory,
  );
  if (request.slug !== undefined) {
    const explicit = validateShotSlug(request.slug);
    const destination = join(shotsDirectory, explicit);
    if (existsSync(destination)) {
      throw new CliError(
        `target already exists; refusing to overwrite: ${destination}`,
      );
    }
  }
  await requireGit(request);
  const sequence = await allocateShotSequence(shotsDirectory);
  const slug = requestedSlug(request, sequence);
  const destination = join(shotsDirectory, slug);
  if (existsSync(destination)) {
    throw new CliError(`target already exists; refusing to overwrite: ${destination}`);
  }
  const now = request.now ?? (() => new Date());
  const createdAt = now();
  const jobId = request.jobId ?? randomUUID();
  const sinks = [
    ...(request.onProgress === undefined ? [] : [request.onProgress]),
    ...(request.io === undefined ? [] : [terminalProgressSink(request.io)]),
  ];
  const reporter = new ShotProgressReporter({
    shotsDirectory,
    jobId,
    door: request.door,
    now,
    sinks,
  });
  const mode = agentMode(request, normalizedInput);
  const verifyAfterAgent = request.verifyAfterAgent ?? (mode !== "none");
  const runAfterCreate = request.runAfterCreate ?? mode === "automated";
  let shotRoot: string | null = null;
  let created: CreatedShot | null = null;
  let release: PreparedRelease | null = null;
  let screenshotPath: string | null = null;
  let previewAvailable = false;
  let verifiedAfterAgent = false;
  await reporter.emit({ type: "allocated", slug, sequence });
  try {
    throwIfAborted(request.signal);
    await reporter.emit({
      type: "preparing-release",
      slug,
      sequence,
    });
    release = await factoryReleaseFor(request);
    created = await materializeShot({
      slug,
      shotsDirectory,
      release,
      selectedAgent: request.agent?.id ?? null,
      sequence,
      door: request.door,
      input: normalizedInput,
      agentMode: mode,
      verifyAfterAgent,
      runAfterCreate,
      environment: request.environment,
      now: createdAt,
      ...(request.signal === undefined ? {} : { signal: request.signal }),
      emit: async (event) => {
        await reporter.emit(event);
      },
    });
    shotRoot = created.path;
    copyPortableEvents(shotRoot, reporter);
    const provenanceSnapshot = privateCreationSnapshot(shotRoot);
    const protectedCreation = captureProtectedCreationState(
      shotRoot,
      normalizedInput,
    );
    throwIfAborted(request.signal);

    if (mode !== "none" && request.agent !== null) {
      await reporter.emit({ type: "agent-started", slug, sequence });
      let exitCode: number;
      try {
        exitCode = mode === "interactive"
          ? await runInherited(
            [request.agent.executable, ...request.agent.launchArguments],
            {
              cwd: created.path,
              env: sanitizedAgentEnvironment(request.environment),
            },
          )
          : await runAutomatedAgent(
            request.agent,
            created.path,
            request.environment,
            request.signal,
          );
      } finally {
        enforceProtectedCreationState({
          shotRoot: created.path,
          shotsDirectory,
          slug,
          jobId,
          expectedMetadata: created.metadata,
          expectedSnapshot: provenanceSnapshot,
          state: protectedCreation,
        });
      }
      if (verifyAfterAgent) {
        await reporter.emit({ type: "verifying", slug, sequence });
        await verifyAgentResultOrIsolate({
          shotRoot: created.path,
          shotsDirectory,
          slug,
          jobId,
          environment: request.environment,
          release,
        });
        verifiedAfterAgent = true;
      }
      if (exitCode !== 0) {
        throw new CliError(
          `${request.agent.label} exited with status ${exitCode}; the verified local shot remains at ${created.path}`,
          exitCode,
        );
      }
      await reporter.emit({ type: "agent-completed", slug, sequence });
    }

    throwIfAborted(request.signal);
    if (verifyAfterAgent && !verifiedAfterAgent) {
      await reporter.emit({ type: "verifying", slug, sequence });
      await verifyPublishedShot(created.path, request.environment, release);
    }

    if (runAfterCreate && request.runner !== undefined) {
      const runResult = await request.runner.runShot(created.path, {
        ...(request.signal === undefined ? {} : { signal: request.signal }),
        onProgress: async (type) => {
          await reporter.emit({ type, slug, sequence });
        },
      });
      screenshotPath = runResult.screenshotPath;
      previewAvailable = runResult.previewAvailable;
      if (runResult.screenshotPath !== null) {
        await reporter.emit({ type: "screenshot-captured", slug, sequence });
      } else if (!runResult.previewAvailable) {
        await reporter.emit({
          type: "preview-unavailable",
          slug,
          sequence,
          ...(runResult.message === undefined
            ? {}
            : { message: runResult.message }),
        });
      }
    }

    await reporter.emit({ type: "completed", slug, sequence });
    copyPortableEvents(shotRoot, reporter);
    return {
      ...created,
      jobId,
      release,
      input: normalizedInput,
      sequence,
      agentMode: mode,
      screenshotPath,
      previewAvailable,
    };
  } catch (error) {
    const interrupted = request.signal?.aborted === true;
    try {
      await reporter.emit({
        type: interrupted ? "interrupted" : "failed",
        slug,
        sequence,
        message: interrupted
          ? "Creation stopped safely."
          : "Creation failed. Immediate command diagnostics were not retained in the progress journal.",
      });
    } finally {
      try {
        copyPortableEvents(shotRoot, reporter);
      } catch {
        // Preserve the original creation error without following an unsafe
        // path that a failed coding-agent run may have left behind.
      }
    }
    throw error;
  }
}
