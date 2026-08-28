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
const nativeReleaseManifestPath = fileURLToPath(
  new URL("../public/releases/native-v1.json", import.meta.url),
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
const landingScriptPath = fileURLToPath(
  new URL("../public/landing.js", import.meta.url),
);
const landingStylePath = fileURLToPath(
  new URL("../public/landing.css", import.meta.url),
);
const INSTALL_COMMAND = "curl -fsSL https://tohseno.com/oneshot.sh | bash";

describe("public pages", () => {
  test("serves the brutalist landing page", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/"));
    expect(response.status).toBe(200);
    const body = await response.text();
    const landingStyle = readFileSync(landingStylePath);
    const landingStyleRevision = createHash("sha256")
      .update(landingStyle)
      .digest("hex")
      .slice(0, 12);
    // The revision is derived from the stylesheet itself, so a style change
    // can never ship behind a stale cached copy.
    expect(body).toContain(`/landing.css?v=${landingStyleRevision}`);

    expect(body).toContain("MAKE THE<br>iPHONE APP<br><em>THAT SHOULD EXIST.</em>");
    expect(body).toContain("One coherent intention becomes a native application");
    expect(body).toContain("YOUR IDEA DOES NOT NEED TO BECOME A STARTUP");
    expect(body).toContain("ONE INTENTION.<br>ONE REAL ATTEMPT.");
    expect(body).toContain("MAKE SOFTWARE<br>FOR YOUR ACTUAL LIFE.");
    expect(body).toContain("THE FACTORY IS<br>A MIDWIFE.");
    expect(body).toContain("THE FACTORY<br>IS OPEN.<br>THE APP<br>IS YOURS.");
    expect(body).toContain("FROM ZERO<br>TO YOUR PHONE.");
    expect(body).toContain("ONE MAC APP. CLEAR CHOICES.");
    expect(body).toContain("LOCAL CREATION.<br>PUBLICATION IS NEXT.");
    expect(body).toContain("WHAT SHOULD<br>EXIST?");
    expect(body).not.toContain("npm i -g tohseno");
    expect(body).not.toContain(INSTALL_COMMAND);
    expect(body.match(/href="\/download\/macos"/g)).toHaveLength(3);
    expect(body).toContain('src="/app-breathekeeper.png"');
    expect(body).toContain('src="/app-room-tone.png"');
    expect(body).not.toContain('src="/landing.js"');
    expect(body).toContain('property="og:title" content="Open Source iOS Apps Factory"');
    expect(body).toContain('name="twitter:title" content="Open Source iOS Apps Factory"');
    expect(body).not.toContain("One App Per Day");
    expect(body.match(/class="ticker-track"/g)).toHaveLength(1);
    expect(body).toContain("REALITY IS NOW CHEAP ENOUGH TO ANSWER.");
    expect(body).not.toContain("data-copy-contract");
    expect(body).not.toContain("cal.com/jpfraneto/day");
    expect(body).toContain("prepaid balance");
    expect(body).not.toContain("sojourn");
    expect(body).not.toContain("BOOK A DAY");

    expect(body).not.toContain("dexscreener.com");
    expect(body).toContain("source hosting, publication, discovery, and forking are not available yet");
    expect(body).not.toContain("bun run tohseno");
    expect(body).not.toContain('href="/intake"');
    expect(body).not.toContain('href="#"');
    expect(body).not.toMatch(/\b(?:revolutionary|unleash|empower)\b/iu);
    expect(body).not.toMatch(/v0\.\d|0\.7|0\.6/);
    expect(body).not.toContain('href="/docs"');
    expect(body).not.toContain('href="/privacy"');
    for (const path of ["/docs", "/privacy"]) {
      expect((await application.fetch(request(path))).status).toBe(200);
    }
    expect(body).toContain("COMMUNITY <span");
    expect(body).toContain('href="https://community.tohseno.com"');
    expect(body).toContain('rel="noopener noreferrer"');
    expect(body).toContain('<a href="#open-source">OPEN SOURCE</a>');
    expect(body).toContain('id="open-source"');
    expect(body).toContain("TOHSENO is open source.");
    expect(body).toContain(
      '>VIEW SOURCE ON GITHUB <span aria-hidden="true">↗</span></a>',
    );
    const primaryNavigation = body.match(
      /<nav aria-label="Primary navigation">[\s\S]*?<\/nav>/,
    )?.[0];
    expect(primaryNavigation).toBeDefined();
    expect(primaryNavigation).not.toContain("github.com");
    expect(body).toContain(
      "<title>TOHSENO — Make the iPhone App That Should Exist</title>",
    );
    expect(body).toContain(
      'content="Describe a small app. Your Mac builds it and puts it on your iPhone."',
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
        expect(body).toContain("App → Intent → App on your iPhone");
        expect(body).toContain("One readiness instruction at a time");
        expect(body).toContain("Create one deliberately small app");
        expect(body).toContain("Use it, then describe the change");
        expect(body).toContain("Download TOHSENO for Mac");
        expect(body).toContain("macOS 14 or newer");
        expect(body).toContain("Managed intelligence is optional");
        expect(body).toContain(
          "loopback Local Workspace Service",
        );
        expect(body).not.toContain("npm i -g tohseno");
        expect(body).not.toContain("$9.99");
        expect(body).toContain("<code>~/Desktop/Tohseno</code>");
        expect(body).toContain(
          "does not upload canonical Shot data",
        );
        expect(body).toContain("not blanket-gitignored");
        expect(body).toContain("<code>.tohseno/private</code>");
        expect(body).toContain("Robinhood Chain mainnet (chain ID 4663)");
        expect(body).toContain("current client-trusted generation");
        expect(body).toContain("0xb1bd208cd2af98e701f43d06aaa889d3a594df65");
        expect(body).toContain("0x3fe6508ba2660bc575080024f402c192a2e035a0");
        expect(body).toContain("source hosting, catalog discovery, and download are not implemented");
        expect(body).toContain("No current TOHSENO workflow uploads an app repository");
        expect(body).not.toContain(INSTALL_COMMAND);
        expect(body).not.toContain("inactive, untrusted candidate");
        expect(body).not.toContain("native <strong>TOHSENO 1.0.1</strong>");
      } else {
        expect(body).toContain("The installed factory sends no TOHSENO telemetry");
        expect(body).toContain("Native and managed intelligence");
        expect(body).toContain("TOHSENO → Bankr → model-provider");
        expect(body).toContain("Pending Relay Intention");
        expect(body).toContain("never receives the decryption key");
        expect(body).toContain("at most seven days");
        expect(body).toContain("single-use bearer token");
        expect(body).toContain("private but <strong>not encrypted</strong>");
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
      expect(target.status).toBe(path === "/download/macos" ? 503 : 200);
    }
    expect(body).not.toContain('href="#"');
    const landingScript = readFileSync(landingScriptPath, "utf8");
    expect(landingScript).not.toContain("innerHTML");

    const landingStyle = readFileSync(landingStylePath, "utf8");
    expect(landingStyle).toContain("@media (prefers-reduced-motion: reduce)");
    expect(landingStyle).toContain(".shot-flow");
    expect(landingStyle).toContain(".shot-grid");
    expect(landingStyle).toContain(".ownership-grid");
    expect(landingStyle).toContain(".open-source-grid");
    expect(landingStyle).toMatch(
      /\.nav nav a \{[\s\S]*?font: 700 14px ui-monospace, monospace;/,
    );
    expect(landingStyle).toContain("@keyframes ticker-scroll");
    expect(landingStyle).toContain("@keyframes ticker-shake");
    expect(landingStyle).toContain("translateX(-50%)");
    expect(landingStyle).not.toContain(".tiers");
    expect(landingStyle).toContain("@media (max-width: 500px)");
  });

  test("serves the health check", async () => {
    const application = await testApplication();
    const response = await application.fetch(request("/healthz"));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ status: "ok", service: "tohseno" });
  });

  test("Mac download stays fail-closed until a notarized digest is configured", async () => {
    const unavailable = await testApplication();
    expect((await unavailable.fetch(request("/download/macos"))).status).toBe(503);
    const configured = await createApplication({ config: loadConfig({
      NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
      MACOS_DOWNLOAD_ENABLED: "true",
      MACOS_DOWNLOAD_URL: "https://downloads.tohseno.com/Tohseno-1.0.2.dmg",
      MACOS_DOWNLOAD_SHA256: "a".repeat(64),
    }) });
    const response = await configured.fetch(request("/download/macos"));
    expect(response.status).toBe(307);
    expect(response.headers.get("location")).toBe("https://downloads.tohseno.com/Tohseno-1.0.2.dmg");
    expect(response.headers.get("x-tohseno-sha256")).toBe("a".repeat(64));
    const metadata = await configured.fetch(request("/api/distribution/v1/macos"));
    expect(await metadata.json()).toEqual({ schema: "tohseno.macos-distribution/1", available: true,
      url: "https://downloads.tohseno.com/Tohseno-1.0.2.dmg", sha256: "a".repeat(64), minimum_macos_version: "14.0" });
  });

  test("serves static assets with correct content types", async () => {
    const application = await testApplication();
    const expectations: Array<[string, string]> = [
      ["/styles.css", "text/css"],
      ["/landing.css", "text/css"],
      ["/landing.js", "text/javascript"],
      ["/fonts/fraunces-latin.woff2", "font/woff2"],
      ["/fonts/plex-mono-latin.woff2", "font/woff2"],
      ["/app.js", "text/javascript"],
      ["/modules/intent-package.js", "text/javascript"],
      ["/manifest.webmanifest", "application/manifest+json"],
      ["/sw.js", "text/javascript"],
      ["/releases/native-v1.json", "application/json"],
      ["/robots.txt", "text/plain"],
      ["/og.png", "image/png"],
      ["/favicon.png", "image/png"],
      ["/tohseno-logo.png", "image/png"],
      ["/app-breathekeeper.png", "image/png"],
      ["/app-who-ate.png", "image/png"],
      ["/app-handoff.png", "image/png"],
      ["/app-water-walk.png", "image/png"],
      ["/app-ink-memory.png", "image/png"],
      ["/app-room-tone.png", "image/png"],
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
    expect(expectedOneshot.toString("utf8")).toContain('version="v1.0.2"');
    expect(expectedOneshot.toString("utf8")).toContain("--claim)");
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

  test("serves the verified native release manifest", async () => {
    const application = await testApplication();
    const expected = JSON.parse(readFileSync(nativeReleaseManifestPath, "utf8"));
    const response = await application.fetch(request("/releases/native-v1.json"));
    expect(response.status).toBe(200);
    expect(response.headers.get("Content-Type")).toBe(
      "application/json; charset=utf-8",
    );
    expect(response.headers.get("Cache-Control")).toBe(
      "public, max-age=0, must-revalidate",
    );
    expect(await response.json()).toEqual(expected);
    expect(expected.schema).toBe("tohseno.native-release-manifest/1");
    expect(expected.native_release_version).toBe("1.0.0");
    expect(expected.minimum_npm_cli_version).toBe("1.0.0");
    expect(expected.artifacts).toHaveLength(2);
    for (const artifact of expected.artifacts) {
      expect(artifact.signing.kind).toBe("apple-developer-id");
      expect(artifact.signing.team_id).toBe("84V63LKV45");
      expect(artifact.signing.designated_requirement).toContain(
        'certificate leaf[subject.OU] = "84V63LKV45"',
      );
    }
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
