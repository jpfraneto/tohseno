export const DEFAULT_MAX_NODE_RESPONSE_BYTES = 4 * 1024 * 1024;
export const DEFAULT_NODE_REQUEST_TIMEOUT_MS = 15_000;

export type NodeFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

export class NodeClientError extends Error {
  override readonly name = "NodeClientError";

  constructor(
    readonly code:
      | "invalid-base-url"
      | "invalid-shot-id"
      | "network-error"
      | "request-timeout"
      | "node-error"
      | "response-too-large"
      | "invalid-response",
    message: string,
    readonly status?: number,
  ) {
    super(message);
  }
}

export function nodeBaseUrl(value: string | URL): URL {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new NodeClientError(
      "invalid-base-url",
      "The reference node URL must be an absolute HTTP(S) origin.",
    );
  }
  if (
    (parsed.protocol !== "http:" && parsed.protocol !== "https:") ||
    parsed.username !== "" ||
    parsed.password !== "" ||
    parsed.pathname !== "/" ||
    parsed.search !== "" ||
    parsed.hash !== ""
  ) {
    throw new NodeClientError(
      "invalid-base-url",
      "The reference node URL must be a bare HTTP(S) origin.",
    );
  }
  return parsed;
}

async function boundedBytes(
  response: Response,
  maximumBytes: number,
): Promise<Uint8Array> {
  if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1) {
    throw new NodeClientError(
      "invalid-response",
      "The node client response limit is invalid.",
    );
  }
  const declared = response.headers.get("content-length");
  let declaredBytes: number | null = null;
  if (declared !== null) {
    if (!/^(?:0|[1-9][0-9]{0,15})$/u.test(declared)) {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid response.",
        response.status,
      );
    }
    declaredBytes = Number(declared);
    if (!Number.isSafeInteger(declaredBytes)) {
      throw new NodeClientError(
        "invalid-response",
        "The reference node returned an invalid response.",
        response.status,
      );
    }
    if (declaredBytes > maximumBytes) {
      throw new NodeClientError(
        "response-too-large",
        "The reference node response exceeded the client limit.",
        response.status,
      );
    }
  }
  if (response.body === null) return new Uint8Array();

  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      total += next.value.byteLength;
      if (total > maximumBytes) {
        await reader.cancel();
        throw new NodeClientError(
          "response-too-large",
          "The reference node response exceeded the client limit.",
          response.status,
        );
      }
      chunks.push(next.value);
    }
  } catch (error) {
    if (error instanceof NodeClientError) throw error;
    throw new NodeClientError(
      "invalid-response",
      "The reference node returned an unreadable response.",
      response.status,
    );
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  if (declaredBytes !== null && declaredBytes !== total) {
    throw new NodeClientError(
      "invalid-response",
      "The reference node returned an invalid response.",
      response.status,
    );
  }
  return bytes;
}

function contentTypeIsJson(response: Response): boolean {
  const mediaType = response.headers
    .get("content-type")
    ?.split(";", 1)[0]
    ?.trim()
    .toLowerCase();
  return mediaType === "application/json" ||
    mediaType === "application/problem+json";
}

export async function boundedJsonResponse(
  response: Response,
  maximumBytes = DEFAULT_MAX_NODE_RESPONSE_BYTES,
): Promise<unknown> {
  if (!contentTypeIsJson(response)) {
    throw new NodeClientError(
      "invalid-response",
      "The reference node returned an unsupported response.",
      response.status,
    );
  }
  const bytes = await boundedBytes(response, maximumBytes);
  let source: string;
  try {
    source = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new NodeClientError(
      "invalid-response",
      "The reference node returned an invalid response.",
      response.status,
    );
  }
  try {
    return JSON.parse(source) as unknown;
  } catch {
    throw new NodeClientError(
      "invalid-response",
      "The reference node returned an invalid response.",
      response.status,
    );
  }
}

export function nodeFailure(status: number): NodeClientError {
  return new NodeClientError(
    "node-error",
    "The reference node could not complete the request.",
    status,
  );
}

export async function safeFetch(
  fetcher: NodeFetch,
  input: URL,
  init: RequestInit,
  timeoutMs = DEFAULT_NODE_REQUEST_TIMEOUT_MS,
): Promise<Response> {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1) {
    throw new NodeClientError(
      "invalid-response",
      "The node client request timeout is invalid.",
    );
  }
  const controller = new AbortController();
  const callerSignal = init.signal;
  const timeoutError = new NodeClientError(
    "request-timeout",
    "The reference node request timed out.",
  );
  let responseReader: ReadableStreamDefaultReader<Uint8Array> | undefined;
  let responseController:
    | ReadableStreamDefaultController<Uint8Array>
    | undefined;
  let expired = false;
  let settled = false;
  let rejectDeadline: ((error: NodeClientError) => void) | undefined;
  const cleanup = (): void => {
    if (settled) return;
    settled = true;
    clearTimeout(timer);
    callerSignal?.removeEventListener("abort", abortFromCaller);
  };
  const failDeadline = (error: NodeClientError): void => {
    if (settled) return;
    expired = true;
    controller.abort();
    rejectDeadline?.(error);
    try {
      responseController?.error(error);
    } catch {
      // The response stream may already be closed.
    }
    if (responseReader !== undefined) {
      void responseReader.cancel(error).catch(() => undefined);
    }
  };
  const abortFromCaller = (): void => failDeadline(new NodeClientError(
    "network-error",
    "The reference node request was cancelled.",
  ));
  const timer = setTimeout(() => failDeadline(timeoutError), timeoutMs);
  const deadline = new Promise<Response>((_resolve, reject) => {
    rejectDeadline = reject;
  });
  if (callerSignal?.aborted) abortFromCaller();
  else callerSignal?.addEventListener("abort", abortFromCaller, { once: true });
  try {
    const request = fetcher(input, {
      ...init,
      credentials: "omit",
      redirect: "error",
      signal: controller.signal,
    });
    const response = await Promise.race([request, deadline]);
    if (response.body === null) {
      cleanup();
      return response;
    }
    responseReader = response.body.getReader();
    const body = new ReadableStream<Uint8Array>({
      start(streamController) {
        responseController = streamController;
        if (expired) streamController.error(timeoutError);
      },
      async pull(streamController) {
        if (expired) return;
        try {
          const next = await responseReader!.read();
          if (next.done) {
            cleanup();
            streamController.close();
          } else {
            streamController.enqueue(next.value);
          }
        } catch (error) {
          cleanup();
          streamController.error(
            error instanceof NodeClientError ? error : new NodeClientError(
              "network-error",
              "The reference node response could not be read.",
            ),
          );
        }
      },
      async cancel(reason) {
        cleanup();
        await responseReader!.cancel(reason);
      },
    });
    return new Response(body, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });
  } catch (error) {
    cleanup();
    if (error instanceof NodeClientError) throw error;
    throw new NodeClientError(
      "network-error",
      "The reference node could not be reached.",
    );
  }
}
