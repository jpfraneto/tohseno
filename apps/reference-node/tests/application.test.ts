import { describe, expect, test } from "bun:test";
import {
  localEd25519VerifierSet,
} from "../../../packages/signer/src/index.ts";
import {
  createReferenceNodeApplication,
  type ReferenceNodeLogger,
} from "../src/application.ts";
import {
  openReferenceNodeDatabase,
  REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
} from "../src/database.ts";
import { SqlitePublicRecordRegistry } from "../src/registry.ts";
import {
  appStoreRecord,
  completeRecordChain,
  createRecord,
  TEST_SHOT_ID,
  testBuilder,
} from "./fixtures.ts";

function request(
  path: string,
  init: RequestInit = {},
): Request {
  return new Request(`http://reference.invalid${path}`, init);
}

async function responseJson(response: Response): Promise<unknown> {
  return await response.json() as unknown;
}

describe("reference node Request-to-Response application", () => {
  test("serves health, OpenAPI, projections, and canonical record export", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
      );
      const fetch = createReferenceNodeApplication({
        registry,
        databaseSchemaVersion: REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
      });
      const health = await fetch(request("/healthz"));
      expect(health.status).toBe(200);
      expect(await responseJson(health)).toEqual({
        status: "ok",
        service: "tohseno-reference-node",
        databaseSchemaVersion: 1,
      });
      const openapi = await fetch(request("/openapi.json"));
      expect(openapi.status).toBe(200);
      expect(
        (await responseJson(openapi) as { openapi?: unknown }).openapi,
      ).toBe("3.1.0");

      const records = await completeRecordChain();
      for (const record of records) {
        const response = await fetch(request("/v1/records", {
          method: "POST",
          headers: { "Content-Type": "application/json; charset=utf-8" },
          body: JSON.stringify(record),
        }));
        expect(response.status).toBe(201);
        expect(response.headers.get("cache-control")).toBe("no-store");
      }

      const replay = await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(records[0]),
      }));
      expect(replay.status).toBe(200);
      expect(await responseJson(replay)).toMatchObject({
        schemaVersion: 1,
        status: "existing",
      });

      const projection = await fetch(request(`/v1/shots/${TEST_SHOT_ID}`));
      expect(projection.status).toBe(200);
      expect(await responseJson(projection)).toMatchObject({
        shotId: TEST_SHOT_ID,
        lifecycle: "APP_STORE",
        recordCount: 5,
      });
      const exported = await fetch(
        request(`/v1/shots/${TEST_SHOT_ID}/records`),
      );
      expect(exported.status).toBe(200);
      expect(await responseJson(exported)).toEqual({
        schemaVersion: 1,
        records,
      });
    } finally {
      opened.database.close();
    }
  });

  test("rejects private fields, signature mutations, forks, and bad lifecycle state", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const logs: Parameters<ReferenceNodeLogger>[0][] = [];
      let time = 10;
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
      );
      const fetch = createReferenceNodeApplication({
        registry,
        databaseSchemaVersion: 1,
        logger: (entry) => logs.push(entry),
        clockMilliseconds: () => time++,
      });
      const signer = testBuilder("application-errors");
      const created = await createRecord(signer);

      const privateShaped = {
        ...created,
        privatePrompt: "this must never be accepted or logged",
      };
      const privateResponse = await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(privateShaped),
      }));
      expect(privateResponse.status).toBe(400);
      expect(await responseJson(privateResponse)).toEqual({
        error: "invalid-record",
      });

      const mutated = structuredClone(created);
      if (mutated.kind !== "SHOT_CREATED") throw new Error("unreachable");
      mutated.body.summary = "A signature mutation that must not be logged.";
      const signatureResponse = await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(mutated),
      }));
      expect(signatureResponse.status).toBe(400);
      expect(await responseJson(signatureResponse)).toEqual({
        error: "invalid-record",
      });

      expect((await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(created),
      }))).status).toBe(201);

      const fork = await createRecord(
        signer,
        TEST_SHOT_ID,
        "A conflicting public fork.",
      );
      const forkResponse = await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(fork),
      }));
      expect(forkResponse.status).toBe(409);

      const invalidLifecycle = await appStoreRecord(signer, created);
      const lifecycleResponse = await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(invalidLifecycle),
      }));
      expect(lifecycleResponse.status).toBe(409);
      expect(registry.getRecords(TEST_SHOT_ID)).toEqual([created]);

      const serializedLogs = JSON.stringify(logs);
      expect(serializedLogs).not.toContain(TEST_SHOT_ID);
      expect(serializedLogs).not.toContain("privatePrompt");
      expect(serializedLogs).not.toContain("signature mutation");
      expect(logs.every((entry) => entry.route === "submit-record")).toBe(
        true,
      );
    } finally {
      opened.database.close();
    }
  });

  test("enforces mutation syntax and exact route methods", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const fetch = createReferenceNodeApplication({
        registry: new SqlitePublicRecordRegistry(
          opened.database,
          localEd25519VerifierSet(),
        ),
        databaseSchemaVersion: 1,
      });
      const query = await fetch(request("/v1/records?source=private", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: "{}",
      }));
      expect(query.status).toBe(400);
      expect(await responseJson(query)).toEqual({
        error: "query-not-allowed",
      });

      const media = await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "text/plain" },
        body: "{}",
      }));
      expect(media.status).toBe(415);
      const encoded = await fetch(request("/v1/records", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Content-Encoding": "gzip",
        },
        body: "{}",
      }));
      expect(encoded.status).toBe(415);

      const wrongMethod = await fetch(request("/healthz", {
        method: "POST",
      }));
      expect(wrongMethod.status).toBe(405);
      expect(wrongMethod.headers.get("allow")).toBe("GET");

      const invalidId = await fetch(request("/v1/shots/not-a-shot"));
      expect(invalidId.status).toBe(404);
      expect(await responseJson(invalidId)).toEqual({
        error: "shot-not-found",
      });
      const missing = await fetch(
        request(`/v1/shots/${`shot_${"Z".repeat(32)}`}/records`),
      );
      expect(missing.status).toBe(404);
    } finally {
      opened.database.close();
    }
  });

  test("reports bounded adapter capacity without accepting an unexportable history", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
        () => new Date("2026-07-25T12:00:00.000Z"),
        { maxRecordsPerShot: 1 },
      );
      const fetch = createReferenceNodeApplication({
        registry,
        databaseSchemaVersion: 1,
      });
      const records = await completeRecordChain();
      expect((await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(records[0]),
      }))).status).toBe(201);
      const capacity = await fetch(request("/v1/records", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(records[1]),
      }));
      expect(capacity.status).toBe(507);
      expect(await responseJson(capacity)).toEqual({
        error: "capacity-exceeded",
      });
      expect(registry.getRecords(TEST_SHOT_ID)).toHaveLength(1);
    } finally {
      opened.database.close();
    }
  });
});
