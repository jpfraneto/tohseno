import { describe, expect, test } from "bun:test";
import { handoffForShot, renderHandoff } from "../src/handoff.ts";
import { createMemoryIo } from "./helpers.ts";

describe("authoritative handoff", () => {
  test("renders evidence, one exact next action, and cwd-independent later commands", () => {
    const io = createMemoryIo(false);
    const handoff = handoffForShot({
      name: "Five Choices",
      slug: "five-choices",
      sequence: 7,
      path: "/private/shots/five-choices",
      skillCount: 4,
      verificationPassed: true,
      agentExitCode: 0,
      buildState: "completed",
      simulatorState: "completed",
      captureState: "completed",
    });
    renderHandoff(handoff, io, { NO_COLOR: "1", TERM: "xterm" });
    const output = io.stdout.join("\n");
    expect(output).toContain("TOHSENO / FIVE CHOICES / SHOT 007");
    expect(output).toContain("✅ REPOSITORY");
    expect(output).toContain("4/4 acceptance sets passed");
    expect(output).toContain("\nNEXT\n");
    expect(output).toContain("tohseno five-choices");
    expect(output).toContain("tohseno verify five-choices");
    expect(output).not.toContain("bun ");
    expect(output).not.toContain("\u001b[");
  });

  test("does not present a failed agent or verifier as ready", () => {
    const failedAgent = handoffForShot({
      name: "Unfinished",
      slug: "unfinished",
      path: "/private/shots/unfinished",
      skillCount: 0,
      verificationPassed: true,
      agentExitCode: 17,
      buildState: "not-attempted",
      simulatorState: "not-attempted",
      captureState: "not-attempted",
    });
    expect(failedAgent.evidence.find((item) => item.id === "agent")?.state)
      .toBe("blocked");
    expect(failedAgent.next.command).toBe("tohseno unfinished");

    const failedVerifier = handoffForShot({
      name: "Unsafe",
      slug: "unsafe",
      path: "/private/shots/unsafe",
      skillCount: 1,
      verificationPassed: false,
      agentExitCode: null,
      buildState: "not-attempted",
      simulatorState: "not-attempted",
      captureState: "not-attempted",
    });
    expect(failedVerifier.evidence.find((item) => item.id === "repository")?.state)
      .toBe("blocked");
    expect(failedVerifier.next.command).toBe(
      "tohseno continue '/private/shots/unsafe'",
    );
    expect(failedVerifier.next.why).toContain("pinned privacy and integrity gate");
  });

  test("uses ANSI only for an interactive color-capable terminal", () => {
    const io = createMemoryIo(true);
    renderHandoff(handoffForShot({
      name: "Blank",
      slug: "blank",
      path: "/private/shots/blank",
      skillCount: 0,
      verificationPassed: true,
      agentExitCode: null,
      buildState: "not-attempted",
      simulatorState: "not-attempted",
      captureState: "not-attempted",
    }), io, { TERM: "xterm-256color" });
    expect(io.stdout.join("\n")).toContain("\u001b[32m");
  });
});
