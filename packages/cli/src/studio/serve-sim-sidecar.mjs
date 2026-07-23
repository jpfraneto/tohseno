#!/usr/bin/env node

import { createServer } from "node:http";
import {
  existsSync,
  lstatSync,
  readFileSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join, resolve } from "node:path";

const EXPECTED_SERVE_SIM_VERSION = "0.1.45";
const HOST = "127.0.0.1";
const UDID_ENV = "TOHSENO_SERVE_SIM_UDID";
const CAPABILITY_ENV = "TOHSENO_SERVE_SIM_CAPABILITY";
const CANONICAL_UDID =
  /^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$/u;
const CAPABILITY = /^[A-Za-z0-9_-]{43,128}$/u;

function fail(code) {
  process.stderr.write(`${JSON.stringify({ schemaVersion: 1, event: "failed", code })}\n`);
  process.exitCode = 1;
}

function packageInstallation() {
  let middlewarePath;
  try {
    middlewarePath = createRequire(import.meta.url).resolve("serve-sim/middleware");
  } catch {
    return null;
  }
  let directory = dirname(middlewarePath);
  while (true) {
    const packageJsonPath = join(directory, "package.json");
    if (existsSync(packageJsonPath)) {
      try {
        const value = JSON.parse(readFileSync(packageJsonPath, "utf8"));
        if (value.name === "serve-sim" && typeof value.version === "string") {
          return {
            version: value.version,
            middleware:
              value.exports &&
              typeof value.exports === "object" &&
              value.exports["./middleware"] !== undefined,
          };
        }
      } catch {
        return null;
      }
    }
    const parent = dirname(directory);
    if (parent === directory) return null;
    directory = parent;
  }
}

function validateTemporaryDirectory() {
  const value = process.env.TMPDIR;
  if (!value || !isAbsolute(value) || !existsSync(value)) return null;
  const details = lstatSync(value);
  if (details.isSymbolicLink() || !details.isDirectory()) return null;
  return realpathSync(resolve(value));
}

function requestUrl(rawUrl) {
  try {
    return new URL(rawUrl ?? "/", "http://sidecar.invalid");
  } catch {
    return null;
  }
}

function selectedDeviceOnly(url, udid) {
  const requested = url.searchParams.getAll("device");
  return requested.length === 0 ||
    (requested.length === 1 && requested[0] === udid);
}

function approvedReadPath(url, basePath, udid) {
  if (!selectedDeviceOnly(url, udid)) return false;
  const path = url.pathname;
  const fixed = new Set([
    basePath,
    `${basePath}/`,
    `${basePath}/api`,
    `${basePath}/api/events`,
    `${basePath}/api/event-log`,
    `${basePath}/api/event-log/events`,
    `${basePath}/appstate`,
    `${basePath}/ax`,
    `${basePath}/grid/api`,
    `${basePath}/grid/api/memory`,
    `${basePath}/grid/api/devicekit-chrome`,
    `${basePath}/grid/api/device-placeholder-asset`,
  ]);
  if (fixed.has(path)) return true;
  const helper = `${basePath}/helper/${udid}`;
  return new Set([
    `${helper}/stream.mjpeg`,
    `${helper}/stream.avcc`,
    `${helper}/config`,
    `${helper}/health`,
    `${helper}/ax`,
    `${helper}/foreground`,
    `${helper}/camera/status`,
  ]).has(path);
}

function rejected(res, status = 404) {
  if (res.headersSent || res.writableEnded) return;
  res.writeHead(status, {
    "Content-Type": "text/plain; charset=utf-8",
    "Cache-Control": "no-store",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
  });
  res.end("Not found");
}

function secureResponseHeaders(res) {
  const original = res.writeHead.bind(res);
  res.writeHead = (statusCode, statusMessageOrHeaders, maybeHeaders) => {
    const hasStatusMessage = typeof statusMessageOrHeaders === "string";
    const provided = hasStatusMessage ? maybeHeaders : statusMessageOrHeaders;
    const headers = {
      ...(provided && typeof provided === "object" ? provided : {}),
      "Referrer-Policy": "no-referrer",
      "X-Content-Type-Options": "nosniff",
      "Cross-Origin-Resource-Policy": "same-site",
    };
    return hasStatusMessage
      ? original(statusCode, statusMessageOrHeaders, headers)
      : original(statusCode, headers);
  };
}

async function closeServer(server, sockets) {
  for (const socket of sockets) socket.destroy();
  server.closeAllConnections?.();
  await new Promise((resolveClose) => {
    if (!server.listening) {
      resolveClose();
      return;
    }
    server.close(() => resolveClose());
  });
}

async function main() {
  // Keep unsupported hosts away from serve-sim's native import entirely.
  if (process.platform !== "darwin") {
    fail("UNSUPPORTED_PLATFORM");
    return;
  }
  if (process.arch !== "arm64") {
    fail("UNSUPPORTED_ARCHITECTURE");
    return;
  }
  const nodeMajor = Number(process.versions.node.split(".", 1)[0]);
  if (!Number.isInteger(nodeMajor) || nodeMajor < 20) {
    fail("UNSUPPORTED_NODE");
    return;
  }
  const installation = packageInstallation();
  if (
    !installation ||
    installation.version !== EXPECTED_SERVE_SIM_VERSION ||
    !installation.middleware
  ) {
    fail("SERVE_SIM_VERSION");
    return;
  }
  const temporaryDirectory = validateTemporaryDirectory();
  if (!temporaryDirectory) {
    fail("INVALID_TEMPORARY_DIRECTORY");
    return;
  }
  const udid = (process.env[UDID_ENV] ?? "").trim();
  const capability = (process.env[CAPABILITY_ENV] ?? "").trim();
  if (!CANONICAL_UDID.test(udid)) {
    fail("INVALID_DEVICE");
    return;
  }
  if (!CAPABILITY.test(capability)) {
    fail("INVALID_CAPABILITY");
    return;
  }

  let upstream;
  try {
    upstream = await import("serve-sim/middleware");
  } catch {
    fail("SERVE_SIM_IMPORT");
    return;
  }
  if (
    typeof upstream.simMiddleware !== "function" ||
    typeof upstream.startDeviceInProcess !== "function"
  ) {
    fail("SERVE_SIM_API");
    return;
  }

  const basePath = `/_tohseno/live/${capability}`;
  const middleware = upstream.simMiddleware({
    basePath,
    device: udid,
    proxyHelpers: true,
    initialState: { panes: [], fit: true },
  });
  if (typeof middleware !== "function" || typeof middleware.handleUpgrade !== "function") {
    fail("SERVE_SIM_API");
    return;
  }

  let expectedHost = null;
  let expectedOrigin = null;
  const sockets = new Set();
  const server = createServer((req, res) => {
    secureResponseHeaders(res);
    if (!expectedHost || req.headers.host !== expectedHost) {
      rejected(res, 403);
      return;
    }
    const origin = req.headers.origin;
    if (origin !== undefined && origin !== expectedOrigin) {
      rejected(res, 403);
      return;
    }
    if (req.method !== "GET" && req.method !== "HEAD") {
      rejected(res, 405);
      return;
    }
    const url = requestUrl(req.url);
    if (!url || !approvedReadPath(url, basePath, udid)) {
      rejected(res);
      return;
    }
    if (req.method === "HEAD") {
      res.writeHead(204, { "Cache-Control": "no-store" });
      res.end();
      return;
    }

    // Upstream intentionally exposes shell exec, grid mutation, and DevTools.
    // The allowlist above is the security boundary: only preview/state/stream
    // reads reach the official middleware, and no POST reaches it at all.
    Promise.resolve(middleware(req, res, () => {
      rejected(res);
    })).catch(() => {
      rejected(res, 500);
    });
  });
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
  });

  server.on("upgrade", (req, socket, head) => {
    if (!expectedHost || req.headers.host !== expectedHost) {
      socket.destroy();
      return;
    }
    if (req.headers.origin !== expectedOrigin) {
      socket.destroy();
      return;
    }
    const url = requestUrl(req.url);
    const hidPath = `${basePath}/helper/${udid}/ws`;
    if (
      req.method !== "GET" ||
      !url ||
      url.pathname !== hidPath ||
      url.search !== ""
    ) {
      socket.destroy();
      return;
    }

    // Only the selected device's HID socket is forwarded. In particular,
    // upstream /exec-ws and every DevTools upgrade remain unreachable.
    middleware.handleUpgrade(req, socket, head);
  });

  try {
    await new Promise((resolveListen, rejectListen) => {
      const onError = () => rejectListen(new Error("listen failed"));
      server.once("error", onError);
      server.listen(0, HOST, () => {
        server.off("error", onError);
        resolveListen();
      });
    });
  } catch {
    fail("SIDECAR_LISTEN");
    return;
  }
  const address = server.address();
  if (!address || typeof address === "string" || address.address !== HOST) {
    await closeServer(server, sockets);
    fail("SIDECAR_ADDRESS");
    return;
  }
  expectedHost = `${HOST}:${address.port}`;
  expectedOrigin = `http://${expectedHost}`;

  let closePromise = null;
  const shutdownServer = () => {
    closePromise ??= closeServer(server, sockets);
    return closePromise;
  };
  let stopRequested = false;
  let resolveSignal;
  const signal = new Promise((resolveStop) => {
    resolveSignal = resolveStop;
  });
  const requestStop = () => {
    stopRequested = true;
    resolveSignal();
    void shutdownServer();
  };
  process.once("SIGINT", requestStop);
  process.once("SIGTERM", requestStop);
  server.on("error", requestStop);

  let startError;
  try {
    startError = await upstream.startDeviceInProcess(
      udid,
      address.port,
      basePath,
    );
  } catch {
    startError = "failed";
  }
  if (startError !== null) {
    await shutdownServer();
    fail("DEVICE_START");
    return;
  }
  const stateFile = join(
    temporaryDirectory,
    "serve-sim",
    `server-${udid}.json`,
  );
  if (stopRequested) {
    await shutdownServer();
    if (existsSync(stateFile)) {
      const details = lstatSync(stateFile);
      if (!details.isSymbolicLink() && details.isFile()) rmSync(stateFile, { force: true });
    }
    return;
  }

  process.stdout.write(`${JSON.stringify({
    schemaVersion: 1,
    event: "ready",
    host: HOST,
    port: address.port,
    device: udid,
  })}\n`);

  await signal;
  await shutdownServer();
  if (existsSync(stateFile)) {
    const details = lstatSync(stateFile);
    if (!details.isSymbolicLink() && details.isFile()) rmSync(stateFile, { force: true });
  }
}

await main();
