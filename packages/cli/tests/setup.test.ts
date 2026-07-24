import { describe, expect, test } from "bun:test";
import {
  chmodSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { main as setup } from "../../../templates/continuity-app/scripts/setup.ts";
import { withScratchEnvironment } from "./helpers.ts";

function prepareWorkspace(root: string): void {
  mkdirSync(join(root, "Config"), { recursive: true });
  writeFileSync(
    join(root, "continuity.manifest.json"),
    `${JSON.stringify({
      application: {
        id: "com.example.audit",
        name: "Audit Shot",
      },
    })}\n`,
  );
}

async function runSetup(root: string): Promise<void> {
  await setup(["--from-manifest", "--team", "ABCDEFGHIJ"], {
    root,
    environment: { HOME: root },
    output: {
      write() {},
      line() {},
    },
  });
}

describe("machine-local setup writes", () => {
  test("atomically converges owner files on mode 0600", async () => {
    await withScratchEnvironment(async (scratch) => {
      prepareWorkspace(scratch.root);
      const local = join(scratch.root, "Config", "Local.xcconfig");
      writeFileSync(local, "DEV_SECRET = test-only-placeholder\n", {
        mode: 0o644,
      });

      await runSetup(scratch.root);

      const config = join(scratch.root, "app.config.json");
      expect(lstatSync(config).mode & 0o777).toBe(0o600);
      expect(lstatSync(local).mode & 0o777).toBe(0o600);
      expect(readFileSync(local, "utf8")).toContain(
        "DEV_SECRET = test-only-placeholder",
      );
      expect(JSON.parse(readFileSync(config, "utf8"))).toMatchObject({
        displayName: "Audit Shot",
        bundleId: "com.example.audit",
        teamId: "ABCDEFGHIJ",
      });
    });
  });

  test("refuses an app-config symlink before touching either output", async () => {
    await withScratchEnvironment(async (scratch) => {
      prepareWorkspace(scratch.root);
      const sentinel = join(scratch.root, "outside-sentinel");
      writeFileSync(sentinel, "preserve\n");
      symlinkSync(sentinel, join(scratch.root, "app.config.json"));

      await expect(runSetup(scratch.root)).rejects.toThrow(
        "Refusing non-regular app config",
      );

      expect(readFileSync(sentinel, "utf8")).toBe("preserve\n");
      expect(
        lstatSync(join(scratch.root, "Config", "Local.xcconfig"), {
          throwIfNoEntry: false,
        }),
      ).toBeUndefined();
    });
  });

  test("refuses a local-config symlink before rewriting app config", async () => {
    await withScratchEnvironment(async (scratch) => {
      prepareWorkspace(scratch.root);
      const config = join(scratch.root, "app.config.json");
      writeFileSync(
        config,
        `${JSON.stringify({
          displayName: "Previous",
          bundleId: "com.example.previous",
          teamId: "ZZZZZZZZZZ",
        })}\n`,
      );
      const before = readFileSync(config, "utf8");
      const sentinel = join(scratch.root, "outside-local-sentinel");
      writeFileSync(sentinel, "preserve local\n");
      symlinkSync(sentinel, join(scratch.root, "Config", "Local.xcconfig"));

      await expect(runSetup(scratch.root)).rejects.toThrow(
        "Refusing non-regular local Xcode config",
      );

      expect(readFileSync(config, "utf8")).toBe(before);
      expect(readFileSync(sentinel, "utf8")).toBe("preserve local\n");
    });
  });

  test("requires a private regular App Store Connect key before validation", async () => {
    await withScratchEnvironment(async (scratch) => {
      prepareWorkspace(scratch.root);
      const key = join(
        scratch.root,
        ["AuthKey", "ABCDEFGHIJ"].join("_") + ".p8",
      );
      writeFileSync(key, "synthetic key fixture\n", { mode: 0o644 });
      const arguments_ = [
        "--from-manifest",
        "--team", "ABCDEFGHIJ",
        "--asc-key", key,
        "--asc-key-id", "ABCDEFGHIJ",
        "--asc-issuer-id", "11111111-2222-3333-4444-555555555555",
      ];
      await expect(setup(arguments_, {
        root: scratch.root,
        environment: { HOME: scratch.root },
        output: { write() {}, line() {} },
        validateAppStoreConnect: async () => {},
      })).rejects.toThrow("owner-only permissions");

      chmodSync(key, 0o600);
      let validated = false;
      await setup(arguments_, {
        root: scratch.root,
        environment: { HOME: scratch.root },
        output: { write() {}, line() {} },
        validateAppStoreConnect: async () => {
          validated = true;
        },
      });
      expect(validated).toBe(true);
    });
  });

  test("refuses manifest names that could become Xcode configuration", async () => {
    await withScratchEnvironment(async (scratch) => {
      prepareWorkspace(scratch.root);
      writeFileSync(
        join(scratch.root, "continuity.manifest.json"),
        `${JSON.stringify({
          application: {
            id: "com.example.audit",
            name: "$(DEV_SECRET)",
          },
        })}\n`,
      );

      await expect(runSetup(scratch.root)).rejects.toThrow(
        "Xcode configuration syntax",
      );
      expect(
        lstatSync(join(scratch.root, "Config", "Local.xcconfig"), {
          throwIfNoEntry: false,
        }),
      ).toBeUndefined();
    });
  });
});
