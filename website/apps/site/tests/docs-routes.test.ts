import { describe, expect, test } from "bun:test";
import { createApplication } from "../server.ts";
import { loadConfig } from "../config.ts";

async function application() {
  return createApplication({
    config: loadConfig({
      NODE_ENV: "test",
      PORT: "3000",
      BASE_URL: "http://localhost:3000",
    }),
  });
}

describe("documentation handoff", () => {
  test("redirects the main-site docs route to the standalone docs site", async () => {
    const app = await application();
    const response = await app.fetch(
      new Request("http://localhost:3000/docs"),
    );
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("https://docs.tohseno.com/");
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=300");
    expect(await response.text()).toBe("");
  });

  test("keeps the main-domain root on the landing page", async () => {
    const app = await application();
    const response = await app.fetch(new Request("http://localhost:3000/"));
    expect(response.status).toBe(200);
    expect(await response.text()).not.toContain('id="course-title"');
  });

  test("supports a bodyless HEAD redirect", async () => {
    const app = await application();
    const response = await app.fetch(
      new Request("http://localhost:3000/docs", { method: "HEAD" }),
    );
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("https://docs.tohseno.com/");
    expect(await response.text()).toBe("");
  });

  test("refuses mutation on the redirect route", async () => {
    const app = await application();
    const response = await app.fetch(
      new Request("http://localhost:3000/docs", { method: "POST" }),
    );
    expect(response.status).toBe(405);
    expect(response.headers.get("Allow")).toBe("GET, HEAD");
  });
});
