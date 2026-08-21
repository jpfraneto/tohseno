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
  test("serves the terminal landing page", async () => {
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

    // The prompt is the page. Its placeholder is the first instruction a
    // person reads, and it is the exact command they will run on their Mac.
    expect(body).toContain('placeholder="tohseno create my-app-name"');
    expect(body).toContain('id="terminal-input"');
    expect(body).toContain('id="term-stream"');
    expect(body).toContain('id="drop-veil"');
    expect(body).toContain(`data-install-command="${INSTALL_COMMAND}"`);

    // Everything above the prompt was removed: with JavaScript the page is a
    // prompt and nothing else. The offer still has to survive in the markup
    // for a crawler and for a reader without JavaScript, so it lives in the
    // noscript block, which is the only place it was ever read from.
    expect(body).toContain(
      "Describe an app. It gets built, and installed on your iPhone.",
    );
    expect(body).toContain("Free and open source.");
    expect(body).toContain("Codex or Claude Code");
    expect(body).toContain(INSTALL_COMMAND);
    expect(body).toContain("<noscript>");
    const noscript = body.slice(
      body.indexOf("<noscript>"),
      body.indexOf("</noscript>"),
    );
    for (const copy of [
      "Describe an app. It gets built, and installed on your iPhone.",
      "Free and open source.",
      "Codex or Claude Code",
    ]) {
      expect(noscript).toContain(copy);
    }
    expect(body).not.toContain('class="boot"');
    // The prompt is the first thing in the terminal, with an empty stream
    // above it rather than a paragraph of copy.
    expect(body).toContain('<div class="term-stream" id="term-stream"></div>');
    expect(body).not.toContain('id="term-send"');
    expect(body).not.toContain(">RUN</button>");
    // The one line under the prompt is the only instruction left on the page.
    // The markup and the browser script must carry the same sentence, or it
    // visibly changes under the reader the moment the script runs.
    const hint =
      "Describe your app, attach images, and experience it on your phone. Type help for commands";
    expect(body).toContain(`<p class="term-hint" id="term-hint">${hint}</p>`);
    expect(readFileSync(browserScriptPath, "utf8")).toContain(`hint: "${hint}"`);
    expect(body).not.toContain("cal.com/jpfraneto/day");
    expect(body).toContain('<span class="beta">BETA</span>');

    // Prices and the booking offer were removed from the page deliberately.
    expect(body).not.toMatch(/\$\d/u);
    expect(body).not.toContain("sojourn");
    expect(body).not.toContain("BOOK A DAY");

    expect(body).toContain(
      'href="https://dexscreener.com/robinhood/0x364415f884fc93775a4c1825c1a3af1f0c2d8ba3"',
    );
    expect(body).toContain(">$TOHSENO</a>");
    expect(body).not.toContain("bun run tohseno");
    expect(body).not.toContain('href="/intake"');
    expect(body).not.toContain('href="#"');
    expect(body).not.toMatch(/\b(?:revolutionary|unleash|empower)\b/iu);
    expect(body).not.toMatch(/v0\.\d|0\.7|0\.6/);
    // Docs and privacy were removed from the status bar by request. The pages
    // stay published and stay reachable through the `docs` and `privacy`
    // commands, so their absence here must not become absence everywhere.
    expect(body).not.toContain('href="/docs"');
    expect(body).not.toContain('href="/privacy"');
    for (const path of ["/docs", "/privacy"]) {
      expect((await application.fetch(request(path))).status).toBe(200);
    }
    expect(body).toContain(">COMMUNITY</a>");
    expect(body).toContain('href="https://community.tohseno.com"');
    expect(body).toContain('rel="noopener noreferrer"');
    expect(body).toContain(
      "<title>TOHSENO — tohseno create my-app-name</title>",
    );
    expect(body).toContain(
      'content="An MVP factory for iOS apps. Describe an app, send the intent to your Mac, and install it on your iPhone. Free and open source."',
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
        expect(body).toContain("TOHSENO 0.8.5");
        expect(body).toContain(
          "binds only to <code>127.0.0.1</code>",
        );
        expect(body).toContain("tohseno shot follow &lt;execution-id&gt;");
        expect(body).toContain("tohseno shot result &lt;execution-id&gt;");
        expect(body).toContain("<strong>BIRTH ACCEPTED</strong>");
        expect(body).toContain("tohseno migrate-legacy");
        expect(body).toContain("<code>~/Desktop/Tohseno</code>");
        expect(body).toContain(
          "Studio does not upload canonical Shot data",
        );
        expect(body).toContain("<strong>TAKE THE SHOT</strong>");
        expect(body).toContain("continue unattended");
        // The interactive approve-and-press-Enter ceremony is retired.
        expect(body).not.toContain("APPROVE &amp; OPEN TERMINAL");
        expect(body).not.toContain("TOHSENO never bypasses this human boundary");
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
      expect(target.status).toBe(200);
    }
    expect(body).not.toContain('href="#"');
    const browserScript = readFileSync(browserScriptPath, "utf8");
    expect(browserScript).toContain("navigator.clipboard.writeText(");
    expect(browserScript).toContain("replaceChildren");
    // The terminal composes and encrypts in the browser and reaches the relay
    // only through the shared modules the Rust CLI is checked against.
    expect(browserScript).toContain("createEncryptedEnvelope(");
    expect(browserScript).toContain("./modules/terminal.js");
    expect(browserScript).toContain("./modules/relay-client.js");
    // Every stream line is built as a node with textContent, so nothing a
    // person types or drops can become markup.
    expect(browserScript).not.toContain("innerHTML");
    expect(browserScript).not.toContain("serviceWorker.register");
    // The phone keyboard opens from a real tap and the terminal gives back
    // the space it covers. Both halves have to stay.
    expect(browserScript).toContain("window.visualViewport");
    expect(browserScript).toContain("--keyboard");
    // The old pricing landing page and its dashboard-shaped controls are gone.
    expect(browserScript).not.toContain("renderQuiver");

    const landingStyle = readFileSync(landingStylePath, "utf8");
    expect(landingStyle).toContain("@media (prefers-reduced-motion: reduce)");
    expect(landingStyle).toContain(".term-screen");
    expect(landingStyle).not.toContain(".tiers");
    expect(landingStyle).toContain("var(--keyboard, 0px)");
    // The boot copy is gone from the page, and its styles with it.
    expect(landingStyle).not.toContain(".boot");
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
      ["/modules/intent-package.js", "text/javascript"],
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
    expect(expectedOneshot.toString("utf8")).toContain('version="v0.8.5"');
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
