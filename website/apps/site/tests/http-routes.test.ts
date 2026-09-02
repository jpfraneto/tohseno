import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
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

async function launchedApplication(): Promise<{
  application: TohsenoApplication;
  cleanup: () => void;
}> {
  const root = mkdtempSync(join(tmpdir(), "tohseno-network-launch."));
  const application = await createApplication({
    config: loadConfig({
      NODE_ENV: "test",
      PORT: "3000",
      BASE_URL: "http://localhost:3000",
      REGISTRY_ENABLED: "true",
      REGISTRY_ROOT: root,
      ROBINHOOD_RPC_URL: "https://rpc.mainnet.chain.robinhood.com",
      REGISTRY_RELAYER_ENABLED: "true",
      REGISTRY_RELAYER_PRIVATE_KEY: `0x${"01".repeat(32)}`,
      MACOS_DOWNLOAD_ENABLED: "true",
      MACOS_DOWNLOAD_URL: "https://downloads.tohseno.com/Tohseno-1.1.0.dmg",
      MACOS_DOWNLOAD_SHA256: "a".repeat(64),
    }),
  });
  return { application, cleanup: () => rmSync(root, { recursive: true, force: true }) };
}

async function candidateApplication(): Promise<TohsenoApplication> {
  return createApplication({
    config: loadConfig({
      NODE_ENV: "test",
      PORT: "3000",
      BASE_URL: "http://localhost:3000",
      MACOS_DOWNLOAD_ENABLED: "true",
      MACOS_DOWNLOAD_CHANNEL: "release-candidate",
      MACOS_DOWNLOAD_URL: "https://downloads.tohseno.com/Tohseno-1.2.0-rc.6.dmg",
      MACOS_DOWNLOAD_SHA256: "c".repeat(64),
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
  test("introduces the network while keeping every public path dark until launch", async () => {
    const application = await testApplication();
    const body = await (await application.fetch(request("/"))).text();
    expect(body).toContain("<h1>Software for Apple devices. <em>Distributed by anyone.</em></h1>");
    expect(body).toContain("Build on Mac. Publish to Tohseno.");
    expect(body).toContain("A link is the channel.<br><em>People are the network.</em>");
    expect(body).toContain("A new path between Apple customers.");
    expect(body).toContain("<strong>Build on Mac</strong>");
    expect(body).toContain("<strong>Publish a Shot</strong>");
    expect(body).toContain("<strong>Share one link</strong>");
    expect(body).toContain("<strong>Verify and install</strong>");
    expect(body).toContain("A friend of Apple.<br>A network for its customers.");
    expect(body).toContain("Apple’s curated marketplace.");
    expect(body).toContain("An open distribution network.");
    expect(body).toContain('<a href="#how-it-works">How it works</a>');
    expect(body).toContain('id="protocol"');
    expect(body).toContain("Every public release leaves a receipt.");
    expect(body).toContain("Builder authority");
    expect(body).toContain("Shot Registry");
    expect(body).toContain("Claim writes remain gated until the Claim release activates.");
    expect(body).toContain("Chain ID 4663");
    expect(body).toContain("macOS 14+");
    expect(body).toContain("Xcode");
    expect(body).toContain("iPhone");
    expect(body).toContain("Apple Account");
    expect(body).toContain("The Mac release and public network are being verified. Public downloads are still closed.");
    expect(body).not.toContain("Public downloads are open");
    expect(body).not.toContain('href="/registry"');
    expect(body).not.toContain('href="/download/macos"');
    expect(body).not.toContain("data-installer-download");
    expect(body).toContain("<footer");
    expect(body).not.toContain("hero-symbol");
    const registry = await (await application.fetch(request("/registry"))).text();
    expect(registry).toContain("Pre-launch verification.");
    expect(registry).toContain("No public app or write path is claimed.");
    expect(registry).not.toContain("The network is ready.");
  });

  test("serves the person-to-person landing page after launch", async () => {
    const launched = await launchedApplication();
    const { application } = launched;
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

    expect(body).toContain("<h1>Software for Apple devices. <em>Distributed by anyone.</em></h1>");
    expect(body).toContain("Build on Mac. Publish to Tohseno.");
    expect(body).toContain("A link is the channel.<br><em>People are the network.</em>");
    expect(body).toContain("Make software.<br>Move it through your circle.</h2>");
    expect(body).toContain("<strong>Build on Mac</strong>");
    expect(body).toContain("<strong>Publish a Shot</strong>");
    expect(body).toContain("<strong>Share one link</strong>");
    expect(body).toContain("<strong>Verify and install</strong>");
    expect(body).toContain("A friend of Apple.<br>A network for its customers.");
    expect(body).toContain("The App Store");
    expect(body).toContain("Apple’s curated marketplace.");
    expect(body).toContain("An open distribution network.");
    expect(body).toContain("macOS 14+");
    expect(body).toContain("Xcode");
    expect(body).toContain("iPhone");
    expect(body).toContain("Apple Account");
    expect(body).toContain('<a class="nav-action" href="/registry">Explore apps</a>');
    expect(body).toContain('id="protocol"');
    expect(body).toContain("Every public release leaves a receipt.");
    expect(body).toContain("Builder authority");
    expect(body).toContain("Shot Registry");
    expect(body).toContain("Claim writes remain gated until the Claim release activates.");
    expect(body).toContain("Chain ID 4663");
    expect(landingStyle.toString()).not.toContain("height: 100svh");
    expect(landingStyle.toString()).toContain("overflow-x: hidden;");
    expect(landingStyle.toString()).toContain("@media (max-width: 820px)");
    expect(body).toContain("<footer");
    expect(body).not.toContain("hero-symbol");
    expect(body).not.toContain("You are the moat");
    expect(body).not.toContain("npm i -g tohseno");
    expect(body).not.toContain(INSTALL_COMMAND);
    expect(body).not.toContain("curl -fsSL https://tohseno.com/install | sh");
    expect(body.match(/data-installer-download/g)).toHaveLength(1);
    expect(body.match(/href="\/download\/macos"/g)).toHaveLength(1);
    expect(body).toContain('data-download-channel="stable"');
    expect(body).toContain("Download for Mac");
    expect(body).toContain("macOS 14+");
    expect(body).not.toContain("paste into Terminal");
    expect(body).toContain('src="/landing-assets/wordmark.svg"');
    expect(body).toContain('<link rel="preload" href="/landing-assets/mascot.png" as="image" type="image/png">');
    expect(body).not.toContain('class="paint-intro"');
    expect(body).toContain('src="/landing.js"');
    expect(body).toContain('property="og:title" content="Software for Apple devices. Distributed by anyone."');
    expect(body).toContain('name="twitter:title" content="Software for Apple devices. Distributed by anyone."');
    expect(body).not.toContain("One App Per Day");
    expect(body).not.toContain("ticker-track");
    expect(body).not.toContain("data-copy-contract");
    expect(body).not.toContain("cal.com/jpfraneto/day");
    expect(body).not.toContain("sojourn");
    expect(body).not.toContain("BOOK A DAY");
    expect(body).not.toContain("Claim this");

    expect(body).not.toContain("dexscreener.com");
    expect(body).not.toContain("bun run tohseno");
    expect(body).not.toContain('href="/intake"');
    expect(body).not.toContain('href="#"');
    expect(body).not.toMatch(/\b(?:revolutionary|unleash|empower)\b/iu);
    expect(body).not.toMatch(/v0\.\d|0\.7|0\.6/);
    expect(body).not.toContain('href="/docs"');
    expect(body).toContain('href="/privacy"');
    expect((await application.fetch(request("/docs"))).status).toBe(308);
    expect((await application.fetch(request("/privacy"))).status).toBe(200);
    expect(body).toContain('rel="noopener noreferrer"');
    expect(body).toContain(
      "<title>Tohseno — Permissionless Distribution for Apple Software</title>",
    );
    expect(body).toContain(
      'content="Tohseno is the permissionless distribution network for Apple software. Build on Mac. Publish a verifiable release. Share it person to person."',
    );
    expect(body).toMatch(
      /property="og:image" content="http:\/\/localhost:3000\/og\.png\?v=[0-9a-f]{8}"/,
    );
    expect(body).toContain('name="twitter:card" content="summary_large_image"');
    expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
    expect(response.headers.get("Content-Security-Policy")).toContain(
      "default-src 'self'",
    );
    launched.cleanup();
  });

  test("makes the release candidate the homepage invitation while public writes stay dark", async () => {
    const application = await candidateApplication();
    const response = await application.fetch(request("/"));
    expect(response.status).toBe(200);
    const body = await response.text();

    expect(body).toContain('class="network-home is-candidate"');
    expect(body).toContain('data-download-channel="release-candidate"');
    expect(body).toContain("RC7 is signed and notarized");
    expect(body).toContain("<h1>Software for Apple devices. <em>Distributed by anyone.</em></h1>");
    expect(body).toContain("Permissionless distribution for Apple software");
    expect(body).toContain("Build on Mac. Publish to Tohseno.");
    expect(body).toContain("<strong>Build on Mac</strong>");
    expect(body).toContain("<strong>Publish a Shot</strong>");
    expect(body).toContain("<strong>Share one link</strong>");
    expect(body).toContain("<strong>Verify and install</strong>");
    expect(body).toContain("A friend of Apple.<br>A network for its customers.");
    expect(body).toContain("macOS 14+");
    expect(body).toContain("Xcode");
    expect(body).toContain("iPhone");
    expect(body).toContain("Apple Account");
    expect(body).toContain('id="protocol"');
    expect(body).toContain("Claim writes remain gated until the Claim release activates.");
    expect(body).toContain("Public network shipping remains closed");
    expect(body).toContain('href="/download/macos" data-installer-download');
    expect(body.match(/href="\/download\/macos"/g)).toHaveLength(1);
    expect(body).toContain("Download for Mac");
    expect(body).not.toContain("Public downloads are still closed");
    expect(body).not.toContain("$99");
    expect(body).not.toContain('href="/registry"');
    expect(body).toContain("<footer");
    expect(body).not.toContain("hero-symbol");
  });

  test("hands documentation to the standalone site and serves privacy", async () => {
    const application = await testApplication();
    const docs = await application.fetch(request("/docs"));
    expect(docs.status).toBe(308);
    expect(docs.headers.get("location")).toBe("https://docs.tohseno.com/");

    const privacy = await application.fetch(request("/privacy"));
    expect(privacy.status).toBe(200);
    const body = await privacy.text();
    expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
    expect(body).not.toContain('href="/intake"');
    expect(body).not.toMatch(/v0\.\d|0\.7|0\.6/);
    expect(body).toContain("The installed factory sends no Tohseno telemetry");
    expect(body).toContain("Native and managed intelligence");
    expect(body).toContain("Tohseno → Bankr → model-provider");
    expect(body).toContain("Pending Relay Intention");
    expect(body).toContain("never receives the decryption key");
    expect(body).toContain("at most seven days");
    expect(body).toContain("single-use bearer token");
    expect(body).toContain("private but <strong>not encrypted</strong>");
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
      // The independently governed purchase route is covered by buy.test.ts.
      if (path === "/buy") continue;
      const target = await application.fetch(request(path));
      expect(target.status).toBe(path === "/download/macos" ? 503 : 200);
    }
    expect(body).not.toContain('href="#"');
    const landingScript = readFileSync(landingScriptPath, "utf8");
    expect(landingScript).not.toContain("innerHTML");
    expect(landingScript).toContain('querySelectorAll("[data-installer-download]")');
    expect(landingScript).toContain('querySelectorAll("[data-download-title]")');
    expect(landingScript).toContain('querySelectorAll("[data-download-detail]")');
    expect(landingScript).toContain("navigator.userAgentData?.platform");
    expect(landingScript).toContain("navigator.maxTouchPoints > 1");
    expect(landingScript).toContain('title: "Download for this Mac"');
    expect(landingScript).toContain('release-candidate');
    expect(landingScript).toContain('"Release candidate · "');
    for (const system of ["iPhone", "Windows", "Android", "ChromeOS", "Linux"]) {
      expect(landingScript).toContain(system);
    }
    expect(landingScript).not.toContain("navigator.clipboard");
    expect(landingScript).not.toContain("execCommand");

    const landingStyle = readFileSync(landingStylePath, "utf8");
    expect(landingStyle).not.toContain("@font-face");
    expect(landingStyle).toContain("--cream: #f7f4ee");
    expect(landingStyle).toContain("--ink: #131313");
    expect(landingStyle).toContain("--orange: #f04a13");
    expect(landingStyle).toContain("var(--cream)");
    expect(landingStyle).toContain('font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display"');
    expect(landingStyle).toContain("@media (prefers-reduced-motion: reduce)");
    expect(landingStyle).not.toContain(".paint-intro");
    expect(landingStyle).not.toContain(".paint-mark-color");
    expect(landingStyle).toContain(".hero-visual");
    expect(landingStyle).toContain("@media (max-width: 1040px)");
    expect(landingStyle).toContain("@media (max-width: 640px)");
    expect(landingStyle).not.toContain(".shot-flow");
    expect(landingStyle).not.toContain(".hero-sequence");
    expect(landingStyle).not.toContain("height: 100svh");
    expect(landingStyle).toContain("overflow-x: hidden");
    expect(landingStyle).toContain(".people-loop");
    expect(landingStyle).toContain(".step-grid");
    expect(landingStyle).toContain(".apple-contrast");
    expect(landingStyle).toContain(".protocol-grid");
    expect(landingStyle).not.toContain(".ticker-track");
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
      MACOS_DOWNLOAD_CHANNEL: "release-candidate",
      MACOS_DOWNLOAD_URL: "https://downloads.tohseno.com/Tohseno-1.1.0.dmg",
      MACOS_DOWNLOAD_SHA256: "a".repeat(64),
    }) });
    const response = await configured.fetch(request("/download/macos"));
    expect(response.status).toBe(307);
    expect(response.headers.get("location")).toBe("https://downloads.tohseno.com/Tohseno-1.1.0.dmg");
    expect(response.headers.get("x-tohseno-sha256")).toBe("a".repeat(64));
    const metadata = await configured.fetch(request("/api/distribution/v1/macos"));
    expect(await metadata.json()).toEqual({ schema: "tohseno.macos-distribution/1", available: true,
      channel: "release-candidate",
      url: "https://downloads.tohseno.com/Tohseno-1.1.0.dmg", sha256: "a".repeat(64), minimum_macos_version: "14.0" });
    expect(response.headers.get("x-tohseno-release-channel")).toBe("release-candidate");
  });

  test("native one-line installer is consentful, verified, and fail-closed", async () => {
    const unavailable = await testApplication();
    for (const path of ["/install", "/download"]) {
      const response = await unavailable.fetch(request(path));
      expect(response.status).toBe(503);
      expect(response.headers.get("cache-control")).toBe("no-store");
      expect(response.headers.get("x-tohseno-install-status")).toBe("not-published");
      expect(await response.json()).toEqual({
        error: "The signed and notarized Mac installer is not published yet.",
      });
    }

    const sha256 = "b".repeat(64);
    const downloadURL = "https://downloads.tohseno.com/Tohseno-1.1.0-rc.2.dmg";
    const configured = await createApplication({ config: loadConfig({
      NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
      MACOS_DOWNLOAD_ENABLED: "true",
      MACOS_DOWNLOAD_URL: downloadURL,
      MACOS_DOWNLOAD_SHA256: sha256,
    }) });
    const install = await configured.fetch(request("/install"));
    const alias = await configured.fetch(request("/download"));
    expect(install.status).toBe(200);
    expect(install.headers.get("content-type")).toBe("text/x-shellscript; charset=utf-8");
    expect(install.headers.get("cache-control")).toBe("no-store");
    expect(install.headers.get("x-tohseno-install-command")).toBe(
      "curl -fsSL https://tohseno.com/install | sh",
    );
    const body = await install.text();
    expect(await alias.text()).toBe(body);
    expect(body).toStartWith("#!/bin/sh\nset -eu\n");
    expect(body).toContain(`dmg_url='${downloadURL}'`);
    expect(body).toContain(`dmg_sha256='${sha256}'`);
    expect(body).toContain("You will install the Tohseno installer.");
    expect(body).toContain("Enter to continue. Esc to exit.");
    expect(body).toContain("Nothing was installed.");
    expect(body).toContain("< /dev/tty");
    expect(body).toContain("/bin/stty -echo -icanon min 1 time 0");
    expect(body).toContain("/bin/dd bs=1 count=1");
    expect(body).toContain("--progress-bar");
    expect(body).toContain("/usr/bin/shasum -a 256");
    expect(body).toContain("/usr/bin/codesign --verify --deep --strict");
    expect(body).toContain("expected_team_id='84V63LKV45'");
    expect(body).toContain("expected_bundle_id='com.tohseno.mac'");
    expect(body).toContain("/usr/sbin/spctl --assess --type execute");
    expect(body).toContain("/usr/bin/hdiutil attach");
    expect(body).toContain("downloads_dir=\"$HOME/Downloads\"");
    expect(body).toContain("download_name='Tohseno-1.1.0-rc.2.dmg'");
    expect(body).toContain("Double-click it, then drag Tohseno into Applications.");
    expect(body).toContain("/usr/bin/open -R \"$destination\"");
    expect(body).not.toContain("/usr/bin/ditto");
    expect(body).not.toContain("target_app=");
    expect(body).not.toMatch(/\bsudo\b/);
    expect(body).not.toContain(".zshrc");

    const head = await configured.fetch(request("/download", { method: "HEAD" }));
    expect(head.status).toBe(200);
    expect(await head.text()).toBe("");
    expect(head.headers.get("x-tohseno-install-command")).toContain("/install | sh");
    const rejected = await configured.fetch(request("/install", { method: "POST" }));
    expect(rejected.status).toBe(405);
    expect(rejected.headers.get("allow")).toBe("GET, HEAD");
  });

  test("serves static assets with correct content types", async () => {
    const application = await testApplication();
    const expectations: Array<[string, string]> = [
      ["/styles.css", "text/css"],
      ["/landing.css", "text/css"],
      ["/landing.js", "text/javascript"],
      ["/buy.css", "text/css"],
      ["/buy.js", "text/javascript"],
      ["/fonts/fraunces-latin.woff2", "font/woff2"],
      ["/fonts/plex-mono-latin.woff2", "font/woff2"],
      ["/app.js", "text/javascript"],
      ["/modules/intent-package.js", "text/javascript"],
      ["/manifest.webmanifest", "application/manifest+json"],
      ["/sw.js", "text/javascript"],
      ["/releases/native-v1.json", "application/json"],
      ["/robots.txt", "text/plain"],
      ["/og.png", "image/png"],
      ["/og-buy.png", "image/png"],
      ["/favicon.png", "image/png"],
      ["/tohseno-logo.png", "image/png"],
      ["/landing-assets/wordmark.svg", "image/svg+xml"],
      ["/landing-assets/mascot.png", "image/png"],
      ["/landing-assets/network.png", "image/png"],
      ["/landing-assets/build-mac.svg", "image/svg+xml"],
      ["/landing-assets/publish.svg", "image/svg+xml"],
      ["/landing-assets/share.svg", "image/svg+xml"],
      ["/landing-assets/verify-install.svg", "image/svg+xml"],
      ["/landing-assets/builder-authority.svg", "image/svg+xml"],
      ["/landing-assets/shot-registry.svg", "image/svg+xml"],
      ["/landing-assets/claims.svg", "image/svg+xml"],
      ["/landing-assets/mac.svg", "image/svg+xml"],
      ["/landing-assets/xcode.svg", "image/svg+xml"],
      ["/landing-assets/iphone.svg", "image/svg+xml"],
      ["/landing-assets/apple-account.svg", "image/svg+xml"],
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
    expect(expectedOneshot.toString("utf8")).toContain('version="v1.1.0"');
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
      route: "public-registry",
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
