import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
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

const installerPath = fileURLToPath(
  new URL("../public/install.sh", import.meta.url),
);
const oneshotInstallerPath = fileURLToPath(
  new URL("../public/oneshot.sh", import.meta.url),
);
const openGraphImagePath = fileURLToPath(
  new URL("../public/og.png", import.meta.url),
);
const faviconPath = fileURLToPath(
  new URL("../public/favicon.png", import.meta.url),
);
const shotIconDirectory = fileURLToPath(
  new URL("../public/shot-icons", import.meta.url),
);
const browserScriptPath = fileURLToPath(
  new URL("../public/app.js", import.meta.url),
);
const landingStylePath = fileURLToPath(
  new URL("../public/landing.css", import.meta.url),
);
const INSTALL_COMMAND = "curl -fsSL https://tohseno.com/oneshot.sh | bash";

describe("public pages", () => {
  test("serves the three-command terminal landing page", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/"));
    expect(response.status).toBe(200);
    const body = await response.text();
    const landingStyle = readFileSync(landingStylePath);
    const landingStyleRevision = createHash("sha256")
      .update(landingStyle)
      .digest("hex")
      .slice(0, 12);
    expect(body).toContain(`/landing.css?v=${landingStyleRevision}`);

    const installIndex = body.indexOf(`data-copy-value="${INSTALL_COMMAND}"`);
    const createIndex = body.indexOf(
      'data-copy-value="tohseno create my-app-name"',
    );
    const studioIndex = body.indexOf('data-copy-value="tohseno studio"');
    expect(installIndex).toBeGreaterThan(-1);
    expect(createIndex).toBeGreaterThan(installIndex);
    expect(studioIndex).toBeGreaterThan(createIndex);

    expect(body.match(/data-copy-command/g)).toHaveLength(3);
    expect(body.match(/data-copied-label="copied ✓"/g)).toHaveLength(3);
    expect(body).toMatch(/<button[^>]*hidden[^>]*data-copy-command/);
    expect(body).toContain('data-terminal');
    expect(body).toContain('data-term-idle');
    expect(body).toContain("GIVE EVERY IDEA A");
    expect(body).toContain('<span class="beta-tag">BETA</span>');
    expect(body).toContain("# give your idea a shot");
    expect(body).not.toContain("bun run tohseno");
    expect(body).not.toContain('class="mobile-home"');
    expect(body).not.toContain("shot-field");
    expect(body).not.toContain("/shot-icons/");
    expect(body).not.toContain('rel="manifest"');
    expect(body).not.toContain('href="/intake"');
    expect(body).not.toMatch(/\b(?:revolutionary|unleash|empower)\b/iu);
    expect(body).not.toMatch(/v0\.\d|0\.7|0\.6/);
    expect(body).toContain('href="/whitepaper.pdf"');
    expect(body).toContain('href="/docs"');
    expect(body).toContain('href="/privacy"');
    expect(body).toContain('src="/logo.svg"');
    expect(body).toContain(">Community</a>");
    expect(body).toContain('href="https://community.tohseno.com"');
    expect(body).toContain('target="_blank"');
    expect(body).toContain('rel="noopener noreferrer"');
    expect(body).toContain("<title>Tohseno — Give Every Idea a Shot</title>");
    expect(body).toContain(
      'content="An open-source factory for independent iOS apps — native shots you can run, use, and own in Simulator."',
    );
    expect(body).toMatch(
      /property="og:image" content="http:\/\/localhost:3000\/og\.png\?v=[0-9a-f]{8}"/,
    );
    expect(body).toContain('name="twitter:card" content="summary_large_image"');
    expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
    expect(response.headers.get("Content-Security-Policy")).toContain(
      "default-src 'self'",
    );
  });

  test("serves current factory docs and privacy", async () => {
    const application = await testApplication();
    for (const path of ["/docs", "/privacy"]) {
      const response = await application.fetch(request(path));
      expect(response.status).toBe(200);
      const body = await response.text();
      expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
      expect(body).not.toContain('href="/intake"');
      // Only the latest release is ever named; retired versions stay unnamed.
      expect(body).not.toMatch(/v0\.\d|0\.7|0\.6/);
      if (path === "/docs") {
        expect(body).toContain("Take another one");
        expect(body).toContain("Studio checks the Mac first");
        expect(body).toContain("Take one deliberately small Shot");
        expect(body).toContain("Experience version 0001, then evolve it");
        expect(body).toContain("tohseno studio");
        expect(body).toContain("TOHSENO 0.8.1");
        expect(body).toContain(
          "binds only to <code>127.0.0.1</code>",
        );
        expect(body).toContain("tohseno shot follow &lt;execution-id&gt;");
        expect(body).toContain("tohseno shot result &lt;execution-id&gt;");
        expect(body).toContain("<strong>SHOT LANDED</strong>");
        expect(body).toContain("tohseno migrate-legacy");
        expect(body).toContain("<code>~/Desktop/Tohseno</code>");
        expect(body).toContain(
          "Studio does not upload Shot data to TOHSENO",
        );
        expect(body).toContain(
          "TOHSENO never bypasses this human boundary",
        );
        expect(body).toContain("Robinhood Chain mainnet (chain ID 4663)");
        expect(body).toContain("inactive, untrusted candidate");
        expect(body).toContain("0xb1bd208cd2af98e701f43d06aaa889d3a594df65");
        expect(body).toContain("0x3fe6508ba2660bc575080024f402c192a2e035a0");
        expect(body).toContain("activation is not authorized");
        expect(body).not.toContain(
          "No TOHSENO contract is deployed on any network",
        );
        expect(body).toContain(INSTALL_COMMAND);
        expect(body).not.toContain("prepared, unpublished");
        expect(body).not.toContain("bun run tohseno");
      } else {
        expect(body).toContain("The installed factory sends no TOHSENO telemetry");
        expect(body).toContain(
          "requires a private local browser session for every Shot read or mutation",
        );
        expect(body).toContain("Every app builds and runs without TOHSENO credentials");
      }
    }
  });

  test("landing navigation and progressive controls target real content", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/"));
    const body = await response.text();
    const ids = new Set(
      [...body.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]),
    );
    const internalLinks = [...body.matchAll(/\shref="(\/[^"]*|#[^"]+)"/g)].map(
      (match) => match[1]!,
    );
    for (const link of internalLinks) {
      if (link.startsWith("#")) {
        expect(ids.has(link.slice(1))).toBe(true);
        continue;
      }
      const path = new URL(link, "http://localhost:3000").pathname;
      const target = await application.fetch(request(path));
      expect(target.status).toBe(200);
    }
    expect(body).not.toContain('href="#"');
    for (const commandId of [
      "command-install",
      "command-create",
      "command-studio",
    ]) {
      expect(body).toContain(`id="${commandId}"`);
      expect(body).toContain(`aria-describedby="${commandId}"`);
    }

    const browserScript = readFileSync(browserScriptPath, "utf8");
    expect(browserScript).toContain("navigator.clipboard.writeText(");
    expect(browserScript).toContain('querySelector("[data-copy-value]")');
    expect(browserScript).toContain("prefers-reduced-motion");
    expect(browserScript).toContain('querySelectorAll("[data-term-step]")');
    expect(browserScript).toContain("registration.unregister()");
    expect(browserScript).not.toContain("serviceWorker.register");

    const landingStyle = readFileSync(landingStylePath, "utf8");
    expect(landingStyle).toContain("@keyframes cursor-blink");
    expect(landingStyle).toMatch(
      /\[data-typing\][\s\S]*?visibility:\s*hidden;/,
    );
    expect(landingStyle).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*\.term-cursor\s*\{[^}]*animation:\s*none;/,
    );
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
      ["/manifest.webmanifest", "application/manifest+json"],
      ["/sw.js", "text/javascript"],
      ["/robots.txt", "text/plain"],
      ["/og.png", "image/png"],
      ["/favicon.png", "image/png"],
      ["/logo.svg", "image/svg+xml"],
      ["/whitepaper.pdf", "application/pdf"],
      ["/shot-icons/shot-001.webp", "image/webp"],
      ["/shot-icons/shot-100.webp", "image/webp"],
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
    const favicon = readFileSync(faviconPath);
    expect(favicon.subarray(1, 4).toString("ascii")).toBe("PNG");
    expect(favicon.readUInt32BE(16)).toBe(192);
    expect(favicon.readUInt32BE(20)).toBe(192);
  });

  test("ships exactly 100 optimized shot icons", () => {
    const shotIcons = readdirSync(shotIconDirectory)
      .filter((file) => /^shot-\d{3}\.webp$/.test(file))
      .sort();
    expect(shotIcons).toHaveLength(100);
    expect(shotIcons[0]).toBe("shot-001.webp");
    expect(shotIcons.at(-1)).toBe("shot-100.webp");
    for (const icon of shotIcons) {
      const bytes = readFileSync(join(shotIconDirectory, icon));
      expect(bytes.subarray(0, 4).toString("ascii")).toBe("RIFF");
      expect(bytes.subarray(8, 12).toString("ascii")).toBe("WEBP");
      expect(bytes.subarray(12, 16).toString("ascii")).toBe("VP8 ");
      expect(bytes.readUInt16LE(26) & 0x3fff).toBe(192);
      expect(bytes.readUInt16LE(28) & 0x3fff).toBe(192);
      expect(bytes.byteLength).toBeLessThan(32_000);
    }
  });

  test("serves both installer paths byte-for-byte", async () => {
    const application = await testApplication();
    const expected = readFileSync(installerPath);
    const response = await application.fetch(request("/install.sh"));
    expect(response.status).toBe(200);
    expect(response.headers.get("Content-Type")).toBe(
      "text/x-shellscript; charset=utf-8",
    );
    expect(response.headers.get("Cache-Control")).toBe(
      "public, max-age=0, must-revalidate",
    );
    expect(Buffer.from(await response.arrayBuffer())).toEqual(expected);

    const expectedOneshot = readFileSync(oneshotInstallerPath);
    expect(expected).toEqual(expectedOneshot);
    expect(expectedOneshot.byteLength).toBeGreaterThan(10_000);
    expect(expectedOneshot.toString("utf8")).toContain('version="v0.8.1"');
    expect(expectedOneshot.toString("utf8")).toContain(
      "releases/download/$version/$artifact",
    );
    const oneshotResponse = await application.fetch(request("/oneshot.sh"));
    expect(oneshotResponse.status).toBe(200);
    expect(oneshotResponse.headers.get("Content-Type")).toBe(
      "text/x-shellscript; charset=utf-8",
    );
    expect(oneshotResponse.headers.get("Cache-Control")).toBe(
      "public, max-age=0, must-revalidate",
    );
    expect(Buffer.from(await oneshotResponse.arrayBuffer())).toEqual(
      expectedOneshot,
    );
  });

  test("serves installer HEAD metadata without installer bytes", async () => {
    const application = await testApplication();
    const response = await application.fetch(
      request("/install.sh", { method: "HEAD" }),
    );
    expect(response.status).toBe(200);
    expect(response.headers.get("Content-Type")).toBe(
      "text/x-shellscript; charset=utf-8",
    );
    expect(response.headers.get("Cache-Control")).toBe(
      "public, max-age=0, must-revalidate",
    );
    expect((await response.arrayBuffer()).byteLength).toBe(0);
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

  test("access logs use semantic routes and never retain arbitrary paths", async () => {
    const records: Array<Record<string, unknown>> = [];
    const application = await createApplication({
      config: loadConfig({
        NODE_ENV: "test",
        PORT: "3000",
        BASE_URL: "http://localhost:3000",
      }),
      log: (record) => records.push(record),
      logError: (record) => records.push(record),
    });

    const response = await application.fetch(
      request("/credential-looking-path-value"),
    );

    expect(response.status).toBe(404);
    expect(records).toHaveLength(1);
    expect(records[0]).toMatchObject({
      event: "request",
      method: "GET",
      route: "unmatched",
      status: 404,
    });
    expect(JSON.stringify(records)).not.toContain(
      "credential-looking-path-value",
    );
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
    expect(response.headers.get("Location")).toBe(
      "http://tohseno.com:3000/docs?x=1",
    );
  });

  test("canonical redirects cannot be turned into protocol-relative redirects", async () => {
    const application = await createApplication({
      config: loadConfig({
        NODE_ENV: "test",
        PORT: "3000",
        BASE_URL: "http://tohseno.com:3000",
      }),
    });
    const response = await application.fetch(
      new Request("http://www.tohseno.com:3000//attacker.example/path"),
    );
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe(
      "http://tohseno.com:3000//attacker.example/path",
    );
  });
});
