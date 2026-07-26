import { afterEach, describe, expect, test } from "bun:test";
import { InMemoryRegistry } from "../../registry/src/index.ts";
import {
  hashSignedPublicShotRecord,
} from "../../protocol/src/index.ts";
import {
  localEd25519VerifierSet,
} from "../../signer/src/index.ts";
import {
  completeRecordChain,
  TEST_SHOT_ID,
} from "../../../apps/reference-node/tests/fixtures.ts";
import {
  startReferenceNode,
  type RunningReferenceNode,
} from "../../../apps/reference-node/server.ts";
import {
  HttpNodeClient,
  NodeClientError,
  type NodeFetch,
} from "../src/index.ts";

const runningNodes: RunningReferenceNode[] = [];

afterEach(async () => {
  for (const running of runningNodes.splice(0)) {
    await running.stop();
  }
});

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

describe("HTTP node client", () => {
  test("submits and exports a complete chain through a live Bun node", async () => {
    const running = startReferenceNode({
      hostname: "127.0.0.1",
      port: 0,
      databasePath: ":memory:",
      logger: () => undefined,
    });
    runningNodes.push(running);
    const client = new HttpNodeClient(running.origin);
    const records = await completeRecordChain();

    for (const record of records) {
      const result = await client.submit(record);
      expect(result.status).toBe("appended");
      expect(result.recordHash).toBe(hashSignedPublicShotRecord(record));
    }
    expect((await client.submit(records[0]!)).status).toBe("existing");
    expect(await client.getRecords(TEST_SHOT_ID)).toEqual(records);
    expect(await client.getProjection(TEST_SHOT_ID)).toMatchObject({
      shotId: TEST_SHOT_ID,
      lifecycle: "APP_STORE",
      evolution: 1,
      recordCount: 5,
    });
    expect(
      await client.getProjection(`shot_${"Q".repeat(32)}`),
    ).toBeUndefined();
    expect(
      await client.getRecords(`shot_${"Q".repeat(32)}`),
    ).toEqual([]);
  });

  test("uses injected fetch and rejects extra submission fields", async () => {
    const [record] = await completeRecordChain();
    if (record === undefined) throw new Error("missing fixture");
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    const appended = await registry.append(record);
    let captured: { input: RequestInfo | URL; init?: RequestInit };
    const injected: NodeFetch = async (input, init) => {
      captured = { input, ...(init === undefined ? {} : { init }) };
      return json({
        schemaVersion: 1,
        status: "appended",
        recordHash: appended.recordHash,
        projection: appended.projection,
        unexpected: "field",
      }, 201);
    };
    const client = new HttpNodeClient("https://replaceable.example", {
      fetch: injected,
    });
    await expect(client.submit(record)).rejects.toMatchObject({
      code: "invalid-response",
    });
    expect(String(captured!.input)).toBe(
      "https://replaceable.example/v1/records",
    );
    expect(captured!.init?.credentials).toBe("omit");
    expect(captured!.init?.redirect).toBe("error");
  });

  test("rejects unrelated projections and broken record chains", async () => {
    const records = await completeRecordChain();
    const registry = new InMemoryRegistry(localEd25519VerifierSet());
    for (const record of records) await registry.append(record);
    const projection = registry.getProjection(TEST_SHOT_ID);
    if (projection === undefined) throw new Error("missing fixture projection");

    const unrelatedClient = new HttpNodeClient(
      "https://replaceable.example",
      {
        fetch: async () =>
          json({
            ...projection,
            shotId: `shot_${"Q".repeat(32)}`,
          }),
      },
    );
    await expect(
      unrelatedClient.getProjection(TEST_SHOT_ID),
    ).rejects.toMatchObject({ code: "invalid-response" });

    const broken = structuredClone(records);
    const second = broken[1];
    if (second === undefined) throw new Error("missing fixture record");
    second.previousRecordHash = `sha256:${"0".repeat(64)}`;
    const brokenClient = new HttpNodeClient(
      "https://replaceable.example",
      {
        fetch: async () => json({ schemaVersion: 1, records: broken }),
      },
    );
    await expect(
      brokenClient.getRecords(TEST_SHOT_ID),
    ).rejects.toMatchObject({ code: "invalid-response" });
  });

  test("rejects noncanonical Shot IDs before making a request", async () => {
    let called = false;
    const client = new HttpNodeClient("https://replaceable.example", {
      fetch: async () => {
        called = true;
        return json({});
      },
    });
    await expect(client.getProjection("not-a-shot")).rejects.toBeInstanceOf(
      NodeClientError,
    );
    expect(called).toBe(false);
  });
});
