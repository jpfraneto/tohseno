export const MAX_RECORD_BODY_BYTES = 256 * 1024;
export const MAX_PUBLIC_RESPONSE_BYTES = 4 * 1024 * 1024;

export class ReferenceNodeHttpError extends Error {
  override readonly name = "ReferenceNodeHttpError";

  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

function contentLength(headers: Headers): number | null {
  const source = headers.get("content-length");
  if (source === null) return null;
  if (!/^(?:0|[1-9][0-9]{0,15})$/u.test(source)) {
    throw new ReferenceNodeHttpError(
      400,
      "invalid-content-length",
      "The request has an invalid content length.",
    );
  }
  const parsed = Number(source);
  if (!Number.isSafeInteger(parsed)) {
    throw new ReferenceNodeHttpError(
      400,
      "invalid-content-length",
      "The request has an invalid content length.",
    );
  }
  return parsed;
}

export async function readBoundedRequestBody(
  request: Request,
  maximumBytes = MAX_RECORD_BODY_BYTES,
): Promise<Uint8Array> {
  if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 0) {
    throw new RangeError("maximumBytes must be a non-negative safe integer");
  }
  const declared = contentLength(request.headers);
  if (declared !== null && declared > maximumBytes) {
    throw new ReferenceNodeHttpError(
      413,
      "record-too-large",
      "The public record exceeds the request limit.",
    );
  }
  if (request.body === null) return new Uint8Array();

  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      total += next.value.byteLength;
      if (total > maximumBytes) {
        await reader.cancel();
        throw new ReferenceNodeHttpError(
          413,
          "record-too-large",
          "The public record exceeds the request limit.",
        );
      }
      chunks.push(next.value);
    }
  } catch (error) {
    if (error instanceof ReferenceNodeHttpError) throw error;
    throw new ReferenceNodeHttpError(
      400,
      "invalid-body",
      "The request body could not be read.",
    );
  } finally {
    reader.releaseLock();
  }

  if (declared !== null && declared !== total) {
    throw new ReferenceNodeHttpError(
      400,
      "content-length-mismatch",
      "The request body does not match its declared length.",
    );
  }
  const result = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return result;
}

export function parseJsonBody(bytes: Uint8Array): unknown {
  let source: string;
  try {
    source = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new ReferenceNodeHttpError(
      400,
      "invalid-json",
      "The request body must be UTF-8 JSON.",
    );
  }
  try {
    return JSON.parse(source) as unknown;
  } catch {
    throw new ReferenceNodeHttpError(
      400,
      "invalid-json",
      "The request body must be valid JSON.",
    );
  }
}

function responseHeaders(): Headers {
  return new Headers({
    "Cache-Control": "no-store",
    "Content-Type": "application/json; charset=utf-8",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
  });
}

export function jsonResponse(value: unknown, status = 200): Response {
  const body = `${JSON.stringify(value)}\n`;
  if (Buffer.byteLength(body) > MAX_PUBLIC_RESPONSE_BYTES) {
    throw new ReferenceNodeHttpError(
      507,
      "response-too-large",
      "The requested public record set is too large.",
    );
  }
  return new Response(body, { status, headers: responseHeaders() });
}

export function emptyResponse(status: number): Response {
  return new Response(null, { status, headers: responseHeaders() });
}
