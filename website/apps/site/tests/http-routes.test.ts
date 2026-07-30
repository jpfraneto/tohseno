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
  test("serves the canonical installer path and no stale intake surface", async () => {
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
    expect(body).toContain(INSTALL_COMMAND);
    expect(body).not.toContain("$ </span>tohseno");
    expect(body).toContain(
      `data-copy-value="${INSTALL_COMMAND}"`,
    );
    expect(body).toContain("Copy installer");
    expect(body).not.toContain("bun run tohseno");
    expect(body).toContain("GIVE EVERY");
    expect(body).toContain("IDEA A");
    expect(body).toContain("The fastest way to prototype iOS apps");
    expect(body).toContain(
      "An open-source factory for independent iOS apps, for builders with more ideas than time.",
    );
    expect(body).toContain(
      "Get rid of your recurring thoughts by turning them into an app you can run, use, and judge in iPhone Simulator.",
    );
    expect(body).toContain("INFINITE SHOTS.");
    expect(body).toContain("Some shots live.");
    expect(body).toContain("Every shot begins from a working SwiftUI base");
    expect(body).toContain(
      "Every coding-agent exit is followed by privacy and integrity verification",
    );
    expect(body).toContain("STOP PROTECTING");
    expect(body).toContain("TAKE ANOTHER ONE.");
    expect(body).toContain(">ONE SHOT</span>");
    expect(body).toContain("100 EXAMPLES / ∞ SHOTS");
    const shotField = body.match(/<ol class="shot-field"[\s\S]*?<\/ol>/)?.[0];
    expect(shotField).toBeDefined();
    const shotIcons = new Set(
      [...(shotField?.matchAll(/\/shot-icons\/shot-(\d{3})\.webp/g) ?? [])].map(
        (match) => match[1],
      ),
    );
    expect(shotIcons.size).toBe(100);
    expect(body).not.toMatch(/\b(?:revolutionary|unleash|empower)\b/iu);
    expect(body).not.toContain("four years");
    expect(body).toContain('href="/whitepaper.pdf"');
    expect(body).toContain('src="/logo.svg"');
    expect(body).toContain(">Community</a>");
    expect(body).toContain('href="https://community.tohseno.com"');
    expect(body).toContain('target="_blank"');
    expect(body).toContain('rel="noopener noreferrer"');
    expect(body).not.toContain("YOUR WEIRDNESS IS NOW EXECUTABLE.");
    expect(body).not.toContain("Keep up to 100 ideas");
    expect(body).not.toContain("UP TO");
    expect(body).not.toContain("slot machine");
    expect(body).not.toContain('href="/intake"');
    expect(body).not.toContain("Managed intake");
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
      if (path === "/docs") {
        expect(body).toContain("Take another one");
        expect(body).toContain("Studio checks the Mac first");
        expect(body).toContain("Take one deliberately small Shot");
        expect(body).toContain("Experience version 0001, then evolve it");
        expect(body).toContain("tohseno studio");
        expect(body).toContain("TOHSENO 0.7.1");
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
        expect(body).toContain(INSTALL_COMMAND);
        expect(body).not.toContain("prepared, unpublished");
        expect(body).not.toContain("bun run tohseno");
      } else {
        expect(body).toContain("The installed 0.7 factory sends no TOHSENO telemetry");
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
    expect(body).toContain('aria-controls="shot-field"');
    expect(body).toContain('aria-describedby="hero-command"');

    const browserScript = readFileSync(browserScriptPath, "utf8");
    expect(browserScript).toContain("navigator.clipboard.writeText(copyValue)");
    expect(browserScript).toContain('querySelector("[data-install-command]")');
    expect(browserScript).toContain('shotToggle.setAttribute("aria-expanded"');

    const landingStyle = readFileSync(landingStylePath, "utf8");
    expect(landingStyle).toMatch(/\.shot-tile\s*\{[^}]*aspect-ratio:\s*1;/s);
    expect(landingStyle).toMatch(
      /\.proof-grid > span\s*\{[^}]*aspect-ratio:\s*1;/s,
    );
    for (const selector of [
      String.raw`\.hero-icon`,
      String.raw`\.shot-tile`,
      String.raw`\.proof-grid > span`,
    ]) {
      expect(landingStyle).toMatch(
        new RegExp(
          `${selector}\\s*\\{[^}]*aspect-ratio:\\s*1;[^}]*min-height:\\s*0;[^}]*align-self:\\s*start;`,
          "s",
        ),
      );
    }
    for (const selector of [
      String.raw`\.hero-icon-grid`,
      String.raw`\.shot-field`,
      String.raw`\.proof-grid`,
    ]) {
      expect(landingStyle).toMatch(
        new RegExp(`${selector}\\s*\\{[^}]*align-items:\\s*start;`, "s"),
      );
    }
    for (const selector of [
      String.raw`\.hero-icon img`,
      String.raw`\.shot-tile img`,
      String.raw`\.icon-constellation img`,
      String.raw`\.proof-grid img`,
    ]) {
      expect(landingStyle).toMatch(
        new RegExp(
          `${selector}\\s*\\{[^}]*height:\\s*auto;[^}]*aspect-ratio:\\s*1\\s*\\/\\s*1;`,
          "s",
        ),
      );
    }
    expect(landingStyle).not.toMatch(
      /\.(?:hero-icon|shot-tile|proof-grid)[^{]*img\s*\{[^}]*height:\s*100%;/s,
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
    expect(expectedOneshot.toString("utf8")).toContain('version="v0.7.1"');
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
