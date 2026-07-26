import {
  canonicalJson,
  hashSignedPublicShotRecord,
  parseSignedPublicShotRecord,
  validateShotId,
  type ShotId,
  type SignedPublicShotRecord,
} from "../../protocol/src/index.ts";
import {
  type PublicShotProjection,
  type RegistryAppendResult,
} from "../../registry/src/index.ts";
import {
  boundedJsonResponse,
  DEFAULT_MAX_NODE_RESPONSE_BYTES,
  DEFAULT_NODE_REQUEST_TIMEOUT_MS,
  nodeBaseUrl,
  NodeClientError,
  nodeFailure,
  type NodeFetch,
  safeFetch,
} from "./http.ts";
import {
  validateNodeErrorPayload,
  validateNodeProjectionPayload,
  validateNodeRecordsPayload,
  validateNodeSubmissionPayload,
} from "./payloads.ts";

export {
  DEFAULT_MAX_NODE_RESPONSE_BYTES,
  DEFAULT_NODE_REQUEST_TIMEOUT_MS,
  NodeClientError,
  type NodeFetch,
} from "./http.ts";
export * from "./payloads.ts";

const CLIENT_MAX_RECORD_BYTES = 256 * 1024;

export interface NodeClient {
  submit(record: SignedPublicShotRecord): Promise<RegistryAppendResult>;
  getProjection(shotId: string): Promise<PublicShotProjection | undefined>;
  getRecords(shotId: string): Promise<readonly SignedPublicShotRecord[]>;
}

export interface HttpNodeClientOptions {
  fetch?: NodeFetch;
  maxResponseBytes?: number;
  timeoutMs?: number;
}

function validErrorResponse(
  value: unknown,
  expectedCode?: string,
): boolean {
  try {
    validateNodeErrorPayload(value, expectedCode);
    return true;
  } catch {
    return false;
  }
}

function canonicalShotId(value: string): ShotId {
  try {
    return validateShotId(value);
  } catch {
    throw new NodeClientError(
      "invalid-shot-id",
      "The Shot ID is invalid.",
    );
  }
}

function shotPathId(value: string): string {
  return encodeURIComponent(canonicalShotId(value));
}

export class HttpNodeClient implements NodeClient {
  readonly #baseUrl: URL;
  readonly #fetch: NodeFetch;
  readonly #maxResponseBytes: number;
  readonly #timeoutMs: number;

  constructor(
    baseUrl: string | URL,
    options: HttpNodeClientOptions = {},
  ) {
    this.#baseUrl = nodeBaseUrl(baseUrl);
    this.#fetch = options.fetch ?? fetch;
    const maximum = options.maxResponseBytes ??
      DEFAULT_MAX_NODE_RESPONSE_BYTES;
    if (!Number.isSafeInteger(maximum) || maximum < 1) {
      throw new NodeClientError(
        "invalid-response",
        "The node client response limit is invalid.",
      );
    }
    this.#maxResponseBytes = maximum;
    const timeout = options.timeoutMs ?? DEFAULT_NODE_REQUEST_TIMEOUT_MS;
    if (!Number.isSafeInteger(timeout) || timeout < 1) {
      throw new NodeClientError(
        "invalid-response",
        "The node client request timeout is invalid.",
      );
    }
    this.#timeoutMs = timeout;
  }

  async submit(
    recordValue: SignedPublicShotRecord,
  ): Promise<RegistryAppendResult> {
    let record: SignedPublicShotRecord;
    try {
      record = parseSignedPublicShotRecord(recordValue);
    } catch {
      throw new NodeClientError(
        "invalid-response",
        "The signed public record is invalid.",
      );
    }
    const body = canonicalJson(record);
    if (Buffer.byteLength(body) > CLIENT_MAX_RECORD_BYTES) {
      throw new NodeClientError(
        "invalid-response",
        "The signed public record exceeds the client limit.",
      );
    }
    const submission = await this.#request(
      new URL("/v1/records", this.#baseUrl),
      {
        method: "POST",
        headers: {
          "Accept": "application/json",
          "Content-Type": "application/json; charset=utf-8",
        },
        body,
      },
    );
    let payload;
    try {
      payload = validateNodeSubmissionPayload(submission.value);
    } catch {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid submission response.",
      );
    }
    if (
      (payload.status === "appended" && submission.status !== 201) ||
      (payload.status === "existing" && submission.status !== 200) ||
      payload.recordHash !== hashSignedPublicShotRecord(record)
    ) {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid submission response.",
      );
    }
    if (
      payload.projection.shotId !== record.shotId ||
      payload.projection.recordCount <= record.sequence
    ) {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an unrelated public projection.",
        submission.status,
      );
    }
    return {
      status: payload.status,
      recordHash: payload.recordHash,
      projection: payload.projection,
    };
  }

  async getProjection(
    shotId: string,
  ): Promise<PublicShotProjection | undefined> {
    const expectedShotId = canonicalShotId(shotId);
    const response = await this.#fetchResponse(
      new URL(`/v1/shots/${shotPathId(shotId)}`, this.#baseUrl),
      { method: "GET", headers: { "Accept": "application/json" } },
    );
    const value = await boundedJsonResponse(
      response,
      this.#maxResponseBytes,
    );
    if (response.status === 404) {
      if (!validErrorResponse(value, "shot-not-found")) {
        throw new NodeClientError(
          "invalid-response",
          "The reference node returned an invalid response.",
          response.status,
        );
      }
      return undefined;
    }
    if (!response.ok) {
      if (!validErrorResponse(value)) {
        throw new NodeClientError(
          "invalid-response",
          "The reference node returned an invalid response.",
          response.status,
        );
      }
      throw nodeFailure(response.status);
    }
    if (response.status !== 200) {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid response.",
        response.status,
      );
    }
    try {
      const projection = validateNodeProjectionPayload(value);
      if (projection.shotId !== expectedShotId) {
        throw new Error("projection Shot ID mismatch");
      }
      return projection;
    } catch {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid public projection.",
        response.status,
      );
    }
  }

  async getRecords(
    shotId: string,
  ): Promise<readonly SignedPublicShotRecord[]> {
    const expectedShotId = canonicalShotId(shotId);
    const response = await this.#fetchResponse(
      new URL(
        `/v1/shots/${shotPathId(shotId)}/records`,
        this.#baseUrl,
      ),
      { method: "GET", headers: { "Accept": "application/json" } },
    );
    const value = await boundedJsonResponse(
      response,
      this.#maxResponseBytes,
    );
    if (response.status === 404) {
      if (!validErrorResponse(value, "shot-not-found")) {
        throw new NodeClientError(
          "invalid-response",
          "The reference node returned an invalid response.",
          response.status,
        );
      }
      return [];
    }
    if (!response.ok) {
      if (!validErrorResponse(value)) {
        throw new NodeClientError(
          "invalid-response",
          "The reference node returned an invalid response.",
          response.status,
        );
      }
      throw nodeFailure(response.status);
    }
    if (response.status !== 200) {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid response.",
        response.status,
      );
    }
    try {
      const payload = validateNodeRecordsPayload(value);
      if (payload.records[0]?.shotId !== expectedShotId) {
        throw new Error("record chain Shot ID mismatch");
      }
      return payload.records;
    } catch {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid public record.",
        response.status,
      );
    }
  }

  async #request(
    url: URL,
    init: RequestInit,
  ): Promise<{ value: unknown; status: number }> {
    const response = await this.#fetchResponse(url, init);
    const value = await boundedJsonResponse(
      response,
      this.#maxResponseBytes,
    );
    if (!response.ok) {
      if (!validErrorResponse(value)) {
        throw new NodeClientError(
          "invalid-response",
          "The reference node returned an invalid response.",
          response.status,
        );
      }
      throw nodeFailure(response.status);
    }
    return { value, status: response.status };
  }

  async #fetchResponse(
    url: URL,
    init: RequestInit,
  ): Promise<Response> {
    return await safeFetch(
      this.#fetch,
      url,
      init,
      this.#timeoutMs,
    );
  }
}
