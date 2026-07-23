import { describe, expect, test } from "bun:test";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createApplication } from "../server.ts";
import type { TohsenoApplication } from "../server.ts";
import { loadConfig } from "../config.ts";

async function testApplication(): Promise<TohsenoApplication> {
  return createApplication({
    config: loadConfig({
      NODE_ENV: "test",
      PORT: "3000",
      BASE_URL: "http://localhost:3000",
    }),
  });
}

function request(path: string, init: RequestInit = {}): Request {
  return new Request(`http://localhost:3000${path}`, init);
}

const oneshotPath = fileURLToPath(new URL("../public/oneshot.sh", import.meta.url));
const installerPath = fileURLToPath(new URL("../public/install.sh", import.meta.url));
const openGraphImagePath = fileURLToPath(new URL("../public/og.png", import.meta.url));

describe("public pages", () => {
  test("serves the local CLI install path and no stale intake surface", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/"));
    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("curl -fsSL https://tohseno.com/install.sh | bash");
    expect(body).toContain("Run <code>tohseno</code>");
    expect(body).toContain("Tell your coding agent");
    expect(body).not.toContain("bun run tohseno:link");
    expect(body).toContain("Take another one.");
    expect(body).toContain("Every idea deserves a body.");
    expect(body).toContain("Most shots miss.");
    expect(body).toContain("Every independent shot begins alive");
    expect(body).toContain(">ONE SHOT</span>");
    expect(body).toContain("your /shots");
    expect(body).not.toMatch(/\b(?:revolutionary|unleash|empower)\b/iu);
    expect(body).not.toContain("four years");
    expect(body).not.toContain("$TOHSENO");
    expect(body).not.toContain("slot machine");
    expect(body).not.toContain('href="/intake"');
    expect(body).not.toContain("Managed intake");
    expect(body).toMatch(/property="og:image" content="http:\/\/localhost:3000\/og\.png\?v=[0-9a-f]{8}"/);
    expect(body).toContain('name="twitter:card" content="summary_large_image"');
    expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
    expect(response.headers.get("Content-Security-Policy")).toContain("default-src 'self'");
  });

  test("serves current factory docs and privacy", async () => {
    const application = await testApplication();
    for (const path of ["/docs", "/privacy"]) {
      const response = await application.fetch(request(path));
      expect(response.status).toBe(200);
      const body = await response.text();
      expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
      expect(body).not.toContain('href="/intake"');
      if (path === "/docs") {
        expect(body).toContain("Take another one");
        expect(body).toContain("Take your first shot");
        expect(body).toContain("The prototype is the payoff");
        expect(body).toContain("iOS is the only implemented app platform");
        expect(body).toContain(
          "tohseno create --file intention.md --reference sketch.png",
        );
        expect(body).toContain("One factory, multiple doors");
        expect(body).toContain("tohseno studio");
        expect(body).toContain("binds to <code>127.0.0.1</code>, never the LAN");
        expect(body).toContain("tohseno run &lt;shot&gt;");
        expect(body).toContain("tohseno preview &lt;shot&gt;");
        expect(body).toContain("it is not an in-browser iOS emulator");
        expect(body).toContain(
          "additionally requires Apple Silicon, a native arm64 Node.js 20 or newer",
        );
        expect(body).toContain("reports the selected Node architecture");
        expect(body).toContain("does not require paid Apple Developer Program membership");
        expect(body).toContain("<code>.tohseno/provenance/</code>");
        expect(body).toContain(
          "Studio does not upload shots or creation input to TOHSENO",
        );
      } else {
        expect(body).toContain("downloads only pinned release artifacts");
        expect(body).toContain("Quick Tunnel");
      }
    }
  });

  test("serves the health check", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/healthz"));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ status: "ok", service: "tohseno" });
  });

  test("serves static assets with correct content types", async () => {
    const application = await testApplication();
    const expectations: Array<[string, string]> = [
      ["/styles.css", "text/css"],
      ["/landing.css", "text/css"],
      ["/fonts/fraunces-latin.woff2", "font/woff2"],
      ["/fonts/plex-mono-latin.woff2", "font/woff2"],
      ["/app.js", "text/javascript"],
      ["/robots.txt", "text/plain"],
      ["/og.png", "image/png"],
      ["/install.sh", "text/x-shellscript"],
      ["/oneshot.sh", "text/x-shellscript"],
    ];
    for (const [path, type] of expectations) {
      const response = await application.fetch(request(path));
      expect(response.status).toBe(200);
      expect(response.headers.get("Content-Type")).toContain(type);
    }
    const openGraphImage = readFileSync(openGraphImagePath);
    expect(openGraphImage.subarray(1, 4).toString("ascii")).toBe("PNG");
    expect(openGraphImage.readUInt32BE(16)).toBe(1_200);
    expect(openGraphImage.readUInt32BE(20)).toBe(630);
  });

  test("the oneshot script must revalidate so a stale pin is never served", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/oneshot.sh"));
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=0, must-revalidate");
    const body = await response.text();
    expect(body).toContain('TOHSENO_PIN="35021b38e71257d137c184081a1ba0d4503fa5ef"');
    expect(body).toContain("This script no longer creates a workspace.");
    expect(body).toContain("curl -fsSL https://tohseno.com/install.sh | bash");
    expect(body).not.toContain('mkdir -p "$target"');
    expect(body).not.toContain('git -C "$target" init');
  });

  test("the canonical installer revalidates and exposes help without touching the machine", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/install.sh"));
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=0, must-revalidate");
    const body = await response.text();
    expect(body).toContain('CLI_VERSION="0.3.0"');
    expect(body).toContain("TOHSENO managed installer");
    expect(body).toContain("TOHSENO_INSTALL_CLI_SHA256");

    const child = Bun.spawn(["/bin/sh", installerPath, "--help"], {
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
    });
    const [exitCode, stdout, stderr] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
    ]);
    expect(exitCode).toBe(0);
    expect(stdout).toContain("--non-interactive");
    expect(stdout).toContain("managed Bun runtime");
    expect(stderr).toBe("");
  });

  test("HEAD requests return headers without a body", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/", { method: "HEAD" }));
    expect(response.status).toBe(200);
    expect(await response.text()).toBe("");
  });
});

describe("removed surfaces stay removed", () => {
  test("intake, status, capsule, checkout, and operator routes are gone", async () => {
    const application = await testApplication();
    for (const path of [
      "/intake",
      "/status/sub_aaaaaaaaaaaaaaaaaaaaaaaa",
      "/c/sub_aaaaaaaaaaaaaaaaaaaaaaaa",
      "/api/submissions",
      "/api/checkout",
      "/api/operator/submissions",
    ]) {
      const response = await application.fetch(request(path));
      expect(response.status).toBe(404);
    }
  });

  test("POST to a page path is method-not-allowed", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/", { method: "POST" }));
    expect(response.status).toBe(405);
    expect(response.headers.get("Allow")).toBe("GET, HEAD");
  });
});

describe("legacy oneshot migration", () => {
  test("prints the CLI migration and creates nothing", async () => {
    const scratch = mkdtempSync(join(tmpdir(), "tohseno-oneshot-migration-"));
    try {
      const child = Bun.spawn(["bash", oneshotPath], {
        cwd: scratch,
        env: { HOME: scratch, PATH: process.env.PATH ?? "" },
        stdin: "ignore",
        stdout: "pipe",
        stderr: "pipe",
      });
      const [exitCode, stdout, stderr] = await Promise.all([
        child.exited,
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
      ]);
      expect(exitCode).toBe(2);
      expect(stdout).toContain("This script no longer creates a workspace.");
      expect(stdout).toContain("curl -fsSL https://tohseno.com/install.sh | bash");
      expect(stdout).toContain("tohseno");
      expect(stderr).toBe("");
      expect(readdirSync(scratch)).toEqual([]);
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  });

  test("keeps help available without claiming creation", async () => {
    const child = Bun.spawn(["bash", oneshotPath, "--help"], {
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
    });
    const [exitCode, stdout] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
    ]);
    expect(exitCode).toBe(0);
    expect(stdout).toContain("legacy workspace creator is retired");
    expect(stdout).toContain("curl -fsSL https://tohseno.com/install.sh | bash");
  });
});

describe("canonical boundary", () => {
  test("redirects the www alias to the canonical origin", async () => {
    const application = await createApplication({
      config: loadConfig({
        NODE_ENV: "test",
        PORT: "3000",
        BASE_URL: "http://tohseno.com:3000",
      }),
    });
    const response = await application.fetch(
      new Request("http://www.tohseno.com:3000/docs?x=1"),
    );
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("http://tohseno.com:3000/docs?x=1");
  });
});
