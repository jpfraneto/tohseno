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
const style = readFileSync(resolve(studio, "style.css"), "utf8");
const readme = readFileSync(resolve(studio, "README.md"), "utf8");

function count(source, needle) {
  return source.split(needle).length - 1;
}

test("browser script is valid JavaScript", () => {
  const checked = spawnSync(process.execPath, ["--check", scriptPath], { encoding: "utf8" });
  assert.equal(checked.status, 0, checked.stderr);
});

test("every JavaScript-bound element exists exactly once", () => {
  const selectors = [...script.matchAll(/document\.querySelector\("#([a-z0-9-]+)"\)/g)]
    .map((match) => match[1]);
  assert.ok(selectors.length > 60, "expected the full Studio surface to be bound");
  assert.equal(new Set(selectors).size, selectors.length, "JavaScript binds an ID more than once");
  for (const id of selectors) {
    assert.equal(count(html, `id="${id}"`), 1, `#${id} must exist exactly once`);
  }

  const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]);
  assert.equal(new Set(ids).size, ids.length, "HTML IDs must be unique");
});

test("assets are compatible with a strict no-inline CSP", () => {
  assert.match(html, /<link rel="stylesheet" href="\/style\.css">/);
  assert.match(html, /<script src="\/app\.js" defer><\/script>/);
  assert.doesNotMatch(html, /<style(?:\s|>)/i);
  assert.doesNotMatch(html, /<script(?![^>]*\bsrc=)[^>]*>[\s\S]*?<\/script>/i);
  assert.doesNotMatch(html, /\sstyle\s*=/i);
  assert.doesNotMatch(html, /\son[a-z]+\s*=/i);
  assert.doesNotMatch(script, /\.innerHTML\b|\.outerHTML\b|insertAdjacentHTML|document\.write|\beval\s*\(|new Function\b/);
  assert.match(script, /textContent/);
});

test("Studio presents the three-region local app factory", () => {
  for (const phrase of [
    "YOUR SHOTS",
    "INTENT / SHOT ACTIVITY",
    "CURRENT APP / EXECUTION",
    "CONNECT IPHONE",
    "LOCAL APP FACTORY",
  ]) {
    assert.match(html, new RegExp(phrase.replaceAll("/", "\\/")));
  }
  assert.match(style, /\.studio-grid\s*\{[\s\S]*grid-template-columns:/);
  assert.match(html, /id="connect-iphone"[^>]*>[\s\S]*CONNECT IPHONE/);
});

test("creation route preserves one exact intention and up to eight image references", () => {
  for (const phrase of ["Exact intention", "Reference images", "0 / 8", "TAKE THE SHOT"])
    assert.match(html, new RegExp(phrase));
  assert.match(html, /id="create-images"[^>]*accept="[^"]*image\/png[^"]*"[^>]*multiple/);
  assert.match(script, /const MAX_REFERENCES = 8;/);
  assert.match(script, /const exactIntention = ui\.createIntention\.value;/);
  assert.match(script, /api\("\/api\/v1\/shots",\s*\{\s*method: "POST"/);
  for (const field of ["command_id", "name", "intention", "pending_intention_id", "references"])
    assert.match(script, new RegExp(`${field}:`));
  for (const field of ["filename", "media_type", "origin", "bytes_base64url"])
    assert.match(script, new RegExp(`${field}:`));
  assert.match(script, /origin: entry\.origin \|\| `studio-file:\$\{entry\.file\.name\}`/);
  assert.match(script, /bytesToBase64\(bytes, true\)/);
  assert.match(script, /route\.pathname === "\/create"/);
  assert.match(script, /route\.searchParams\.get\("name"\)/);
  assert.match(script, /route\.searchParams\.get\("pending"\)/);
  assert.match(script, /\/api\/v1\/pending-intentions\/\$\{encodeURIComponent\(pendingId\)\}/);
  assert.match(script, /ui\.createIntention\.readOnly = true/);
  assert.match(script, /createReferences\.setLocked\(true\)/);
  assert.match(script, /route\.pathname\.startsWith\("\/shots\/"\)/);
  assert.match(script, /history\.replaceState\(null, "", `\/shots\/\$\{encodeURIComponent\(shotId\)\}`\)/);
});

test("Studio uses only the Local Workspace Service API contract", () => {
  for (const endpoint of [
    "/api/v1/health",
    "/api/v1/studio-session",
    "/api/v1/workspace",
    "/api/v1/shots",
    "/api/v1/events",
    "/api/v1/companion/devices",
    "/api/v1/companion/status",
    "/api/v1/companion/pairing-sessions",
  ]) {
    assert.ok(script.includes(endpoint), `missing ${endpoint}`);
  }
  assert.match(script, /\/api\/v1\/executions\/\$\{encodeURIComponent\(executionId\)\}/);
  assert.match(script, /\/api\/v1\/shots\/\$\{encodeURIComponent\(binding\.shotId\)\}\/feedback/);
  assert.match(script, /\/api\/v1\/shots\/\$\{encodeURIComponent\(binding\.shotId\)\}\/evolutions/);
  assert.match(script, /\/api\/v1\/shots\/\$\{encodeURIComponent\(shot\.shot_id\)\}\/marketing/);
  assert.doesNotMatch(script, /\/api\/(?:apps|versions)\b/);
});

test("mutations use the same-origin Studio session and anti-CSRF token", () => {
  assert.match(script, /session\.schema !== "tohseno\.local-studio-session\/1"/);
  assert.match(script, /session\.origin !== window\.location\.origin/);
  assert.match(script, /health\.origin !== window\.location\.origin/);
  assert.match(script, /health\.instance_id !== state\.sessionInstanceId/);
  assert.match(script, /headers\.set\("X-Tohseno-CSRF", state\.csrfToken\)/);
  assert.match(script, /credentials: "same-origin"/);
  assert.match(script, /cache: "no-store"/);
  assert.match(script, /headers\.set\("Content-Type", "application\/json"\)/);
  assert.doesNotMatch(html + script, /Access-Control-Allow-Origin|x-tohseno-studio/i);
});

test("feedback and evolution remain bound to an exact accepted Version", () => {
  for (const field of ["expression_id", "version_id", "version_ordinal"])
    assert.match(script, new RegExp(`${field}: binding\\.`));
  for (const field of ["base_expression_id", "base_version_id", "base_version_ordinal"])
    assert.match(script, new RegExp(`${field}: binding\\.`));
  assert.match(script, /const feedbackActions = selectedFeedbackActions\(\);/);
  assert.match(script, /selected_feedback_actions: feedbackActions/);
  assert.match(script, /!exactIntention\.trim\(\) && feedbackActions\.length === 0/);
  assert.match(html, /Required unless one or more exact Feedback actions are selected\./);
  assert.match(html, /A newer base is never selected silently\./);
  assert.match(html, /id="feedback-rebind"[^>]*>Use current Version<\/button>/);
  assert.match(html, /id="evolve-rebind"[^>]*>Use current Version<\/button>/);
  assert.match(html, />EVOLVE FROM THIS<\/button>/);
  assert.match(script, /classList\.contains\("stale"\)/);
});

test("marketing notes are private and Shot-bound", () => {
  assert.match(html, /PRIVATE NOTE/);
  assert.match(html, /SHOT-BOUND/);
  assert.match(html, /It is not posted, scheduled, or sent to a model\./);
  assert.match(script, /command_id: stableCommandId\(ui\.marketingForm, "marketing"\)/);
  assert.match(script, /body: ui\.marketingBody\.value/);
});

test("pairing uses the service-rendered standard QR and an event-driven expiring lifecycle", () => {
  assert.match(html, /ONE-USE PAIRING SEAL/);
  assert.match(html, /scan this standard QR/);
  assert.match(html, /class="pairing-seal"/);
  assert.match(style, /--orange:/);
  assert.match(style, /url\("\/pairing-seal\.png"\)/);
  assert.match(style, /\.qr-frame\s*\{[\s\S]*background: (?:#fff|white)/);
  assert.match(script, /session\.schema !== "tohseno\.studio-pairing-session\/1"/);
  assert.match(script, /typeof session\.qr_svg !== "string"/);
  assert.match(script, /new TextEncoder\(\)\.encode\(pairing\.qr_svg\)/);
  assert.match(script, /data:image\/svg\+xml;base64/);
  assert.match(script, /Date\.parse\(state\.pairing\.expires_at\) - Date\.now\(\)/);
  assert.match(script, /source\.addEventListener\("workspace\.changed", scheduleRefresh\)/);
  assert.match(script, /if \(state\.pairing\?\.state === "waiting"\) await refreshPairingSession\(\)/);
  assert.doesNotMatch(script, /pairingRefreshTimer/);
  assert.match(script, /route\.searchParams\.get\("pair"\)/);
  assert.match(script, /refreshPairingSession\(\)/);
  assert.doesNotMatch(script, /pairing_uri/);
});

test("workspace changes stream over SSE instead of polling the workspace", () => {
  assert.match(script, /new EventSource\("\/api\/v1\/events"\)/);
  assert.match(script, /addEventListener\("workspace\.changed", scheduleRefresh\)/);
  assert.match(script, /addEventListener\("workspace\.reconcile", scheduleRefresh\)/);
  assert.doesNotMatch(script, /setInterval\([^)]*refreshWorkspace/);
  assert.doesNotMatch(script, /setTimeout\([^)]*refreshWorkspace[^)]*,\s*1000\b/);
});

test("paired devices expose capabilities, real timestamps, sync state, and revocation", () => {
  assert.match(html, /PAIRED DEVICES/);
  assert.match(script, /device\.device_id_abbreviation/);
  assert.match(script, /formatTimestamp\(device\.paired_at\)/);
  assert.match(script, /relativeTime\(device\.last_seen\)/);
  assert.match(script, /device\.sync_state/);
  for (const capability of [
    "workspace.read",
    "execution.read",
    "shot.create",
    "shot.evolve",
    "feedback.write",
    "marketing.write",
  ]) assert.ok(script.includes(capability));
  assert.match(script, /method: "DELETE"/);
  assert.match(script, /This iPhone will immediately lose workspace capabilities/);
});

test("recording-only folders are identified and never silently promoted", () => {
  assert.match(html, /Recording-only folder/);
  assert.match(html, /will not silently turn it into a factory Shot/);
  assert.match(script, /shot\.kind === "recording_only"/);
  assert.match(script, /shot\.kind !== "factory_shot"/);
});

test("Shot views expose archived and retired workspace state", () => {
  assert.match(html, /<dt>Shot status<\/dt><dd id="current-shot-status">/);
  assert.match(script, /shot\.retired \? "Retired" : shot\.archived \? "Archived" : "Active"/);
  assert.match(script, /Retired · local history preserved/);
  assert.match(script, /Archived · local history preserved/);
});

test("Studio keeps private implementation material off the browser and phone surfaces", () => {
  assert.match(html, /Source and harness output stay on this Mac\./);
  assert.doesNotMatch(html, /chat interface/i);
  assert.doesNotMatch(html, /model selector|choose model|source browser|build logs/i);
  assert.doesNotMatch(script, /harness_output|source_code|recovery_phrase|private_key|seed_phrase/i);
  assert.doesNotMatch(script, /tohseno-node|\/api\/v1\/public/i);
});

test("Studio documentation describes the persistent factory and browser boundary", () => {
  const prose = readme.replace(/\s+/g, " ");
  for (const phrase of [
    "persistent Local Workspace Service",
    "`/create?name=tohseno`",
    "up to eight validated reference images",
    "anti-CSRF",
    "CONNECT IPHONE",
    "recording_only",
    "raw harness output",
  ]) assert.ok(prose.includes(phrase), `README is missing ${phrase}`);
});
