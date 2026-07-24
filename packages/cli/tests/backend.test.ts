import { afterEach, describe, expect, test } from "bun:test";
import {
  existsSync,
  linkSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Database } from "bun:sqlite";
import { startShotApi, type RunningShotApi } from "../../../templates/continuity-app/Backend/server.ts";

const roots: string[] = [];
const running: RunningShotApi[] = [];

afterEach(async () => {
  await Promise.all(running.splice(0).map((api) => api.stop()));
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe("shot API foundation", () => {
  test("binds to an assigned localhost port, migrates SQLite, and reports non-sensitive health", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno backend with spaces-"));
    roots.push(root);
    mkdirSync(join(root, ".tohseno"), { recursive: true });
    writeFileSync(
      join(root, ".tohseno", "shot.json"),
      `${JSON.stringify({
        slug: "private-product-slug",
        platform: "ios",
        factory: {
          releaseId: `content-${"a".repeat(32)}`,
          templateVersion: "0.4.0",
        },
      })}\n`,
      { mode: 0o600 },
    );
    const databasePath = join(root, "persistent data", "development.sqlite3");
    const readyFile = join(root, "runtime", "ready.json");
    const records: Array<Record<string, unknown>> = [];
    const api = await startShotApi({
      root,
      environment: { ...process.env, TOHSENO_DATABASE_PATH: databasePath },
      readyFile,
      instanceId: "test-instance",
      log: (record) => records.push(record),
    });
    running.push(api);

    expect(api.hostname).toBe("127.0.0.1");
    expect(api.port).toBeGreaterThan(0);
    expect(existsSync(readyFile)).toBe(true);
    const response = await fetch(`${api.origin}/health`);
    expect(response.status).toBe(200);
    const health = await response.json() as Record<string, unknown>;
    expect(health).toMatchObject({
      schemaVersion: 1,
      status: "ok",
      ready: true,
      service: "shot-api",
      shot: { platform: "ios" },
      runtime: { databaseSchemaVersion: 1, persistence: "sqlite" },
    });
    expect(JSON.stringify(health)).not.toContain(databasePath);
    expect(JSON.stringify(health)).not.toContain("private-product-slug");
    expect(health.shot).not.toHaveProperty("slug");

    const database = new Database(databasePath, { readonly: true });
    expect(database.query<{ version: number }, []>("SELECT version FROM schema_migrations").get()?.version).toBe(1);
    expect(database.query<{ name: string }, []>("SELECT name FROM sqlite_master WHERE name = 'runtime_metadata'").get()?.name).toBe("runtime_metadata");
    database.close();
    expect(statSync(databasePath).mode & 0o077).toBe(0);

    expect((await fetch(`${api.origin}/private`)).status).toBe(404);
    expect((await fetch(`${api.origin}/health`, { method: "POST" })).status).toBe(405);
    expect(records.every((record) => !("body" in record) && !("headers" in record))).toBe(true);
    expect(JSON.stringify(records)).not.toContain("/private");
    expect(records.some((record) => record.method === "OTHER")).toBe(true);

    await api.stop();
    running.pop();
    expect(existsSync(databasePath)).toBe(true);
    expect(existsSync(readyFile)).toBe(false);
  });

  test("refuses non-local development binding and unresolved production persistence", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno backend boundary-"));
    roots.push(root);
    await expect(startShotApi({
      root,
      hostname: "0.0.0.0",
      environment: { ...process.env, NODE_ENV: "development" },
      log: () => {},
    })).rejects.toThrow("must bind to localhost");
    await expect(startShotApi({
      root,
      environment: { ...process.env, NODE_ENV: "production", TOHSENO_DATABASE_PATH: undefined },
      log: () => {},
    })).rejects.toThrow("absolute TOHSENO_DATABASE_PATH");

    const outside = join(tmpdir(), `tohseno-outside-${crypto.randomUUID()}.sqlite3`);
    await expect(startShotApi({
      root,
      environment: {
        ...process.env,
        NODE_ENV: "development",
        TOHSENO_DATABASE_PATH: outside,
      },
      log: () => {},
    })).rejects.toThrow("must remain inside the shot");
    expect(existsSync(outside)).toBe(false);

    const redirected = join(root, "redirected-data");
    const outsideDirectory = mkdtempSync(join(tmpdir(), "tohseno-db-victim-"));
    roots.push(outsideDirectory);
    symlinkSync(outsideDirectory, redirected, "dir");
    await expect(startShotApi({
      root,
      environment: {
        ...process.env,
        NODE_ENV: "development",
        TOHSENO_DATABASE_PATH: join(redirected, "development.sqlite3"),
      },
      log: () => {},
    })).rejects.toThrow("must remain inside the shot");
    expect(existsSync(join(outsideDirectory, "development.sqlite3"))).toBe(false);
  });

  test("refuses database and SQLite auxiliary-file symlinks without touching their targets", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno backend links-"));
    roots.push(root);
    const data = join(root, "data");
    mkdirSync(data);
    const victim = join(root, "owner-file");
    writeFileSync(victim, "do not touch", { mode: 0o640 });
    const databasePath = join(data, "development.sqlite3");
    symlinkSync(victim, databasePath);

    await expect(startShotApi({
      root,
      environment: { ...process.env, TOHSENO_DATABASE_PATH: databasePath },
      log: () => {},
    })).rejects.toThrow("regular file");
    expect(readFileSync(victim, "utf8")).toBe("do not touch");
    expect(statSync(victim).mode & 0o777).toBe(0o640);

    rmSync(databasePath);
    symlinkSync(victim, `${databasePath}-wal`);
    await expect(startShotApi({
      root,
      environment: { ...process.env, TOHSENO_DATABASE_PATH: databasePath },
      log: () => {},
    })).rejects.toThrow("regular file");
    expect(readFileSync(victim, "utf8")).toBe("do not touch");

    rmSync(`${databasePath}-wal`);
    rmSync(databasePath);
    linkSync(victim, databasePath);
    await expect(startShotApi({
      root,
      environment: { ...process.env, TOHSENO_DATABASE_PATH: databasePath },
      log: () => {},
    })).rejects.toThrow("single-link regular file");
    expect(readFileSync(victim, "utf8")).toBe("do not touch");
    expect(statSync(victim).mode & 0o777).toBe(0o640);
  });
});
