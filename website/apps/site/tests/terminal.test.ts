import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  appNameProblem,
  claimCommand,
  COMMANDS,
  cycleIndex,
  demoHeadline,
  DEMO_STEPS,
  DOORS,
  endsWithBlankLine,
  FALLBACK_LINKS,
  formatBytes,
  intentSummary,
  LINK_COMMANDS,
  resolveCommand,
  sanitizeAppName,
  toSafeReferenceName,
} from "../public/modules/terminal.js";

const INSTALL_COMMAND = "curl -fsSL https://tohseno.com/oneshot.sh | bash";

describe("terminal command surface", () => {
  test("the tohseno prefix is optional and aliases resolve", () => {
    expect(resolveCommand("tohseno create my-app")).toEqual({
      kind: "create",
      argument: "my-app",
    });
    expect(resolveCommand("create my-app")).toEqual({
      kind: "create",
      argument: "my-app",
    });
    expect(resolveCommand("  TOHSENO   CREATE   my-app  ")).toEqual({
      kind: "create",
      argument: "my-app",
    });
    expect(resolveCommand("?")).toEqual({ kind: "help" });
    expect(resolveCommand("cls")).toEqual({ kind: "clear" });
    expect(resolveCommand("github")).toEqual({ kind: "link", link: "source" });
  });

  test("empty, bare, and unknown input are distinct outcomes", () => {
    expect(resolveCommand("   ")).toEqual({ kind: "empty" });
    expect(resolveCommand("tohseno")).toEqual({ kind: "bare" });
    expect(resolveCommand("nonsense")).toEqual({
      kind: "unknown",
      typed: "nonsense",
    });
  });

  // The page asks a person to describe an app. Answering that at the prompt
  // used to produce "command not found: a", which is the page refusing the
  // exact thing it asked for.
  test("a sentence at the prompt is an intention, not a failed command", () => {
    expect(
      resolveCommand("a running app that only counts the days i went out"),
    ).toEqual({
      kind: "intention",
      text: "a running app that only counts the days i went out",
    });
    expect(resolveCommand("chess but the pieces argue")).toEqual({
      kind: "intention",
      text: "chess but the pieces argue",
    });
  });

  test("the intention keeps the person's own spacing and case", () => {
    const written = "A Running App.  It counts days.";
    expect(resolveCommand(`  ${written}  `)).toEqual({
      kind: "intention",
      text: written,
    });
  });

  test("typing the tohseno prefix is always a command attempt", () => {
    // Someone who typed `tohseno` meant to run something; turning their typo
    // into an app intention would be worse than telling them it is a typo.
    expect(resolveCommand("tohseno destroy everything")).toEqual({
      kind: "unknown",
      typed: "destroy",
    });
  });

  test("$TOHSENO reaches the token link however it is typed", () => {
    for (const line of ["$TOHSENO", "$tohseno", "tohseno token", "ca", "chart"]) {
      expect(resolveCommand(line)).toEqual({ kind: "link", link: "token" });
    }
  });

  test("pages kept out of the status bar still have a resolvable href", () => {
    expect(FALLBACK_LINKS.docs).toBe("/docs");
    expect(FALLBACK_LINKS.privacy).toBe("/privacy");
    for (const name of Object.keys(FALLBACK_LINKS)) {
      expect(LINK_COMMANDS).toContain(name);
    }
  });

  test("a multi-word intention after create is kept whole", () => {
    expect(resolveCommand("create the running app")).toEqual({
      kind: "create",
      argument: "the running app",
    });
  });

  test("every advertised command resolves to something real", () => {
    for (const command of COMMANDS) {
      const resolved = resolveCommand(command.usage.replace("<name>", "x"));
      expect(resolved.kind).not.toBe("unknown");
    }
    for (const link of LINK_COMMANDS) {
      expect(resolveCommand(`tohseno ${link}`)).toEqual({ kind: "link", link });
    }
  });

  test("evolve is recognized so it can be redirected rather than not-found", () => {
    expect(resolveCommand("tohseno evolve running-days")).toEqual({
      kind: "evolve",
      argument: "running-days",
    });
  });
});

describe("app names match the engine rule", () => {
  // engine/src/ledger.rs is the authority. A name accepted here must be a name
  // `tohseno create` accepts on the Mac.
  test("sanitizing mirrors sanitize_component", () => {
    expect(sanitizeAppName("My App")).toBe("my-app");
    expect(sanitizeAppName("  --Paper__Press!!  ")).toBe("paper-press");
    expect(sanitizeAppName("running days 2026")).toBe("running-days-2026");
    expect(sanitizeAppName("🌲🌲")).toBe("");
    expect(sanitizeAppName("café")).toBe("caf");
  });

  test("sanitizing is idempotent, so an accepted name survives a round trip", () => {
    for (const value of ["my-app", "a1", "paper-press", "x".repeat(63)]) {
      expect(sanitizeAppName(value)).toBe(value);
      expect(appNameProblem(value)).toBeNull();
    }
  });

  test("empty, reserved, and over-long names are refused with human copy", () => {
    expect(appNameProblem("")).toContain("tohseno create my-app-name");
    expect(appNameProblem("identity")).toContain("reserved");
    expect(appNameProblem("config")).toContain("reserved");
    expect(appNameProblem("x".repeat(64))).toContain("63");
    expect(appNameProblem("My App")).toContain("my-app");
  });
});

describe("doors", () => {
  test("only the Mac door is offered as ready", () => {
    expect(DOORS.map((door) => door.id)).toEqual(["mac", "app", "demo"]);
    expect(DOORS.find((door) => door.id === "mac")?.note).toBe("ready");
    // The iPhone Companion is not published, and the page must not imply it is.
    expect(DOORS.find((door) => door.id === "app")?.note).toBe("soon");
  });

  test("selection wraps in both directions", () => {
    expect(cycleIndex(0, -1, 3)).toBe(2);
    expect(cycleIndex(2, 1, 3)).toBe(0);
    expect(cycleIndex(0, 1, 3)).toBe(1);
    expect(cycleIndex(0, -1, 0)).toBe(0);
  });
});

describe("the demo cannot invent a state", () => {
  const fixture = JSON.parse(
    readFileSync(
      fileURLToPath(new URL("../../../../fixtures/presentation-v1.json", import.meta.url)),
      "utf8",
    ),
  );

  test("every demo step is a state the Mac and the phone also publish", () => {
    for (const step of DEMO_STEPS) {
      expect(fixture.presented_states).toContain(step.state);
    }
  });

  test("the replay walks the whole successful path in order", () => {
    expect(DEMO_STEPS.map((step) => step.state)).toEqual([
      "waiting",
      "building",
      "ready_for_phone",
      "installing",
      "installed",
    ]);
  });

  test("the closing headline is the Mac's own sentence", () => {
    const last = DEMO_STEPS.at(-1)!;
    expect(demoHeadline(last, "running-days")).toBe(
      "running-days is on your iPhone ✓",
    );
  });
});

describe("handoff formatting", () => {
  test("the claim command puts the token after the installer's argument break", () => {
    const token = `ti1.${"a".repeat(32)}.${"b".repeat(43)}.${"c".repeat(43)}`;
    expect(claimCommand(INSTALL_COMMAND, token)).toBe(
      `${INSTALL_COMMAND} -s -- --claim ${token}`,
    );
  });

  test("sizes and counts read like a terminal", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2 KB");
    expect(formatBytes(3 * 1024 * 1024)).toBe("3.0 MB");
    expect(intentSummary("one", 0)).toBe("1 word");
    expect(intentSummary("a b c", 1)).toBe("3 words · 1 image");
    expect(intentSummary("a b c", 2)).toBe("3 words · 2 images");
  });

  test("a blank last line sends, and whitespace alone does not", () => {
    expect(endsWithBlankLine("a running app\n")).toBe(true);
    expect(endsWithBlankLine("a running app")).toBe(false);
    expect(endsWithBlankLine("\n\n")).toBe(false);
  });
});

describe("dropped and pasted image names", () => {
  test("unsafe names become portable names the package format accepts", () => {
    expect(toSafeReferenceName("Screen Shot 2026.PNG", "image/png", 0)).toBe(
      "screen-shot-2026.png",
    );
    expect(toSafeReferenceName("../../etc/passwd", "image/jpeg", 0)).toBe(
      "etc-passwd.jpg",
    );
    expect(toSafeReferenceName("🌲.heic", "image/heic", 3)).toBe("image-4.heic");
    expect(toSafeReferenceName("", "image/webp", 0)).toBe("image-1.webp");
    // A dot that is not a recognized image suffix never truncates the name.
    expect(toSafeReferenceName("my.app.icon", "image/png", 0)).toBe(
      "my-app-icon.png",
    );
  });

  test("collisions are resolved because the package rejects duplicates", () => {
    expect(toSafeReferenceName("shot.png", "image/png", 1, ["shot.png"])).toBe(
      "shot-2.png",
    );
    expect(
      toSafeReferenceName("shot.png", "image/png", 2, ["shot.png", "shot-2.png"]),
    ).toBe("shot-3.png");
  });
});
