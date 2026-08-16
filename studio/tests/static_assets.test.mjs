import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const studio = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scriptPath = resolve(studio, "app.js");
const html = readFileSync(resolve(studio, "index.html"), "utf8");
const script = readFileSync(scriptPath, "utf8");

test("browser script is valid JavaScript", () => {
  const checked = spawnSync(process.execPath, ["--check", scriptPath], { encoding: "utf8" });
  assert.equal(checked.status, 0, checked.stderr);
});

test("Studio contains only the recording loop", () => {
  for (const phrase of ["Apps", "Versions", "Initialize app", "Open folder", "Record version"])
    assert.match(html, new RegExp(phrase));
  assert.match(script, /fetch\(path/);
  assert.match(script, /\/api\/versions/);
  assert.match(script, /\/api\/apps/);
});

test("factory and delivery concepts are absent", () => {
  for (const removed of [
    "intention", "reference images", "harness", "model", "route", "preview", "simulator",
    "install", "evolve app", "Bankr", "token", "protocol", "\/shots", "\/api\/executions",
  ]) assert.doesNotMatch(html + script, new RegExp(removed, "i"));
});

test("mutations carry the loopback Studio header", () => {
  assert.match(script, /"x-tohseno-studio": "1"/);
});
