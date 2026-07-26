import { describe, expect, test } from "bun:test";
import {
  DEFAULT_REFERENCE_NODE_HOST,
  DEFAULT_REFERENCE_NODE_PORT,
  loadReferenceNodeConfig,
} from "../src/config.ts";

describe("reference node configuration", () => {
  test("has a loopback default and no public host default", () => {
    const config = loadReferenceNodeConfig({});
    expect(config.hostname).toBe(DEFAULT_REFERENCE_NODE_HOST);
    expect(config.port).toBe(DEFAULT_REFERENCE_NODE_PORT);
    expect(config.databasePath).toEndWith(
      "/apps/reference-node/data/reference-node.sqlite",
    );
  });

  test("accepts explicit local configuration and rejects malformed values", () => {
    expect(
      loadReferenceNodeConfig({
        TOHSENO_NODE_HOST: "localhost",
        TOHSENO_NODE_PORT: "9000",
        TOHSENO_NODE_DATABASE_PATH: "/tmp/tohseno-reference.sqlite",
      }),
    ).toEqual({
      hostname: "localhost",
      port: 9000,
      databasePath: "/tmp/tohseno-reference.sqlite",
    });
    expect(() =>
      loadReferenceNodeConfig({ TOHSENO_NODE_PORT: "0" })
    ).toThrow("whole number");
    expect(() =>
      loadReferenceNodeConfig({ TOHSENO_NODE_HOST: "host/path" })
    ).toThrow("invalid");
  });
});
