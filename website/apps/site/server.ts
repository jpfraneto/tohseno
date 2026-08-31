import { join } from "node:path";
import type { AppConfig } from "./config.ts";
import { loadConfig, PRODUCT, safeStartupSummary } from "./config.ts";
import { HttpError, withSecurityHeaders } from "./src/security.ts";
import { INTENT_LIMITS } from "./src/intent-limits.ts";
import { createRelayRouter } from "./src/relay-routes.ts";
import { createBillingRouter } from "./src/billing.ts";
import { createManagedRouter } from "./src/managed.ts";
import { createRegistryRouter } from "./src/registry.ts";
import { createClaimsRouter } from "./src/claims.ts";
import type { RegistryRouter } from "./src/registry.ts";

const PUBLIC_DIRECTORY = join(import.meta.dir, "public");

export interface ApplicationOptions {
  config?: AppConfig;
  log?: (record: Record<string, unknown>) => void;
  logError?: (record: Record<string, unknown>) => void;
}

export interface TohsenoApplication {
  config: AppConfig;
  fetch(request: Request): Promise<Response>;
}

function json(data: unknown, status = 200): Response {
  return withSecurityHeaders(
    new Response(JSON.stringify(data), {
      status,
      headers: { "Content-Type": "application/json; charset=utf-8" },
    }),
  );
}

function html(content: string, status = 200): Response {
  return withSecurityHeaders(
    new Response(content, {
      status,
      headers: { "Content-Type": "text/html; charset=utf-8" },
    }),
  );
}

function htmlEscape(value: string): string {
  return value.replace(
    /[&<>"']/g,
    (character) =>
      ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;",
      })[character] ?? character,
  );
}

function renderTemplate(
  template: string,
  extra: Record<string, string> = {},
): string {
  const values: Record<string, string> = {
    ...PRODUCT.copy,
    INSTALL_COMMAND: PRODUCT.installCommand,
    REPOSITORY_URL: PRODUCT.repositoryUrl,
    ...extra,
  };
  const rendered = template.replace(
    /\{\{([A-Z0-9_]+)\}\}/g,
    (_match, key: string) => {
      const value = values[key];
      if (value === undefined)
        throw new Error(`Unknown template placeholder: ${key}`);
      return htmlEscape(value);
    },
  );
  if (/\{\{[A-Z0-9_]+\}\}/.test(rendered))
    throw new Error("Template contains unresolved placeholders");
  return rendered;
}

function headResponse(response: Response, method: string): Response {
  if (method !== "HEAD") return response;
  return new Response(null, {
    status: response.status,
    statusText: response.statusText,
    headers: response.headers,
  });
}

function methodNotAllowed(): Response {
  const response = json({ error: "Method not allowed" }, 405);
  const headers = new Headers(response.headers);
  headers.set("Allow", "GET, HEAD");
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

function shellSingleQuote(value: string): string {
  return `'${value.replaceAll("'", `'\\''`)}'`;
}

export function renderNativeMacInstaller(
  downloadURL: string,
  downloadSHA256: string,
): string {
  const artifactPathComponent = new URL(downloadURL).pathname.split("/").at(-1) ?? "";
  const downloadName = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}\.dmg$/.test(artifactPathComponent)
    ? artifactPathComponent
    : "Tohseno.dmg";
  const script = `#!/bin/sh
set -eu

dmg_url=__DMG_URL__
dmg_sha256=__DMG_SHA256__
download_name=__DOWNLOAD_NAME__
expected_team_id='84V63LKV45'
expected_bundle_id='com.tohseno.mac'
temp_dir=''
mount_point=''
tty_state=''

say() {
  /usr/bin/printf '%s\\n' "$*" > /dev/tty
}

die() {
  say "Tohseno was not installed: $*"
  exit 1
}

cleanup() {
  if [ -n "$tty_state" ]; then
    /bin/stty "$tty_state" < /dev/tty >/dev/null 2>&1 || true
    tty_state=''
  fi
  if [ -n "$mount_point" ] && /sbin/mount | /usr/bin/grep -F " on $mount_point " >/dev/null 2>&1; then
    /usr/bin/hdiutil detach "$mount_point" -quiet >/dev/null 2>&1 || true
  fi
  if [ -n "$temp_dir" ] && [ -d "$temp_dir" ]; then
    case "$temp_dir" in
      /tmp/tohseno-native-install.*) /bin/rm -rf "$temp_dir" ;;
    esac
  fi
}

trap cleanup EXIT
trap 'exit 130' HUP INT TERM

[ -r /dev/tty ] && [ -w /dev/tty ] || {
  /usr/bin/printf '%s\\n' 'Tohseno needs an interactive terminal for installation.' >&2
  exit 1
}

for tool in /usr/bin/curl /usr/bin/shasum /usr/bin/hdiutil /usr/bin/codesign /usr/sbin/spctl /usr/bin/open /usr/bin/sw_vers /usr/bin/uname /usr/bin/grep /usr/bin/sed /bin/dd /bin/stty /sbin/mount; do
  [ -x "$tool" ] || die "required macOS tool is missing: $tool"
done

[ "$(/usr/bin/uname -s)" = 'Darwin' ] || die 'this installer only runs on macOS.'
architecture=$(/usr/bin/uname -m)
case "$architecture" in
  arm64|x86_64) ;;
  *) die "unsupported Mac architecture: $architecture" ;;
esac
macos_version=$(/usr/bin/sw_vers -productVersion)
macos_major=$(/usr/bin/printf '%s\\n' "$macos_version" | /usr/bin/sed 's/\\..*$//')
case "$macos_major" in
  ''|*[!0-9]*) die 'the macOS version could not be read.' ;;
esac
[ "$macos_major" -ge 14 ] || die "macOS 14 or newer is required; this Mac has $macos_version."

say ''
say 'You will install the Tohseno installer.'
say 'Enter to continue. Esc to exit.'
escape_character=$(/usr/bin/printf '\\033')
while :; do
  tty_state=$(/bin/stty -g < /dev/tty) || die 'the terminal state could not be read.'
  /bin/stty -echo -icanon min 1 time 0 < /dev/tty || die 'the terminal could not enter confirmation mode.'
  confirmation=$(/bin/dd bs=1 count=1 2>/dev/null < /dev/tty || true)
  /bin/stty "$tty_state" < /dev/tty || true
  tty_state=''
  case "$confirmation" in
    '') say ''; break ;;
    "$escape_character") say ''; say 'Nothing was installed.'; exit 0 ;;
    *) /usr/bin/printf '\\a' > /dev/tty ;;
  esac
done

temp_dir=$(/usr/bin/mktemp -d /tmp/tohseno-native-install.XXXXXX) || die 'a temporary folder could not be created.'
dmg_path="$temp_dir/Tohseno.dmg"
mount_point="$temp_dir/mount"
/bin/mkdir "$mount_point"
downloads_dir="$HOME/Downloads"
/bin/mkdir -p "$downloads_dir" || die 'the Downloads folder could not be prepared.'
destination="$downloads_dir/$download_name"
[ ! -L "$destination" ] || die "$destination is a symbolic link and was left untouched."
if [ -e "$destination" ]; then
  [ -f "$destination" ] || die "$destination already exists and is not a regular file."
  existing_sha256=$(/usr/bin/shasum -a 256 "$destination" | /usr/bin/sed 's/[[:space:]].*$//')
  if [ "$existing_sha256" = "$dmg_sha256" ]; then
    say ''
    say 'Tohseno is ready.'
    say "$destination"
    say 'Double-click it, then drag Tohseno into Applications.'
    /usr/bin/open -R "$destination" || die 'the verified installer could not be revealed in Finder.'
    exit 0
  fi
  die "$destination already exists with different contents. Move it elsewhere, then try again."
fi

say 'Downloading Tohseno…'
/usr/bin/curl --fail --location --progress-bar --show-error --proto '=https' --proto-redir '=https' --tlsv1.2 --max-filesize 536870912 --output "$dmg_path" "$dmg_url" 2> /dev/tty || die 'the DMG download failed.'

actual_sha256=$(/usr/bin/shasum -a 256 "$dmg_path" | /usr/bin/sed 's/[[:space:]].*$//')
[ "$actual_sha256" = "$dmg_sha256" ] || die 'the downloaded DMG did not match the published SHA-256.'

say 'Checking the app signature and notarization…'
/usr/bin/hdiutil attach "$dmg_path" -readonly -nobrowse -noautoopen -mountpoint "$mount_point" -quiet || die 'the verified DMG could not be mounted.'
source_app="$mount_point/Tohseno.app"
[ -d "$source_app" ] && [ ! -L "$source_app" ] || die 'the DMG does not contain the expected Tohseno.app.'
/usr/bin/codesign --verify --deep --strict --verbose=2 "$source_app" >/dev/null 2>&1 || die 'the Tohseno app signature is invalid.'
signature=$(/usr/bin/codesign -d --verbose=4 "$source_app" 2>&1)
bundle_id=$(/usr/bin/printf '%s\\n' "$signature" | /usr/bin/sed -n 's/^Identifier=//p')
team_id=$(/usr/bin/printf '%s\\n' "$signature" | /usr/bin/sed -n 's/^TeamIdentifier=//p')
[ "$bundle_id" = "$expected_bundle_id" ] || die 'the app has an unexpected bundle identifier.'
[ "$team_id" = "$expected_team_id" ] || die 'the app was not signed by the expected Tohseno Apple team.'
/usr/sbin/spctl --assess --type execute --verbose=2 "$source_app" >/dev/null 2>&1 || die 'Gatekeeper did not accept the notarized Tohseno app.'

/usr/bin/hdiutil detach "$mount_point" -quiet || die 'the verified DMG could not be closed cleanly.'
mount_point=''
/bin/mv "$dmg_path" "$destination" || die 'the verified installer could not be moved into Downloads.'
say ''
say 'Tohseno is ready.'
say "$destination"
say 'Double-click it, then drag Tohseno into Applications.'
/usr/bin/open -R "$destination" || die 'the verified installer could not be revealed in Finder.'
`;
  return script
    .replace("__DMG_URL__", shellSingleQuote(downloadURL))
    .replace("__DMG_SHA256__", shellSingleQuote(downloadSHA256))
    .replace("__DOWNLOAD_NAME__", shellSingleQuote(downloadName));
}

const PAGE_PATHS = ["/", "/docs", "/privacy", "/healthz"] as const;

const STATIC_FILES: Record<
  string,
  { file: string; type: string; revalidate?: boolean }
> = {
  "/styles.css": { file: "styles.css", type: "text/css; charset=utf-8" },
  "/landing.css": {
    file: "landing.css",
    type: "text/css; charset=utf-8",
    revalidate: true,
  },
  "/fonts/fraunces-latin.woff2": {
    file: "fonts/fraunces-latin.woff2",
    type: "font/woff2",
  },
  "/fonts/plex-mono-latin.woff2": {
    file: "fonts/plex-mono-latin.woff2",
    type: "font/woff2",
  },
  "/app.js": {
    file: "app.js",
    type: "text/javascript; charset=utf-8",
    revalidate: true,
  },
  "/landing.js": {
    file: "landing.js",
    type: "text/javascript; charset=utf-8",
    revalidate: true,
  },
  "/manifest.webmanifest": {
    file: "manifest.webmanifest",
    type: "application/manifest+json; charset=utf-8",
    revalidate: true,
  },
  "/sw.js": {
    file: "sw.js",
    type: "text/javascript; charset=utf-8",
    revalidate: true,
  },
  "/install.sh": {
    file: "install.sh",
    type: "text/x-shellscript; charset=utf-8",
    revalidate: true,
  },
  "/oneshot.sh": {
    file: "oneshot.sh",
    type: "text/x-shellscript; charset=utf-8",
    revalidate: true,
  },
  "/releases/native-v1.json": {
    file: "releases/native-v1.json",
    type: "application/json; charset=utf-8",
    revalidate: true,
  },
  "/robots.txt": { file: "robots.txt", type: "text/plain; charset=utf-8" },
  "/logo.svg": { file: "logo.svg", type: "image/svg+xml" },
  "/whitepaper.pdf": {
    file: "tohseno-whitepaper.pdf",
    type: "application/pdf",
    revalidate: true,
  },
  "/og.png": { file: "og.png", type: "image/png" },
  "/favicon.png": { file: "favicon.png", type: "image/png" },
  "/tohseno-logo.png": { file: "tohseno-logo.png", type: "image/png" },
  "/app-breathekeeper.png": {
    file: "app-breathekeeper.png",
    type: "image/png",
  },
  "/app-who-ate.png": { file: "app-who-ate.png", type: "image/png" },
  "/app-handoff.png": { file: "app-handoff.png", type: "image/png" },
  "/app-water-walk.png": { file: "app-water-walk.png", type: "image/png" },
  "/app-ink-memory.png": { file: "app-ink-memory.png", type: "image/png" },
  "/app-room-tone.png": { file: "app-room-tone.png", type: "image/png" },
};

const SHOT_ICON_PATH = /^\/shot-icons\/shot-(?:00[1-9]|0[1-9]\d|100)\.webp$/;
const BROWSER_MODULE_PATH = /^\/modules\/[a-z0-9-]+\.js$/;

function semanticRoute(pathname: string): string {
  if (pathname.startsWith("/api/intent-relay/")) return "intent-relay";
  if (pathname.startsWith("/api/billing/v1/")) return "billing";
  if (pathname.startsWith("/api/managed/v1/")) return "managed-compute";
  if (pathname.startsWith("/api/registry/v1/")) return "registry-api";
  if (pathname === "/install" || pathname === "/download") return "native-installer";
  if (pathname === "/download/macos" || pathname === "/api/distribution/v1/macos") return "macos-download";
  if (pathname === "/") return "landing-page";
  if (pathname === "/registry" || pathname.startsWith("/s/") || pathname.startsWith("/@")) return "public-registry";
  if (pathname === "/docs") return "docs-page";
  if (pathname === "/privacy") return "privacy-page";
  if (pathname === "/healthz") return "health";
  if (pathname === "/install.sh") return "installer";
  if (pathname === "/oneshot.sh") return "installer";
  if (pathname === "/releases/native-v1.json")
    return "native-release-manifest";
  if (pathname === "/whitepaper.pdf") return "whitepaper";
  if (STATIC_FILES[pathname]) return "static-asset";
  if (SHOT_ICON_PATH.test(pathname)) return "shot-icon";
  if (BROWSER_MODULE_PATH.test(pathname)) return "browser-module";
  return "unmatched";
}

function externalRequestHostname(request: Request, config: AppConfig): string {
  if (config.trustProxy) {
    const forwarded = request.headers
      .get("x-forwarded-host")
      ?.split(",", 1)[0]
      ?.trim();
    if (forwarded) {
      try {
        return new URL(`https://${forwarded}`).hostname.toLowerCase();
      } catch {
        return "";
      }
    }
  }
  return new URL(request.url).hostname.toLowerCase();
}

function canonicalBoundary(
  request: Request,
  config: AppConfig,
): Response | null {
  const canonical = new URL(config.baseUrl);
  const aliasHost = canonical.hostname.startsWith("www.")
    ? canonical.hostname.slice(4)
    : `www.${canonical.hostname}`;
  const requestedHostname = externalRequestHostname(request, config);
  const method = request.method.toUpperCase();
  const forwardedProtocol = config.trustProxy
    ? request.headers
        .get("x-forwarded-proto")
        ?.split(",", 1)[0]
        ?.trim()
        .toLowerCase()
    : undefined;
  const insecureProductionRequest =
    config.nodeEnv === "production" && forwardedProtocol === "http";
  const canonicalAlias = requestedHostname === aliasHost;
  if (!canonicalAlias && !insecureProductionRequest) return null;
  if (method !== "GET" && method !== "HEAD") return methodNotAllowed();
  const source = new URL(request.url);
  const destination = new URL(config.baseUrl);
  destination.pathname = source.pathname;
  destination.search = source.search;
  return headResponse(
    withSecurityHeaders(Response.redirect(destination, 308)),
    method,
  );
}

export async function createApplication(
  options: ApplicationOptions = {},
): Promise<TohsenoApplication> {
  const config = options.config ?? loadConfig();
  const relay = await createRelayRouter(config);
  const billing = await createBillingRouter(config);
  const managed = await createManagedRouter(config);
  let registryReference: RegistryRouter | undefined;
  const claims = await createClaimsRouter(config, undefined, undefined, {
    currentClaimContext: async (shotID, releaseDigest) => {
      if (!registryReference) throw new HttpError(503, "Registry startup is incomplete");
      return registryReference.currentClaimContext(shotID, releaseDigest);
    },
    claimReceiptContext: async (shotID, releaseDigest) => {
      if (!registryReference) throw new HttpError(503, "Registry startup is incomplete");
      return registryReference.claimReceiptContext(shotID, releaseDigest);
    },
  });
  const registry = await createRegistryRouter(config, undefined, claims);
  registryReference = registry;
  const log = options.log ??
    ((record: Record<string, unknown>) => console.info(JSON.stringify(record)));
  const logError = options.logError ??
    ((record: Record<string, unknown>) => console.error(JSON.stringify(record)));
  // Social scrapers and the CDN cache /og.png aggressively; a content-hash
  // query makes every new image a new URL so previews update on deploy.
  const ogImageBytes = await Bun.file(join(PUBLIC_DIRECTORY, "og.png")).bytes();
  const ogImageVersion = new Bun.CryptoHasher("sha256")
    .update(ogImageBytes)
    .digest("hex")
    .slice(0, 8);
  // The terminal's stylesheet must revalidate the moment it changes, and the
  // revision has to be derived rather than hand-maintained in the markup.
  const landingStyleBytes = await Bun.file(
    join(PUBLIC_DIRECTORY, "landing.css"),
  ).bytes();
  const landingStyleRevision = new Bun.CryptoHasher("sha256")
    .update(landingStyleBytes)
    .digest("hex")
    .slice(0, 12);
  const renderPage = async (file: string): Promise<string> =>
    renderTemplate(await Bun.file(join(PUBLIC_DIRECTORY, file)).text(), {
      CANONICAL_ORIGIN: config.baseUrl,
      OG_IMAGE_URL: `${config.baseUrl}/og.png?v=${ogImageVersion}`,
      LANDING_STYLE_REVISION: landingStyleRevision,
      DOWNLOAD_CHANNEL: config.distribution.macosChannel,
    });
  const networkLaunchEnabled = config.registry.enabled
    && config.registry.relayerEnabled
    && config.distribution.macosEnabled;
  const [landingPage, docsPage, privacyPage] = await Promise.all([
    renderPage(networkLaunchEnabled ? "index-network.html" : "index.html"),
    renderPage("docs.html"),
    renderPage("privacy.html"),
  ]);
  const pages: Record<string, string> = {
    "/": landingPage,
    "/docs": docsPage,
    "/privacy": privacyPage,
  };

  async function handle(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const { pathname } = url;
    const method = request.method.toUpperCase();
    const canonicalResponse = canonicalBoundary(request, config);
    if (canonicalResponse) return canonicalResponse;
    if (relay.handles(pathname)) return relay.fetch(request);
    if (billing.handles(pathname)) return billing.fetch(request);
    if (managed.handles(pathname)) return managed.fetch(request);
    if (claims.handles(pathname)) return claims.fetch(request);
    if (registry.handles(pathname)) return registry.fetch(request);

    if (pathname === "/registry" || pathname.startsWith("/s/") || pathname.startsWith("/@")
        || pathname.startsWith("/claims/")) {
      if (method !== "GET" && method !== "HEAD") return methodNotAllowed();
      let content: string | undefined;
      if (pathname === "/registry") content = await registry.renderRegistry(url.searchParams.get("q") ?? undefined);
      else if (/^\/claims\/[1-9]\d*$/.test(pathname)) content = await claims.renderReceipt(pathname.slice(8));
      else if (/^\/s\/[0-9a-f]{64}$/.test(pathname)) content = await registry.renderShot(`0x${pathname.slice(3)}`);
      else if (/^\/@[^/]+$/.test(pathname)) content = await registry.renderBuilder(decodeURIComponent(pathname.slice(2)));
      else content = await registry.renderHumanRoute(pathname);
      if (!content) throw new HttpError(404, "Not found");
      return headResponse(html(content), method);
    }

    if (pathname === "/install" || pathname === "/download") {
      if (method !== "GET" && method !== "HEAD") return methodNotAllowed();
      if (!config.distribution.macosEnabled || !config.distribution.macosUrl || !config.distribution.macosSha256) {
        const unavailable = json({
          error: "The signed and notarized Mac installer is not published yet.",
        }, 503);
        const headers = new Headers(unavailable.headers);
        headers.set("cache-control", "no-store");
        headers.set("x-tohseno-install-status", "not-published");
        return headResponse(new Response(unavailable.body, {
          status: unavailable.status,
          statusText: unavailable.statusText,
          headers,
        }), method);
      }
      const installer = renderNativeMacInstaller(
        config.distribution.macosUrl,
        config.distribution.macosSha256,
      );
      return headResponse(withSecurityHeaders(new Response(installer, {
        headers: {
          "content-type": "text/x-shellscript; charset=utf-8",
          "cache-control": "no-store",
          "x-content-type-options": "nosniff",
          "x-tohseno-install-command": "curl -fsSL https://tohseno.com/install | sh",
          "x-tohseno-source": PRODUCT.repositoryUrl,
        },
      })), method);
    }

    if (pathname === "/download/macos" || pathname === "/api/distribution/v1/macos") {
      if (method !== "GET" && method !== "HEAD") return methodNotAllowed();
      if (!config.distribution.macosEnabled || !config.distribution.macosUrl || !config.distribution.macosSha256) {
        return headResponse(json({ error: "The signed and notarized Mac download is not published yet." }, 503), method);
      }
      if (pathname === "/api/distribution/v1/macos") {
        return headResponse(json({ schema: "tohseno.macos-distribution/1", available: true,
          channel: config.distribution.macosChannel,
          url: config.distribution.macosUrl, sha256: config.distribution.macosSha256,
          minimum_macos_version: "14.0" }), method);
      }
      return headResponse(withSecurityHeaders(new Response(null, { status: 307, headers: {
        location: config.distribution.macosUrl,
        "x-tohseno-sha256": config.distribution.macosSha256,
        "x-tohseno-release-channel": config.distribution.macosChannel,
        "cache-control": "no-store",
      } })), method);
    }

    if (method !== "GET" && method !== "HEAD") {
      if (
        (PAGE_PATHS as readonly string[]).includes(pathname) ||
        STATIC_FILES[pathname] ||
        SHOT_ICON_PATH.test(pathname) ||
        BROWSER_MODULE_PATH.test(pathname)
      ) {
        return methodNotAllowed();
      }
      throw new HttpError(404, "Not found");
    }

    const page = pages[pathname];
    if (page !== undefined) return headResponse(html(page), method);
    if (pathname === "/healthz") {
      return headResponse(json({ status: "ok", service: "tohseno" }), method);
    }

    const staticFile = STATIC_FILES[pathname];
    if (staticFile) {
      const response = new Response(
        Bun.file(join(PUBLIC_DIRECTORY, staticFile.file)),
        {
          headers: {
            "Content-Type": staticFile.type,
            "Cache-Control": staticFile.revalidate
              ? "public, max-age=0, must-revalidate"
              : "public, max-age=3600",
          },
        },
      );
      return headResponse(withSecurityHeaders(response), method);
    }

    if (SHOT_ICON_PATH.test(pathname)) {
      return headResponse(
        withSecurityHeaders(
          new Response(Bun.file(join(PUBLIC_DIRECTORY, pathname.slice(1))), {
            headers: {
              "Content-Type": "image/webp",
              "Cache-Control": "public, max-age=3600",
            },
          }),
        ),
        method,
      );
    }

    if (BROWSER_MODULE_PATH.test(pathname)) {
      return headResponse(
        withSecurityHeaders(
          new Response(Bun.file(join(PUBLIC_DIRECTORY, pathname.slice(1))), {
            headers: {
              "Content-Type": "text/javascript; charset=utf-8",
              "Cache-Control": "public, max-age=0, must-revalidate",
            },
          }),
        ),
        method,
      );
    }

    throw new HttpError(404, "Not found");
  }

  return {
    config,
    async fetch(request: Request): Promise<Response> {
      const requestId = crypto.randomUUID();
      const started = performance.now();
      const requestedMethod = request.method.toUpperCase();
      const method = requestedMethod === "GET" || requestedMethod === "HEAD"
        ? requestedMethod
        : "OTHER";
      const route = semanticRoute(new URL(request.url).pathname);
      let status = 500;
      try {
        const response = await handle(request);
        status = response.status;
        return response;
      } catch (error) {
        let response: Response;
        if (error instanceof HttpError)
          response = json({ error: error.message }, error.status);
        else {
          logError({
            event: "request_failure",
            requestId,
            method,
            route,
            errorType:
              error instanceof Error ? error.constructor.name : "Unknown",
          });
          response = json({ error: "The request could not be completed" }, 500);
        }
        status = response.status;
        return response;
      } finally {
        log({
          event: "request",
          requestId,
          method,
          route,
          status,
          durationMs: Math.round((performance.now() - started) * 100) / 100,
        });
      }
    },
  };
}

if (import.meta.main) {
  try {
    const application = await createApplication();
    console.info(
      JSON.stringify({
        event: "startup",
        ...safeStartupSummary(application.config),
      }),
    );
    Bun.serve({
      port: application.config.port,
      // Encrypted transfers are deliberately chunked; this global bound only
      // admits one 1 MiB chunk plus conservative HTTP framing.
      maxRequestBodySize: INTENT_LIMITS.chunkBytes + INTENT_LIMITS.framingAllowance,
      fetch: application.fetch,
    });
  } catch (error) {
    console.error(
      JSON.stringify({
        event: "startup_failed",
        errorType: error instanceof Error ? error.constructor.name : "Unknown",
      }),
    );
    process.exit(1);
  }
}
