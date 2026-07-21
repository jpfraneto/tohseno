/**
 * One-time credential flow: `bun run setup`
 *
 * Turns "fill in the blanks" into a script. Asks one question at a time,
 * offers derived defaults, and writes:
 *
 *   app.config.json        identifiers and key *paths* (gitignored)
 *   Config/Local.xcconfig  the overrides the Xcode project reads (gitignored)
 *
 * Secret VALUES never touch disk here — only public identifiers and the
 * filesystem path to your App Store Connect .p8 key. Nothing is hard-coded
 * to any specific person's accounts: run this with your own Apple
 * credentials and the machine is yours.
 */

import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");

interface SetupConfig {
  displayName: string;
  bundleId: string;
  teamId: string;
  appStoreConnect?: {
    keyPath: string;
    keyId: string;
    issuerId: string;
  };
  revenueCatPublicKey?: string;
}

// Bun's console is an async iterable of stdin lines; unlike readline it
// behaves identically for interactive terminals and piped input.
const stdinLines = console[Symbol.asyncIterator]();

async function ask(question: string, fallback: string): Promise<string> {
  const suffix = fallback ? ` [${fallback}]` : " (press enter to skip)";
  process.stdout.write(`${question}${suffix}: `);
  const line = await stdinLines.next();
  const answer = line.done ? "" : String(line.value).trim();
  if (line.done) process.stdout.write("\n");
  return answer || fallback;
}

async function askValidated(
  question: string,
  fallback: string,
  validate: (value: string) => string | null,
): Promise<string> {
  while (true) {
    const answer = await ask(question, fallback);
    const problem = validate(answer);
    if (problem === null) return answer;
    console.log(`  ${problem}`);
  }
}

function deriveBundleId(displayName: string): string {
  const slug = displayName
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "")
    .slice(0, 40);
  return `com.${slug || "myapp"}.app`;
}

async function main(): Promise<void> {
  console.log("\nTOHSENO app setup — one question at a time.");
  console.log("Everything written here stays on this machine and out of git.\n");

  const displayName = await askValidated(
    "App display name",
    "Writing",
    (value) => (value.length > 0 && value.length <= 30 ? null : "Use 1–30 characters."),
  );

  const bundleId = await askValidated(
    "Bundle ID",
    deriveBundleId(displayName),
    (value) =>
      /^[A-Za-z0-9]+(\.[A-Za-z0-9-]+)+$/.test(value)
        ? null
        : "A bundle ID looks like com.yourname.yourapp.",
  );

  const teamId = await askValidated(
    "Apple Team ID (Membership page of developer.apple.com)",
    "",
    (value) =>
      value === "" || /^[A-Z0-9]{10}$/.test(value)
        ? null
        : "A Team ID is 10 uppercase letters/digits. Press enter to skip for now.",
  );

  const config: SetupConfig = { displayName, bundleId, teamId };

  const keyPath = await askValidated(
    "App Store Connect API key path (.p8, for TestFlight uploads)",
    "",
    (value) => {
      if (value === "") return null;
      if (!value.endsWith(".p8")) return "That should be a .p8 file path. Press enter to skip.";
      return existsSync(resolve(value.replace(/^~(?=\/)/, process.env.HOME ?? "~")))
        ? null
        : "No file at that path. Check it, or press enter to skip.";
    },
  );

  if (keyPath !== "") {
    const keyId = await askValidated(
      "  API key ID",
      "",
      (value) => (/^[A-Z0-9]{10}$/.test(value) ? null : "A key ID is 10 uppercase letters/digits."),
    );
    const issuerId = await askValidated(
      "  Issuer ID",
      "",
      (value) =>
        /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value)
          ? null
          : "An issuer ID is a UUID (from the App Store Connect Keys page).",
    );
    config.appStoreConnect = {
      keyPath: resolve(keyPath.replace(/^~(?=\/)/, process.env.HOME ?? "~")),
      keyId,
      issuerId,
    };
  }

  const revenueCatPublicKey = await askValidated(
    "RevenueCat public API key (only if you'll enable the paywall)",
    "",
    (value) =>
      value === "" || /^[A-Za-z0-9_]{10,}$/.test(value)
        ? null
        : "That doesn't look like a RevenueCat public key. Press enter to skip.",
  );
  if (revenueCatPublicKey !== "") config.revenueCatPublicKey = revenueCatPublicKey;

  const configPath = join(ROOT, "app.config.json");
  await Bun.write(configPath, JSON.stringify(config, null, 2) + "\n");

  const xcconfigPath = join(ROOT, "Config", "Local.xcconfig");
  await Bun.write(
    xcconfigPath,
    [
      "// Written by `bun run setup`. Gitignored: identifiers are yours, not the repo's.",
      `APP_DISPLAY_NAME = ${displayName}`,
      `APP_BUNDLE_ID = ${bundleId}`,
      `DEVELOPMENT_TEAM = ${teamId}`,
      `REVENUECAT_PUBLIC_KEY = ${revenueCatPublicKey}`,
      "",
    ].join("\n"),
  );

  console.log(`\n  ✓ ${configPath}`);
  console.log(`  ✓ ${xcconfigPath}`);
  console.log("\nNext:");
  console.log("  open Writing.xcodeproj        # run on the simulator, zero keys needed");
  if (config.appStoreConnect) {
    console.log("  fastlane beta                 # TestFlight upload — run it yourself when ready");
  } else {
    console.log("  (re-run setup with an App Store Connect key when you want TestFlight)");
  }
  console.log("");
}

await main();
