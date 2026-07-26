import { afterEach, describe, expect, test } from "bun:test";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  acquireShotEvolutionLock,
  advanceLocalShotEvolution,
  createShotProtocolPointer,
  initialShotProtocolState,
  readLocalShotProtocolState,
  releaseShotEvolutionLock,
  writeLocalShotProtocolState,
} from "../src/protocol-state.ts";

const scratchDirectories: string[] = [];

afterEach(() => {
  for (const directory of scratchDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

function shotRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "tohseno-protocol-state-"));
  scratchDirectories.push(root);
  mkdirSync(join(root, ".tohseno"));
  return root;
}

describe("local Shot protocol metadata", () => {
  test("serializes the whole Evolution session and preserves increments", () => {
    const root = shotRoot();
    const pointer = createShotProtocolPointer();
    writeLocalShotProtocolState(
      root,
      initialShotProtocolState(pointer.shotId),
    );

    const first = acquireShotEvolutionLock(root);
    expect(() => acquireShotEvolutionLock(root)).toThrow(
      "already evolving this Shot",
    );
    expect(
      advanceLocalShotEvolution(root, pointer, first).evolution,
    ).toBe(1);
    releaseShotEvolutionLock(first);

    const second = acquireShotEvolutionLock(root);
    expect(
      advanceLocalShotEvolution(root, pointer, second).evolution,
    ).toBe(2);
    releaseShotEvolutionLock(second);
    expect(readLocalShotProtocolState(root)?.evolution).toBe(2);
  });

  test("cannot assert a public lifecycle in local state", () => {
    const root = shotRoot();
    const pointer = createShotProtocolPointer();
    writeFileSync(
      join(root, ".tohseno", "protocol-state.json"),
      `${JSON.stringify({
        protocolVersion: 1,
        shotId: pointer.shotId,
        lifecycle: "APP_STORE",
        evolution: 999,
      })}\n`,
    );

    expect(() => readLocalShotProtocolState(root)).toThrow(
      "local Shot protocol state is invalid",
    );
  });

  test("refuses a counter rewritten during an Evolution session", () => {
    const root = shotRoot();
    const pointer = createShotProtocolPointer();
    writeLocalShotProtocolState(
      root,
      initialShotProtocolState(pointer.shotId),
    );

    const lock = acquireShotEvolutionLock(root);
    writeLocalShotProtocolState(root, {
      ...initialShotProtocolState(pointer.shotId),
      evolution: 999,
    });
    expect(() =>
      advanceLocalShotEvolution(root, pointer, lock)
    ).toThrow("refusing to record a forged Evolution");
    releaseShotEvolutionLock(lock);
    expect(readLocalShotProtocolState(root)?.evolution).toBe(999);
  });
});
