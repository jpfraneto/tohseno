import {
  parseSignedPublicShotRecord,
  ProtocolValidationError,
  validateShotId,
} from "../../../packages/protocol/src/index.ts";
import {
  createNodeErrorPayload,
  createNodeRecordsPayload,
  createNodeSubmissionPayload,
  validateNodeProjectionPayload,
} from "../../../packages/node-client/src/payloads.ts";
import {
  type PublicRecordRegistry,
  RegistryError,
} from "../../../packages/registry/src/index.ts";
import { ReferenceNodeCapacityError } from "./registry.ts";
import {
  jsonResponse,
  parseJsonBody,
  readBoundedRequestBody,
  ReferenceNodeHttpError,
} from "./http.ts";
import {
  methodClass,
  type ReferenceNodeRequestLog,
  semanticRoute,
} from "./logging.ts";
import { REFERENCE_NODE_OPENAPI } from "./openapi.ts";

export type ReferenceNodeLogger = (
  entry: ReferenceNodeRequestLog,
) => void;

export interface ReferenceNodeApplicationOptions {
  registry: PublicRecordRegistry;
  databaseSchemaVersion: number;
  openApiDocument?: unknown;
  logger?: ReferenceNodeLogger;
  clockMilliseconds?: () => number;
}

interface MappedError {
  response: Response;
  code: string;
}

function errorResponse(
  status: number,
  code: string,
  allow?: string,
): Response {
  const response = jsonResponse(createNodeErrorPayload(code), status);
  if (allow === undefined) return response;
  const headers = new Headers(response.headers);
  headers.set("Allow", allow);
  return new Response(response.body, {
    status: response.status,
    headers,
  });
}

function mappedError(error: unknown): MappedError {
  if (error instanceof ReferenceNodeHttpError) {
    return {
      response: errorResponse(error.status, error.code),
      code: error.code,
    };
  }
  if (error instanceof ProtocolValidationError) {
    return {
      response: errorResponse(400, "invalid-record"),
      code: "invalid-record",
    };
  }
  if (error instanceof RegistryError) {
    return {
      response: errorResponse(409, "record-conflict"),
      code: "record-conflict",
    };
  }
  if (error instanceof ReferenceNodeCapacityError) {
    return {
      response: errorResponse(507, "capacity-exceeded"),
      code: "capacity-exceeded",
    };
  }
  return {
    response: errorResponse(500, "internal-error"),
    code: "internal-error",
  };
}

function jsonMediaType(request: Request): boolean {
  return request.headers
    .get("content-type")
    ?.split(";", 1)[0]
    ?.trim()
    .toLowerCase() === "application/json";
}

function decodedShotId(encoded: string): string {
  let value: string;
  try {
    value = decodeURIComponent(encoded);
  } catch {
    throw new ReferenceNodeHttpError(
      404,
      "shot-not-found",
      "No public Shot was found.",
    );
  }
  try {
    return validateShotId(value);
  } catch {
    throw new ReferenceNodeHttpError(
      404,
      "shot-not-found",
      "No public Shot was found.",
    );
  }
}

export function createReferenceNodeApplication(
  options: ReferenceNodeApplicationOptions,
): (request: Request) => Promise<Response> {
  const openApiDocument = options.openApiDocument ?? REFERENCE_NODE_OPENAPI;
  const logger = options.logger ?? (() => undefined);
  const clock = options.clockMilliseconds ?? (() => performance.now());

  async function dispatch(request: Request, url: URL): Promise<Response> {
    if (url.pathname === "/healthz") {
      if (request.method !== "GET") {
        return errorResponse(405, "method-not-allowed", "GET");
      }
      return jsonResponse({
        status: "ok",
        service: "tohseno-reference-node",
        databaseSchemaVersion: options.databaseSchemaVersion,
      });
    }

    if (url.pathname === "/openapi.json") {
      if (request.method !== "GET") {
        return errorResponse(405, "method-not-allowed", "GET");
      }
      return jsonResponse(openApiDocument);
    }

    if (url.pathname === "/v1/records") {
      if (request.method !== "POST") {
        return errorResponse(405, "method-not-allowed", "POST");
      }
      if (url.search !== "") {
        throw new ReferenceNodeHttpError(
          400,
          "query-not-allowed",
          "Record submission does not accept query parameters.",
        );
      }
      const contentEncoding = request.headers.get("content-encoding");
      if (
        contentEncoding !== null &&
        contentEncoding.trim().toLowerCase() !== "identity"
      ) {
        throw new ReferenceNodeHttpError(
          415,
          "unsupported-content-encoding",
          "Encoded record bodies are not accepted.",
        );
      }
      if (!jsonMediaType(request)) {
        throw new ReferenceNodeHttpError(
          415,
          "unsupported-media-type",
          "Record submissions must use application/json.",
        );
      }
      const record = parseSignedPublicShotRecord(parseJsonBody(
        await readBoundedRequestBody(request),
      ));
      const result = await options.registry.append(record);
      return jsonResponse(
        createNodeSubmissionPayload(result),
        result.status === "appended" ? 201 : 200,
      );
    }

    const recordsMatch = /^\/v1\/shots\/([^/]+)\/records$/u.exec(
      url.pathname,
    );
    if (recordsMatch !== null) {
      if (request.method !== "GET") {
        return errorResponse(405, "method-not-allowed", "GET");
      }
      const encoded = recordsMatch[1];
      if (encoded === undefined) {
        return errorResponse(404, "shot-not-found");
      }
      const shotId = decodedShotId(encoded);
      const records = options.registry.getRecords(shotId);
      if (records.length === 0) {
        return errorResponse(404, "shot-not-found");
      }
      return jsonResponse(createNodeRecordsPayload(records));
    }

    const projectionMatch = /^\/v1\/shots\/([^/]+)$/u.exec(url.pathname);
    if (projectionMatch !== null) {
      if (request.method !== "GET") {
        return errorResponse(405, "method-not-allowed", "GET");
      }
      const encoded = projectionMatch[1];
      if (encoded === undefined) {
        return errorResponse(404, "shot-not-found");
      }
      const projection = options.registry.getProjection(decodedShotId(encoded));
      return projection === undefined
        ? errorResponse(404, "shot-not-found")
        : jsonResponse(validateNodeProjectionPayload(projection));
    }

    return errorResponse(404, "not-found");
  }

  return async (request: Request): Promise<Response> => {
    const started = clock();
    const url = new URL(request.url);
    let response: Response;
    let code: string | undefined;
    try {
      response = await dispatch(request, url);
      if (!response.ok) {
        try {
          const value = await response.clone().json() as unknown;
          if (
            typeof value === "object" &&
            value !== null &&
            !Array.isArray(value) &&
            typeof (value as { error?: unknown }).error === "string"
          ) {
            code = (value as { error: string }).error;
          }
        } catch {
          code = "internal-error";
        }
      }
    } catch (error) {
      const mapped = mappedError(error);
      response = mapped.response;
      code = mapped.code;
    }

    const entry: ReferenceNodeRequestLog = {
      event: "request",
      method: methodClass(request.method),
      route: semanticRoute(url.pathname),
      status: response.status,
      durationMs: Math.max(0, Math.round(clock() - started)),
    };
    if (code !== undefined) entry.code = code;
    try {
      logger(entry);
    } catch {
      // Logging can never alter the public protocol response.
    }
    return response;
  };
}
