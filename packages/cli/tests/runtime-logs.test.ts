import { describe, expect, test } from "bun:test";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  appendStructuredLog,
  capRuntimeLog,
  MAX_CAPTURED_OUTPUT_BYTES,
  MAX_RUNTIME_LOG_BYTES,
  MAX_TAIL_READ_BYTES,
  tailLines,
  runCaptured,
} from "../factory/runtime/shared.ts";
import { withScratchEnvironment } from "./helpers.ts";

describe("bounded runtime logs", () => {
  test("tail reads only a bounded suffix and returns complete final lines", async () => {
    await withScratchEnvironment((scratch) => {
      const log = join(scratch.root, "large.log");
      writeFileSync(
        log,
        `${"x".repeat(MAX_TAIL_READ_BYTES + 1_024)}\nsecond-last\nlast\n`,
      );

      expect(tailLines(log, 2)).toEqual(["second-last", "last"]);
    });
  });

  test("rotation caps files in place and structured appends remain private", async () => {
    await withScratchEnvironment((scratch) => {
      const log = join(scratch.root, "bounded.log");
      writeFileSync(log, "x".repeat(MAX_RUNTIME_LOG_BYTES + 1));

      expect(capRuntimeLog(log)).toBe(true);
      expect(statSync(log).size).toBeLessThan(MAX_RUNTIME_LOG_BYTES);
      expect(readFileSync(log, "utf8")).toContain('"event":"log_rotated"');

      appendStructuredLog(log, { event: "synthetic", status: "ok" });
      expect(readFileSync(log, "utf8")).toContain('"event":"synthetic"');
      expect(statSync(log).mode & 0o777).toBe(0o600);
    });
  });

  test("captured child output cannot exhaust machine memory", async () => {
    await withScratchEnvironment(async (scratch) => {
      await expect(runCaptured(
        [
          process.execPath,
          "-e",
          `process.stdout.write("x".repeat(${MAX_CAPTURED_OUTPUT_BYTES + 1}))`,
        ],
        { cwd: scratch.root },
      )).rejects.toThrow("output exceeded");
    });
  });
});
