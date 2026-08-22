#!/usr/bin/env node
import process from "node:process";
import { spawnSync } from "node:child_process";
import { HELP, parseCommand, redact } from "../src/cli.js";
import { NPM_CLI_VERSION } from "../src/constants.js";
import { delegate, installedNative } from "../src/native.js";
import { installAuthorizedNative } from "../src/installer.js";

async function main() {
  const command = parseCommand(process.argv.slice(2));
  if (command.kind === "help") { console.log(HELP); return 0; }
  if (command.kind === "version") { console.log(`tohseno npm ${NPM_CLI_VERSION}`); return 0; }
  if (process.platform !== "darwin") throw new Error("TOHSENO installs on macOS only.");
  let installed = await installedNative();
  if (command.kind === "doctor") {
    const macOS = spawnSync("/usr/bin/sw_vers", ["-productVersion"], { encoding: "utf8" });
    const tools = spawnSync("/usr/bin/xcode-select", ["-p"], { encoding: "utf8" });
    const xcode = spawnSync("/usr/bin/xcodebuild", ["-version"], { encoding: "utf8" });
    console.log(`macOS: ${macOS.status === 0 ? macOS.stdout.trim() : "unknown"}`);
    console.log(`architecture: ${process.arch}`);
    console.log(`Node: ${process.version}`);
    console.log(`Xcode command-line tools: ${tools.status === 0 ? "ready" : "not ready"}`);
    console.log(`Xcode: ${xcode.status === 0 ? xcode.stdout.split("\n", 1)[0] : "not installed"}`);
    if (!installed) {
      console.log("native TOHSENO: not installed");
      console.log("release manifest: checked during install");
      console.log("Local Workspace Service: unavailable until native install");
      console.log("Companion and entitlement: unavailable until native install");
      return 0;
    }
    return delegate(["doctor"]);
  }
  if (command.kind === "delegate") {
    if (!installed) throw new Error("Install TOHSENO first with `tohseno install`.");
    return delegate(command.args);
  }
  if (!installed) {
    console.log("Installing TOHSENO…");
    await installAuthorizedNative();
    installed = await installedNative();
    if (!installed) throw new Error("the verified native release did not activate safely");
  }
  if (command.kind === "install") return 0;
  console.log("Starting TOHSENO…");
  delegate(["service", "install"]);
  delegate(["service", "start"]);
  console.log("Opening TOHSENO…");
  return delegate(["studio"]);
}

main().then((code) => { process.exitCode = code; }).catch((error) => {
  console.error(`tohseno: ${redact(error.message)}`);
  process.exitCode = 1;
});
