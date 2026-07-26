import { describe, expect, test } from "bun:test";
import {
  methodClass,
  semanticRoute,
} from "../src/logging.ts";

describe("reference node semantic logging", () => {
  test("classifies routes without retaining identifiers", () => {
    const identifier = "shot_private_identifier_value";
    expect(semanticRoute(`/v1/shots/${identifier}`)).toBe("shot-projection");
    expect(semanticRoute(`/v1/shots/${identifier}/records`)).toBe(
      "shot-records",
    );
    expect(
      JSON.stringify([
        semanticRoute(`/v1/shots/${identifier}`),
        semanticRoute(`/v1/shots/${identifier}/records`),
      ]),
    ).not.toContain(identifier);
  });

  test("coarsens unsupported methods and arbitrary paths", () => {
    expect(methodClass("GET")).toBe("GET");
    expect(methodClass("DELETE")).toBe("OTHER");
    expect(semanticRoute("/credential-looking-path")).toBe("unmatched");
  });
});
