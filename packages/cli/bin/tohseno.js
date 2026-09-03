#!/usr/bin/env node
import process from "node:process";
import { spawnSync } from "node:child_process";
import { GUIDE, HELP, parseCommand, redact } from "../src/cli.js";
import { NPM_CLI_VERSION, PRODUCT_VERSION } from "../src/constants.js";
import { delegate, installedNative } from "../src/native.js";
import { installAuthorizedNative } from "../src/installer.js";
import { startProduct } from "../src/start.js";

async function main() {
  const command = parseCommand(process.argv.slice(2));
  if (command.kind === "help") { console.log(HELP); return 0; }
  if (command.kind === "version") { console.log(`tohseno ${NPM_CLI_VERSION}`); return 0; }
  if (command.kind === "guide") { console.log(GUIDE); return 0; }
  if (process.platform !== "darwin") throw new Error("TOHSENO installs on macOS only.");
  let installed = await installedNative(PRODUCT_VERSION);
  if (command.kind === "doctor") {
    const diagnosticNative = installed ?? await installedNative();
    const macOS = spawnSync("/usr/bin/sw_vers", ["-productVersion"], { encoding: "utf8" });
    const tools = spawnSync("/usr/bin/xcode-select", ["-p"], { encoding: "utf8" });
    const xcode = spawnSync("/usr/bin/xcodebuild", ["-version"], { encoding: "utf8" });
    console.log(`macOS: ${macOS.status === 0 ? macOS.stdout.trim() : "unknown"}`);
    console.log(`architecture: ${process.arch}`);
    console.log(`Node: ${process.version}`);
    console.log(`Xcode command-line tools: ${tools.status === 0 ? "ready" : "not ready"}`);
    console.log(`Xcode: ${xcode.status === 0 ? xcode.stdout.split("\n", 1)[0] : "not installed"}`);
    if (!diagnosticNative) {
      console.log("native TOHSENO: not installed");
      console.log("release manifest: checked during install");
      console.log("Local Workspace Service: unavailable until native install");
      console.log("Companion and entitlement: unavailable until native install");
      return 0;
    }
    return delegate(["doctor"]);
  }
  if (!installed) {
    console.log("Installing the verified TOHSENO CLI runtime…");
    await installAuthorizedNative();
    installed = await installedNative(PRODUCT_VERSION);
    if (!installed) throw new Error("the verified native release did not activate safely");
  }
  if (command.kind === "delegate") return delegate(command.args);
  return startProduct();
}

main().then((code) => { process.exitCode = code; }).catch((error) => {
  console.error(`tohseno: ${redact(error.message)}`);
  process.exitCode = 1;
});
