import { describe, expect, test } from "bun:test";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { resolveConfig } from "../src/config.ts";
import { createShot } from "../src/creation.ts";
import {
  MAX_PROGRESS_JOURNAL_BYTES,
  progressJournalPath,
  readProgressJournal,
  ShotProgressReporter,
} from "../src/progress.ts";
import {
  createStudioApplication,
  type StudioApplication,
} from "../src/studio/application.ts";
import { MAX_STUDIO_UPLOAD_BYTES } from "../src/studio/uploads.ts";
import { startStudioServer } from "../src/studio/server.ts";
import {
  WorkspaceObserver,
  type WorkspaceStudioEvent,
} from "../src/studio/observer.ts";
import type { CreationProvenance } from "../src/provenance.ts";
import {
  REPOSITORY_ROOT,
  withScratchEnvironment,
} from "./helpers.ts";

interface BoundApplication {
  origin: string;
  server: Bun.Server<undefined>;
  close(): Promise<void>;
}

function bindApplication(application: StudioApplication): BoundApplication {
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    maxRequestBodySize: MAX_STUDIO_UPLOAD_BYTES,
    fetch: application.fetch,
  });
  if (server.port === undefined) throw new Error("test server has no port");
  application.setPort(server.port);
  return {
    origin: `http://127.0.0.1:${server.port}`,
    server,
    async close() {
      await application.close();
      await server.stop(true);
    },
  };
}

async function studioFetch(
  bound: BoundApplication,
  application: StudioApplication,
  path: string,
  options: RequestInit = {},
): Promise<Response> {
  const scopedPath = path === "/api"
    ? application.security.apiBase
    : path.startsWith("/api/")
      ? `${application.security.apiBase}${path.slice(4)}`
      : path;
  return await fetch(`${bound.origin}${scopedPath}`, {
    ...options,
    headers: {
      Cookie:
        `${application.security.cookieName}=${application.security.sessionToken}`,
      ...(options.method === "POST"
        ? {
            Origin: bound.origin,
          }
        : {}),
      ...(options.headers ?? {}),
    },
  });
}

describe("Tohseno Studio local server", () => {
  test("binds to loopback, serves the contact sheet, and shuts down cleanly", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      let opened = "";
      const studio = startStudioServer({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        port: 0,
        openTarget: async (path) => {
          opened = path;
        },
      });
      expect(studio.url).toMatch(/^http:\/\/127\.0\.0\.1:[0-9]+$/);
      expect(studio.selectedAgent).toBeNull();
      const shell = await fetch(studio.url);
      expect(shell.status).toBe(200);
      const shellText = await shell.text();
      expect(shellText).toContain("YOUR CONTACT SHEET");
      expect(shellText).toContain(
        "No coding agent is available, so contact-sheet viewing remains available",
      );
      expect(shellText).toContain(
        'id="watch-indicator" role="status" aria-live="polite" aria-atomic="true"',
      );
      expect(shellText).toContain("<dt>LAST CREATION ACTIVITY</dt>");
      expect(shellText).not.toContain("{{AGENT_PRIVACY_NOTICE}}");
      expect(shellText).not.toContain(studio.application.security.sessionToken);
      expect(shell.headers.get("content-security-policy")).toContain(
        "frame-src 'self' http://127.0.0.1:*",
      );
      const clientScript = await fetch(`${studio.url}/studio.js`);
      expect(clientScript.status).toBe(200);
      const clientScriptText = await clientScript.text();
      expect(clientScriptText).toContain('return `CREATION / ${status}`;');
      expect(clientScriptText).toContain('if (!quiet) setDetailStatus("");');
      await studio.open();
      expect(opened).toBe(studio.launcherPath);
      expect(statSync(studio.launcherPath).mode & 0o777).toBe(0o600);
      expect(readFileSync(studio.launcherPath, "utf8")).toContain(
        "#tohseno-session=",
      );

      await studio.stop();
      expect(existsSync(studio.launcherPath)).toBe(false);
      const afterClose = await studio.application.fetch(new Request(
        `${studio.url}/api/shots`,
        { headers: { Host: `127.0.0.1:${studio.port}` } },
      ));
      expect(afterClose.status).toBe(503);
    });
  });

  test("identifies the selected coding agent without allowing shell markup injection", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: {
            id: "codex",
            label: "Codex <configured> & reviewed",
            binary: "codex",
            executable: "/usr/bin/false",
            launchArguments: [],
          },
        },
        security: { port: 4747 },
      });
      try {
        const shell = await application.fetch(new Request(
          "http://127.0.0.1:4747/",
          { headers: { Host: "127.0.0.1:4747" } },
        ));
        const shellText = await shell.text();
        expect(shell.status).toBe(200);
        expect(shellText).toContain(
          "Studio will use Codex &lt;configured&gt; &amp; reviewed",
        );
        expect(shellText).not.toContain("Codex <configured> & reviewed");
        expect(shellText).not.toContain("{{AGENT_PRIVACY_NOTICE}}");
      } finally {
        await application.close();
      }
    });
  });

  test("ships non-dismissible job acceptance and resilient dialog focus recovery", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: null,
        },
        security: { port: 4747 },
      });
      try {
        const headers = { Host: "127.0.0.1:4747" };
        const shell = await application.fetch(new Request(
          "http://127.0.0.1:4747/",
          { headers },
        ));
        const shellText = await shell.text();
        expect(shell.status).toBe(200);
        expect(shellText).toContain('id="header-create-action"');
        expect(shellText).toContain(
          'class="shot-actions" role="group" aria-label="Shot actions"',
        );
        expect(shellText).toContain("Choose up to eight files, 12 MiB each");

        const client = await application.fetch(new Request(
          "http://127.0.0.1:4747/studio.js",
          { headers },
        ));
        const clientText = await client.text();
        expect(client.status).toBe(200);
        expect(clientText).toContain("createRequestPending: false");
        expect(clientText).toContain(
          'elements.createForm.toggleAttribute("aria-busy", pending);',
        );
        expect(clientText).toContain(
          'document.querySelectorAll("[data-close-create]")',
        );
        expect(clientText).toContain("control.disabled = pending;");
        expect(clientText).toContain(
          "if (state.createRequestPending) return;",
        );
        expect(clientText).toContain(
          'elements.createDialog?.addEventListener("cancel", (event) => {',
        );
        expect(clientText).toContain(
          "if (state.createRequestPending) event.preventDefault();",
        );
        expect(clientText).toContain(
          'element.closest("[hidden], [inert], [aria-hidden=\'true\']")',
        );
        expect(clientText).toContain(
          '!candidate.matches("[data-open-create]")',
        );
        expect(clientText).toContain("elements.headerCreateAction");
        expect(clientText).toContain("elements.studioMain");
      } finally {
        await application.close();
      }
    });
  });

  test("rejects hostile hosts, origins, sessions, and traversal", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: null,
        },
        security: { port: 4747 },
      });
      try {
        const hostileHost = await application.fetch(new Request(
          "http://127.0.0.1:4747/api/shots",
          { headers: { Host: "attacker.example" } },
        ));
        expect(hostileHost.status).toBe(421);

        const hostileOrigin = await application.fetch(new Request(
          "http://127.0.0.1:4747/api/session",
          {
            method: "POST",
            headers: {
              Host: "127.0.0.1:4747",
              Origin: "http://attacker.example",
              "X-Tohseno-Session": application.security.sessionToken,
            },
          },
        ));
        expect(hostileOrigin.status).toBe(403);

        const hostileSession = await application.fetch(new Request(
          "http://127.0.0.1:4747/api/session",
          {
            method: "POST",
            headers: {
              Host: "127.0.0.1:4747",
              Origin: "http://127.0.0.1:4747",
              "X-Tohseno-Session": "A".repeat(43),
            },
          },
        ));
        expect(hostileSession.status).toBe(403);

        const bootstrap = await application.fetch(new Request(
          "http://127.0.0.1:4747/api/session",
          {
            method: "POST",
            headers: {
              Host: "127.0.0.1:4747",
              Origin: "http://127.0.0.1:4747",
              "X-Tohseno-Session": application.security.sessionToken,
            },
          },
        ));
        expect(bootstrap.status).toBe(200);
        expect(await bootstrap.json()).toEqual({
          apiBase: application.security.apiBase,
        });
        expect(bootstrap.headers.get("set-cookie")).toContain("HttpOnly");
        expect(bootstrap.headers.get("set-cookie")).toContain(
          `Path=/__tohseno/${application.security.sessionId}/`,
        );

        const missingCookie = await application.fetch(new Request(
          `http://127.0.0.1:4747${application.security.apiBase}/shots`,
          { headers: { Host: "127.0.0.1:4747" } },
        ));
        expect(missingCookie.status).toBe(403);

        const traversal = await application.fetch(new Request(
          `http://127.0.0.1:4747${application.security.apiBase}/shots/..%2Foutside`,
          {
            headers: {
              Host: "127.0.0.1:4747",
              Cookie:
                `${application.security.cookieName}=${application.security.sessionToken}`,
            },
          },
        ));
        expect(traversal.status).toBe(404);
      } finally {
        await application.close();
      }
    });
  });

  test("creates through the shared factory, streams progress, and validates uploads", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      let factoryCalls = 0;
      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: null,
        },
        security: { port: 4747 },
        factory: async (request) => {
          factoryCalls += 1;
          return await createShot({
            ...request,
            agent: null,
            noLaunch: true,
            verifyAfterAgent: false,
            runAfterCreate: false,
          });
        },
      });
      const bound = bindApplication(application);
      try {
        const form = new FormData();
        form.set("name", "Studio Test");
        form.set("intention", "A private one-paragraph writing app.");
        form.append(
          "reference",
          new File([
            Buffer.from(
              "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z1iQAAAAASUVORK5CYII=",
              "base64",
            ),
          ], "sketch.png", { type: "image/png" }),
        );
        const started = await studioFetch(
          bound,
          application,
          "/api/shots",
          { method: "POST", body: form },
        );
        expect(started.status).toBe(202);
        const { jobId } = await started.json() as { jobId: string };
        expect(jobId).toMatch(/^[A-Za-z0-9-]{8,80}$/);

        const progress = await studioFetch(
          bound,
          application,
          `/api/jobs/${encodeURIComponent(jobId)}/events`,
        );
        expect(progress.status).toBe(200);
        const progressText = await progress.text();
        expect(progressText).toContain('"type":"allocated"');
        expect(progressText).toContain('"type":"completed"');
        expect(progressText).not.toContain("A private one-paragraph");
        expect(factoryCalls).toBe(1);

        const listed = await studioFetch(
          bound,
          application,
          "/api/shots",
        );
        expect(await listed.json()).toMatchObject({
          count: 1,
          shots: [{
            slug: "studio-test",
            status: "READY",
            screenshotUrl: null,
          }],
        });
        const detail = await studioFetch(
          bound,
          application,
          "/api/shots/studio-test",
        );
        expect(await detail.json()).toMatchObject({
          slug: "studio-test",
          intention: "A private one-paragraph writing app.\n",
          references: [{ originalFilename: "sketch.png" }],
          creation: { door: "studio", referenceCount: 1 },
        });
        const screenshot = await studioFetch(
          bound,
          application,
          "/api/shots/studio-test/screenshot",
        );
        expect(screenshot.status).toBe(404);

        const provenance = JSON.parse(readFileSync(
          join(
            scratch.shotsDirectory,
            "studio-test",
            ".tohseno",
            "provenance",
            "provenance.json",
          ),
          "utf8",
        )) as CreationProvenance;
        expect(provenance.door).toBe("studio");
        expect(provenance.references[0]?.originalName).toBe("sketch.png");
        const uploadRoot = join(scratch.factoryHome, "studio", "uploads");
        expect(existsSync(uploadRoot)).toBe(true);
        expect(readdirSync(uploadRoot)).toEqual([]);

        const invalid = new FormData();
        invalid.set("intention", "Use this visual.");
        invalid.append(
          "reference",
          new File(["not an image"], "fake.png", { type: "image/png" }),
        );
        const rejected = await studioFetch(
          bound,
          application,
          "/api/shots",
          { method: "POST", body: invalid },
        );
        expect(rejected.status).toBe(415);
        expect(factoryCalls).toBe(1);
      } finally {
        await bound.close();
      }
    });
  }, 30_000);

  test("serializes creation, run, preview, and verify operations", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      await createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "operation-target",
        door: "cli",
        agent: null,
        noLaunch: true,
      });

      let releaseFactory = (): void => {};
      const factoryBarrier = new Promise<void>((resolveBarrier) => {
        releaseFactory = resolveBarrier;
      });
      let releaseRun = (): void => {};
      const runBarrier = new Promise<void>((resolveBarrier) => {
        releaseRun = resolveBarrier;
      });
      let signalRunStarted = (): void => {};
      const runStarted = new Promise<void>((resolveStarted) => {
        signalRunStarted = resolveStarted;
      });
      let holdRun = false;
      let factoryCalls = 0;
      const actionCalls = {
        preview: 0,
        run: 0,
        verify: 0,
      };
      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: null,
        },
        security: { port: 4747 },
        factory: async (request) => {
          factoryCalls += 1;
          await factoryBarrier;
          return await createShot({
            ...request,
            agent: null,
            noLaunch: true,
            verifyAfterAgent: false,
            runAfterCreate: false,
          });
        },
        actions: {
          preview: async () => {
            actionCalls.preview += 1;
            return {
              url: "http://127.0.0.1:4748/_tohseno/live/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            };
          },
          run: async () => {
            actionCalls.run += 1;
            if (holdRun) {
              signalRunStarted();
              await runBarrier;
            }
          },
          verify: async () => {
            actionCalls.verify += 1;
          },
        },
      });
      const bound = bindApplication(application);
      try {
        const form = new FormData();
        form.set("name", "Operation Creation");
        form.set("intention", "Hold the shared Studio operation gate.");
        const started = await studioFetch(
          bound,
          application,
          "/api/shots",
          { method: "POST", body: form },
        );
        expect(started.status).toBe(202);
        const { jobId } = await started.json() as { jobId: string };

        for (const action of ["run", "preview", "verify"] as const) {
          const conflicting = await studioFetch(
            bound,
            application,
            `/api/shots/operation-target/${action}`,
            { method: "POST" },
          );
          expect(conflicting.status).toBe(409);
          expect(await conflicting.json()).toMatchObject({
            error: "operation-busy",
          });
        }
        expect(actionCalls).toEqual({ preview: 0, run: 0, verify: 0 });

        releaseFactory();
        const progress = await studioFetch(
          bound,
          application,
          `/api/jobs/${encodeURIComponent(jobId)}/events`,
        );
        expect(await progress.text()).toContain('"type":"completed"');
        expect(factoryCalls).toBe(1);

        holdRun = true;
        const running = studioFetch(
          bound,
          application,
          "/api/shots/operation-target/run",
          { method: "POST" },
        );
        await runStarted;

        const rejectedForm = new FormData();
        rejectedForm.set("name", "Rejected While Running");
        rejectedForm.set(
          "intention",
          "This upload must not outlive an operation conflict.",
        );
        const rejectedCreation = await studioFetch(
          bound,
          application,
          "/api/shots",
          { method: "POST", body: rejectedForm },
        );
        expect(rejectedCreation.status).toBe(409);
        expect(await rejectedCreation.json()).toMatchObject({
          error: "operation-busy",
        });
        expect(factoryCalls).toBe(1);
        expect(
          readdirSync(join(scratch.factoryHome, "studio", "uploads")),
        ).toEqual([]);

        const rejectedVerify = await studioFetch(
          bound,
          application,
          "/api/shots/operation-target/verify",
          { method: "POST" },
        );
        expect(rejectedVerify.status).toBe(409);
        expect(actionCalls.verify).toBe(0);

        releaseRun();
        expect((await running).status).toBe(200);
        expect(actionCalls.run).toBe(1);
      } finally {
        releaseFactory();
        releaseRun();
        await bound.close();
      }
    });
  }, 30_000);

  test("aborts and awaits an active heavy action during shutdown", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      await createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "shutdown-target",
        door: "cli",
        agent: null,
        noLaunch: true,
      });

      let signalStarted = (): void => {};
      const started = new Promise<void>((resolveStarted) => {
        signalStarted = resolveStarted;
      });
      let observedAbort = false;
      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: null,
        },
        security: { port: 4747 },
        actions: {
          run: async (_shot, context) => {
            signalStarted();
            await new Promise<void>((resolveAbort) => {
              const onAbort = (): void => {
                observedAbort = true;
                resolveAbort();
              };
              context.signal.addEventListener("abort", onAbort, { once: true });
              if (context.signal.aborted) onAbort();
            });
          },
        },
      });
      const bound = bindApplication(application);
      try {
        const running = studioFetch(
          bound,
          application,
          "/api/shots/shutdown-target/run",
          { method: "POST" },
        );
        await started;
        await application.close();
        expect(observedAbort).toBe(true);
        expect((await running).status).toBe(200);

        const afterClose = await studioFetch(
          bound,
          application,
          "/api/shots",
        );
        expect(afterClose.status).toBe(503);
      } finally {
        await bound.close();
      }
    });
  }, 30_000);

  test("derives in-progress and interrupted shot status from its journal", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const result = await createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "journal-status",
        door: "cli",
        agent: null,
        noLaunch: true,
      });
      const workspaceEvents = readProgressJournal(
        progressJournalPath(scratch.shotsDirectory, result.jobId),
      );
      const publishedIndex = workspaceEvents.findIndex(
        (event) => event.type === "published",
      );
      if (publishedIndex < 0) throw new Error("test shot was not published");
      const inProgress = workspaceEvents.slice(0, publishedIndex + 1);
      const portableJournal = join(
        result.path,
        ".tohseno",
        "provenance",
        "events.jsonl",
      );
      writeFileSync(
        portableJournal,
        `${inProgress.map((event) => JSON.stringify(event)).join("\n")}\n`,
      );

      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: null,
        },
        security: { port: 4747 },
      });
      const bound = bindApplication(application);
      const listedStatus = async (): Promise<string | undefined> => {
        const response = await studioFetch(
          bound,
          application,
          "/api/shots",
        );
        const value = await response.json() as {
          shots?: Array<{ status?: string }>;
        };
        return value.shots?.[0]?.status;
      };
      try {
        expect(await listedStatus()).toBe("CREATING");

        const published = inProgress.at(-1);
        if (published === undefined) throw new Error("test journal is empty");
        appendFileSync(
          portableJournal,
          `${JSON.stringify({
            ...published,
            at: new Date(Date.parse(published.at) + 1).toISOString(),
            type: "failed",
            message: "Factory stopped.",
          })}\n`,
        );
        expect(await listedStatus()).toBe("INTERRUPTED");

        writeFileSync(
          portableJournal,
          `${workspaceEvents.map((event) => JSON.stringify(event)).join("\n")}\n`,
        );
        expect(await listedStatus()).toBe("READY");
      } finally {
        await bound.close();
      }
    });
  }, 30_000);

  test("observes a shot created externally through the CLI factory", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const application = createStudioApplication({
        creation: {
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          agent: null,
        },
        security: { port: 4747 },
      });
      const events = await application.fetch(new Request(
        `http://127.0.0.1:4747${application.security.apiBase}/events`,
        {
          headers: {
            Host: "127.0.0.1:4747",
            Cookie:
              `${application.security.cookieName}=${application.security.sessionToken}`,
          },
        },
      ));
      const reader = events.body?.getReader();
      if (reader === undefined) throw new Error("workspace SSE has no body");
      try {
        await reader.read(); // initial ready event
        await createShot({
          config,
          cwd: scratch.root,
          environment: scratch.environment,
          sourceRoot: REPOSITORY_ROOT,
          slug: "external-cli-shot",
          door: "cli",
          agent: null,
          noLaunch: true,
        });

        let observed = "";
        const deadline = Date.now() + 4_000;
        while (
          !observed.includes("shot-created") &&
          !observed.includes("shots-changed") &&
          Date.now() < deadline
        ) {
          const next = await Promise.race([
            reader.read(),
            new Promise<null>((resolveTimeout) => {
              setTimeout(() => resolveTimeout(null), 1_000);
            }),
          ]);
          if (next === null || next.done) continue;
          observed += new TextDecoder().decode(next.value);
        }
        expect(observed).toMatch(/shot-created|shots-changed/);
      } finally {
        await reader.cancel();
        await application.close();
      }
    });
  }, 30_000);

  test("never follows a workspace progress journal outside the shots directory", async () => {
    await withScratchEnvironment((scratch) => {
      mkdirSync(scratch.shotsDirectory, { recursive: true });
      const externalControl = join(scratch.root, "external-control");
      const externalEvents = join(externalControl, "events");
      mkdirSync(externalEvents, { recursive: true });
      const event = {
        schemaVersion: 1,
        jobId: "escaped-job-0001",
        at: new Date().toISOString(),
        type: "allocated",
        door: "cli",
        slug: "escaped-shot",
      } as const;
      writeFileSync(
        join(externalEvents, `${event.jobId}.jsonl`),
        `${JSON.stringify(event)}\n`,
      );
      symlinkSync(
        externalControl,
        join(scratch.shotsDirectory, ".tohseno"),
        "dir",
      );

      expect(() =>
        new ShotProgressReporter({
          shotsDirectory: scratch.shotsDirectory,
          jobId: "reporter-job-0001",
          door: "cli",
        })
      ).toThrow("workspace control path is not a private directory");

      const observed: WorkspaceStudioEvent[] = [];
      const observer = new WorkspaceObserver({
        shotsDirectory: scratch.shotsDirectory,
        pollIntervalMs: 60_000,
      });
      const unsubscribe = observer.subscribe((record) => observed.push(record));
      try {
        observer.start();
        observer.requestScan();
        expect(observed.some((record) => record.slug === "escaped-shot")).toBe(
          false,
        );
      } finally {
        unsubscribe();
        observer.close();
      }
    });
  });

  test("skips oversized workspace progress journals", async () => {
    await withScratchEnvironment((scratch) => {
      const eventsDirectory = join(
        scratch.shotsDirectory,
        ".tohseno",
        "events",
      );
      mkdirSync(eventsDirectory, { recursive: true });
      const observed: WorkspaceStudioEvent[] = [];
      const observer = new WorkspaceObserver({
        shotsDirectory: scratch.shotsDirectory,
        pollIntervalMs: 60_000,
      });
      const unsubscribe = observer.subscribe((record) => observed.push(record));
      try {
        observer.start();
        const event = {
          schemaVersion: 1,
          jobId: "oversized-job-0001",
          at: new Date().toISOString(),
          type: "allocated",
          door: "studio",
          slug: "oversized-shot",
        } as const;
        writeFileSync(
          join(eventsDirectory, `${event.jobId}.jsonl`),
          `${JSON.stringify(event)}\n${"x".repeat(
            MAX_PROGRESS_JOURNAL_BYTES,
          )}\n`,
        );
        observer.requestScan();
        expect(
          observed.some((record) => record.slug === "oversized-shot"),
        ).toBe(false);
      } finally {
        unsubscribe();
        observer.close();
      }
    });
  });

  test("progress journals refuse link substitution and bound persisted diagnostics", async () => {
    await withScratchEnvironment(async (scratch) => {
      const events = join(
        scratch.shotsDirectory,
        ".tohseno",
        "events",
      );
      mkdirSync(events, { recursive: true });
      const victim = join(scratch.root, "owner-file");
      writeFileSync(victim, "preserve\n", { mode: 0o640 });
      const linkedPath = progressJournalPath(
        scratch.shotsDirectory,
        "linked-job-0001",
      );
      symlinkSync(victim, linkedPath);

      expect(() =>
        new ShotProgressReporter({
          shotsDirectory: scratch.shotsDirectory,
          jobId: "linked-job-0001",
          door: "cli",
        })
      ).toThrow("already exists or is unsafe");
      expect(readFileSync(victim, "utf8")).toBe("preserve\n");

      const reporter = new ShotProgressReporter({
        shotsDirectory: scratch.shotsDirectory,
        jobId: "bounded-job-0001",
        door: "studio",
      });
      const event = await reporter.emit({
        type: "preview-unavailable",
        message: `line one\n${"x".repeat(10_000)}`,
      });
      expect(event.message).not.toContain("\n");
      expect(Buffer.byteLength(event.message ?? "")).toBeLessThanOrEqual(2_048);
      expect(statSync(reporter.journalPath).mode & 0o077).toBe(0);

      appendFileSync(
        reporter.journalPath,
        "x".repeat(MAX_PROGRESS_JOURNAL_BYTES),
      );
      await expect(reporter.emit({ type: "completed" })).rejects.toThrow(
        "safety limit",
      );
      expect(readProgressJournal(reporter.journalPath)).toEqual([]);
    });
  });
});
