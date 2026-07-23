import { randomBytes, timingSafeEqual } from "node:crypto";

const SESSION_TOKEN_BYTES = 32;
const SESSION_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/u;

export class StudioHttpError extends Error {
  override readonly name = "StudioHttpError";
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

export interface StudioSecurityOptions {
  hostname?: string;
  port?: number;
  sessionToken?: string;
}

function loopbackAuthorities(port: number): ReadonlySet<string> {
  const suffix = port === 80 ? "" : `:${port}`;
  return new Set([`127.0.0.1${suffix}`, `localhost${suffix}`]);
}

function exactAuthority(value: string | null): string {
  if (
    value === null ||
    value === "" ||
    value.includes(",") ||
    value.includes("@") ||
    /[\s/\\]/u.test(value)
  ) {
    throw new StudioHttpError(
      421,
      "unsafe-host",
      "Studio accepts requests only from its local loopback address.",
    );
  }
  return value.toLowerCase();
}

function equalToken(expected: string, supplied: string | null): boolean {
  if (supplied === null || !SESSION_TOKEN_PATTERN.test(supplied)) return false;
  const expectedBytes = Buffer.from(expected);
  const suppliedBytes = Buffer.from(supplied);
  return expectedBytes.length === suppliedBytes.length &&
    timingSafeEqual(expectedBytes, suppliedBytes);
}

export function createStudioSessionToken(): string {
  return randomBytes(SESSION_TOKEN_BYTES).toString("base64url");
}

export class StudioRequestSecurity {
  readonly sessionToken: string;
  readonly #hostname: "127.0.0.1";
  #port: number;

  constructor(options: StudioSecurityOptions = {}) {
    const hostname = options.hostname ?? "127.0.0.1";
    if (hostname !== "127.0.0.1") {
      throw new StudioHttpError(
        500,
        "unsafe-bind",
        "Studio must bind to 127.0.0.1.",
      );
    }
    const port = options.port ?? 4747;
    if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
      throw new StudioHttpError(500, "invalid-port", "Studio has an invalid port.");
    }
    const sessionToken = options.sessionToken ?? createStudioSessionToken();
    if (!SESSION_TOKEN_PATTERN.test(sessionToken)) {
      throw new StudioHttpError(
        500,
        "invalid-session",
        "Studio could not create a secure local session.",
      );
    }
    this.#hostname = hostname;
    this.#port = port;
    this.sessionToken = sessionToken;
  }

  get port(): number {
    return this.#port;
  }

  /**
   * Supports a server initially bound to port 0. Call this with Bun's selected
   * port before accepting requests.
   */
  setPort(port: number): void {
    if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
      throw new StudioHttpError(500, "invalid-port", "Studio has an invalid port.");
    }
    this.#port = port;
  }

  allowedOrigins(): readonly string[] {
    const suffix = this.#port === 80 ? "" : `:${this.#port}`;
    return [
      `http://${this.#hostname}${suffix}`,
      `http://localhost${suffix}`,
    ];
  }

  assertHost(request: Request): string {
    const authority = exactAuthority(request.headers.get("host"));
    if (!loopbackAuthorities(this.#port).has(authority)) {
      throw new StudioHttpError(
        421,
        "unsafe-host",
        "Studio accepts requests only from its local loopback address.",
      );
    }
    const origin = request.headers.get("origin");
    if (origin !== null && origin !== `http://${authority}`) {
      throw new StudioHttpError(
        403,
        "unsafe-origin",
        "Studio rejected a request from another origin.",
      );
    }
    return authority;
  }

  assertMutation(request: Request): void {
    const authority = this.assertHost(request);
    const origin = request.headers.get("origin");
    if (origin !== `http://${authority}`) {
      throw new StudioHttpError(
        403,
        "unsafe-origin",
        "Studio rejected a request from another origin.",
      );
    }
    const fetchSite = request.headers.get("sec-fetch-site");
    if (
      fetchSite !== null &&
      fetchSite !== "same-origin" &&
      fetchSite !== "none"
    ) {
      throw new StudioHttpError(
        403,
        "unsafe-fetch-site",
        "Studio rejected a cross-site request.",
      );
    }
    if (!equalToken(this.sessionToken, request.headers.get("x-tohseno-session"))) {
      throw new StudioHttpError(
        403,
        "invalid-session",
        "Studio rejected an invalid local session.",
      );
    }
  }
}

export function studioSecurityHeaders(
  options: { html?: boolean; liveRedirect?: boolean } = {},
): Headers {
  const headers = new Headers({
    "Cache-Control": "no-store",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
    "Permissions-Policy":
      "camera=(), geolocation=(), microphone=(), payment=(), usb=()",
  });
  if (!options.liveRedirect) {
    headers.set("Cross-Origin-Resource-Policy", "same-origin");
  }
  if (options.html) {
    headers.set(
      "Content-Security-Policy",
      [
        "default-src 'self'",
        "base-uri 'none'",
        "connect-src 'self'",
        "font-src 'self'",
        "form-action 'self'",
        "frame-ancestors 'none'",
        "frame-src 'self' http://127.0.0.1:*",
        "img-src 'self' data: blob:",
        "object-src 'none'",
        "script-src 'self'",
        "style-src 'self'",
      ].join("; "),
    );
  }
  return headers;
}

export function withStudioSecurityHeaders(
  response: Response,
  options: { html?: boolean; liveRedirect?: boolean } = {},
): Response {
  const headers = new Headers(response.headers);
  for (const [name, value] of studioSecurityHeaders(options)) {
    if (!headers.has(name)) headers.set(name, value);
  }
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
