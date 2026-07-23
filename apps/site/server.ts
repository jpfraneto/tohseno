import { join } from "node:path";
import type { AppConfig } from "./config.ts";
import { loadConfig, PRODUCT, safeStartupSummary } from "./config.ts";
import { HttpError, withSecurityHeaders } from "./src/security.ts";

const PUBLIC_DIRECTORY = join(import.meta.dir, "public");

export interface ApplicationOptions {
  config?: AppConfig;
}

export interface TohsenoApplication {
  config: AppConfig;
  fetch(request: Request): Promise<Response>;
}

function json(data: unknown, status = 200): Response {
  return withSecurityHeaders(new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8" },
  }));
}

function html(content: string, status = 200): Response {
  return withSecurityHeaders(new Response(content, {
    status,
    headers: { "Content-Type": "text/html; charset=utf-8" },
  }));
}

function htmlEscape(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    "\"": "&quot;",
    "'": "&#39;",
  })[character] ?? character);
}

function renderTemplate(template: string, extra: Record<string, string> = {}): string {
  const values: Record<string, string> = {
    ...PRODUCT.copy,
    INSTALL_COMMAND: PRODUCT.installCommand,
    REPOSITORY_URL: PRODUCT.repositoryUrl,
    ...extra,
  };
  const rendered = template.replace(/\{\{([A-Z0-9_]+)\}\}/g, (_match, key: string) => {
    const value = values[key];
    if (value === undefined) throw new Error(`Unknown template placeholder: ${key}`);
    return htmlEscape(value);
  });
  if (/\{\{[A-Z0-9_]+\}\}/.test(rendered)) throw new Error("Template contains unresolved placeholders");
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

const PAGE_PATHS = ["/", "/docs", "/privacy", "/healthz"] as const;

const STATIC_FILES: Record<string, { file: string; type: string; revalidate?: boolean }> = {
  "/styles.css": { file: "styles.css", type: "text/css; charset=utf-8" },
  "/landing.css": { file: "landing.css", type: "text/css; charset=utf-8", revalidate: true },
  "/fonts/fraunces-latin.woff2": { file: "fonts/fraunces-latin.woff2", type: "font/woff2" },
  "/fonts/plex-mono-latin.woff2": { file: "fonts/plex-mono-latin.woff2", type: "font/woff2" },
  "/app.js": { file: "app.js", type: "text/javascript; charset=utf-8", revalidate: true },
  "/robots.txt": { file: "robots.txt", type: "text/plain; charset=utf-8" },
  "/og.png": { file: "og.png", type: "image/png" },
  "/install.sh": { file: "install.sh", type: "text/x-shellscript; charset=utf-8", revalidate: true },
  // Bootstrap scripts and their pins must revalidate.
  "/oneshot.sh": { file: "oneshot.sh", type: "text/x-shellscript; charset=utf-8", revalidate: true },
};

function externalRequestHostname(request: Request, config: AppConfig): string {
  if (config.trustProxy) {
    const forwarded = request.headers.get("x-forwarded-host")?.split(",", 1)[0]?.trim();
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

function canonicalBoundary(request: Request, config: AppConfig): Response | null {
  const canonical = new URL(config.baseUrl);
  const aliasHost = canonical.hostname.startsWith("www.")
    ? canonical.hostname.slice(4)
    : `www.${canonical.hostname}`;
  const requestedHostname = externalRequestHostname(request, config);
  const method = request.method.toUpperCase();
  const forwardedProtocol = config.trustProxy
    ? request.headers.get("x-forwarded-proto")?.split(",", 1)[0]?.trim().toLowerCase()
    : undefined;
  const insecureProductionRequest = config.nodeEnv === "production" && forwardedProtocol === "http";
  const canonicalAlias = requestedHostname === aliasHost;
  if (!canonicalAlias && !insecureProductionRequest) return null;
  if (method !== "GET" && method !== "HEAD") return methodNotAllowed();
  const source = new URL(request.url);
  const destination = new URL(`${source.pathname}${source.search}`, config.baseUrl);
  return headResponse(withSecurityHeaders(Response.redirect(destination, 308)), method);
}

export async function createApplication(options: ApplicationOptions = {}): Promise<TohsenoApplication> {
  const config = options.config ?? loadConfig();
  // Social scrapers and the CDN cache /og.png aggressively; a content-hash
  // query makes every new image a new URL so previews update on deploy.
  const ogImageBytes = await Bun.file(join(PUBLIC_DIRECTORY, "og.png")).bytes();
  const ogImageVersion = new Bun.CryptoHasher("sha256").update(ogImageBytes).digest("hex").slice(0, 8);
  const renderPage = async (file: string): Promise<string> =>
    renderTemplate(await Bun.file(join(PUBLIC_DIRECTORY, file)).text(), {
      CANONICAL_ORIGIN: config.baseUrl,
      OG_IMAGE_URL: `${config.baseUrl}/og.png?v=${ogImageVersion}`,
    });
  const [landingPage, docsPage, privacyPage] = await Promise.all([
    renderPage("index.html"),
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

    if (method !== "GET" && method !== "HEAD") {
      if ((PAGE_PATHS as readonly string[]).includes(pathname) || STATIC_FILES[pathname]) {
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
      const response = new Response(Bun.file(join(PUBLIC_DIRECTORY, staticFile.file)), {
        headers: {
          "Content-Type": staticFile.type,
          "Cache-Control": staticFile.revalidate
            ? "public, max-age=0, must-revalidate"
            : "public, max-age=3600",
        },
      });
      return headResponse(withSecurityHeaders(response), method);
    }

    throw new HttpError(404, "Not found");
  }

  return {
    config,
    async fetch(request: Request): Promise<Response> {
      const requestId = crypto.randomUUID();
      const started = performance.now();
      const route = `${request.method.toUpperCase()} ${new URL(request.url).pathname}`;
      let status = 500;
      try {
        const response = await handle(request);
        status = response.status;
        return response;
      } catch (error) {
        let response: Response;
        if (error instanceof HttpError) response = json({ error: error.message }, error.status);
        else {
          console.error(JSON.stringify({ event: "request_failure", requestId, route, errorType: error instanceof Error ? error.constructor.name : "Unknown" }));
          response = json({ error: "The request could not be completed" }, 500);
        }
        status = response.status;
        return response;
      } finally {
        console.info(JSON.stringify({
          requestId,
          route,
          status,
          durationMs: Math.round((performance.now() - started) * 100) / 100,
        }));
      }
    },
  };
}

if (import.meta.main) {
  try {
    const application = await createApplication();
    console.info(JSON.stringify({ event: "startup", ...safeStartupSummary(application.config) }));
    Bun.serve({ port: application.config.port, fetch: application.fetch });
  } catch (error) {
    console.error(JSON.stringify({ event: "startup_failed", error: error instanceof Error ? error.message : "Unknown startup error" }));
    process.exit(1);
  }
}
