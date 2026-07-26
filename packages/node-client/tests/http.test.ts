import { describe, expect, test } from "bun:test";
import {
  boundedJsonResponse,
  nodeBaseUrl,
  NodeClientError,
  safeFetch,
} from "../src/http.ts";

describe("node client HTTP boundary", () => {
  test("requires a caller-supplied bare HTTP(S) origin", () => {
    expect(nodeBaseUrl("https://node.example")).toEqual(
      new URL("https://node.example"),
    );
    for (const value of [
      "",
      "/relative",
      "ftp://node.example",
      "https://user:secret@node.example",
      "https://node.example/path",
      "https://node.example?query=private",
      "https://node.example/#fragment",
    ]) {
      expect(() => nodeBaseUrl(value)).toThrow(NodeClientError);
    }
  });

  test("bounds response bytes and rejects malformed content length", async () => {
    await expect(
      boundedJsonResponse(
        new Response(JSON.stringify({ value: "x".repeat(100) }), {
          headers: { "Content-Type": "application/json" },
        }),
        32,
      ),
    ).rejects.toMatchObject({ code: "response-too-large" });

    await expect(
      boundedJsonResponse(
        new Response("{}", {
          headers: {
            "Content-Length": "not-a-number",
            "Content-Type": "application/json",
          },
        }),
      ),
    ).rejects.toMatchObject({ code: "invalid-response" });
  });

  test("does not expose response content or fetch errors", async () => {
    const privateValue = "private-record-content";
    await expect(
      boundedJsonResponse(
        new Response(privateValue, {
          status: 500,
          headers: { "Content-Type": "application/json" },
        }),
      ),
    ).rejects.not.toThrow(privateValue);

    await expect(
      safeFetch(
        async () => {
          throw new Error(privateValue);
        },
        new URL("https://node.example/v1/records"),
        { method: "POST", body: privateValue },
      ),
    ).rejects.toMatchObject({
      code: "network-error",
      message: "The reference node could not be reached.",
    });
  });

  test("times out even when an injected transport ignores abort", async () => {
    await expect(
      safeFetch(
        async () => await new Promise<Response>(() => undefined),
        new URL("https://node.example/v1/records"),
        { method: "GET" },
        10,
      ),
    ).rejects.toMatchObject({
      code: "request-timeout",
      message: "The reference node request timed out.",
    });
  });

  test("keeps the deadline active after headers until the body ends", async () => {
    const response = await safeFetch(
      async () => new Response(
        new ReadableStream<Uint8Array>({
          start() {
            // Deliberately never enqueue or close.
          },
        }),
        { headers: { "Content-Type": "application/json" } },
      ),
      new URL("https://node.example/v1/records"),
      { method: "GET" },
      10,
    );
    await expect(boundedJsonResponse(response)).rejects.toMatchObject({
      code: "request-timeout",
    });
  });

  test("disables credentials and redirects for signed record requests", async () => {
    let captured: RequestInit | undefined;
    await safeFetch(
      async (_input, init) => {
        captured = init;
        return new Response("{}", {
          headers: { "Content-Type": "application/json" },
        });
      },
      new URL("https://node.example/v1/records"),
      { method: "POST", credentials: "include", redirect: "follow" },
    );
    expect(captured?.credentials).toBe("omit");
    expect(captured?.redirect).toBe("error");
  });
});
