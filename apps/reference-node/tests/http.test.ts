import { describe, expect, test } from "bun:test";
import {
  jsonResponse,
  parseJsonBody,
  readBoundedRequestBody,
  ReferenceNodeHttpError,
} from "../src/http.ts";

describe("reference node HTTP bounds", () => {
  test("reads exact bounded request bytes", async () => {
    const request = new Request("http://127.0.0.1/v1/records", {
      method: "POST",
      body: "{\"public\":true}",
      headers: {
        "Content-Length": "15",
        "Content-Type": "application/json",
      },
    });
    const bytes = await readBoundedRequestBody(request, 15);
    expect(new TextDecoder().decode(bytes)).toBe("{\"public\":true}");
    expect(parseJsonBody(bytes)).toEqual({ public: true });
  });

  test("rejects declared and streamed oversize bodies", async () => {
    const declared = new Request("http://127.0.0.1/v1/records", {
      method: "POST",
      body: "12345",
      headers: { "Content-Length": "5" },
    });
    await expect(readBoundedRequestBody(declared, 4)).rejects.toMatchObject({
      status: 413,
      code: "record-too-large",
    });

    const streamed = new Request("http://127.0.0.1/v1/records", {
      method: "POST",
      body: new ReadableStream<Uint8Array>({
        start(controller) {
          controller.enqueue(new TextEncoder().encode("123"));
          controller.enqueue(new TextEncoder().encode("45"));
          controller.close();
        },
      }),
      duplex: "half",
    } as RequestInit & { duplex: "half" });
    await expect(readBoundedRequestBody(streamed, 4)).rejects.toMatchObject({
      status: 413,
      code: "record-too-large",
    });
  });

  test("rejects invalid UTF-8 and bounds JSON responses", () => {
    expect(() => parseJsonBody(Uint8Array.of(0xff))).toThrow(
      ReferenceNodeHttpError,
    );
    expect(() => jsonResponse("x".repeat(4 * 1024 * 1024))).toThrow(
      ReferenceNodeHttpError,
    );
  });
});
