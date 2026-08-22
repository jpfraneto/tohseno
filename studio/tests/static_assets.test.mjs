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

test("Studio keeps four normal views and one replacement gate, not a dashboard", () => {
  const views = [...html.matchAll(/<section id="([a-z-]+)-view"/g)].map((match) => match[1]);
  assert.deepEqual(views, ["gate", "apps", "compose", "state", "settings"]);
  // The grid of simultaneous factory-control regions is gone for good.
  assert.doesNotMatch(style, /grid-template-areas|\.studio-grid/);
  assert.equal(count(html, "<form"), 1, "one intent form serves both create and evolve");
});

test("the normal path never teaches TOHSENO's ontology", () => {
  // Details and Settings are the deliberate pressure-release valves. Every
  // other pixel of the normal create/evolve path is checked here.
  const visible = html
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/<details[\s\S]*?<\/details>/g, "")
    .split('<section id="settings-view"')[0];
  assert.equal(count(visible, ">Take a Shot<"), 1, "the one branded creation action must stay singular");
  const ontologyChecked = visible.replace(">Take a Shot<", "><");
  for (const noun of [
    "Shot",
    "Expression",
    "Version",
    "Execution",
    "Feedback",
    "Marketing",
    "Harness",
    "Lineage",
    "Factory Control",
    "Local Truth",
    "Pending Relay",
  ]) {
    assert.doesNotMatch(
      ontologyChecked,
      new RegExp(`>[^<]*\\b${noun}\\b`, "i"),
      `${noun} must not be visible product vocabulary`,
    );
  }
  assert.match(html, /What do you want this app to be\?/);
  assert.match(script, /"What should change\?"/);
  assert.match(html, />Create App</);
  assert.match(script, /"Evolve App"/);
  assert.match(html, />Take a Shot</);
  assert.match(html, /\+ Add images/);
});

test("apps load explicitly and use the local icon and brand assets", () => {
  assert.match(html, /id="apps-loading"[^>]*role="status"/);
  assert.match(html, /Loading your apps…/);
  assert.match(html, /src="\/tohseno-logo\.png"/);
  assert.match(script, /ui\.appsLoading\.hidden = !loading/);
  assert.match(script, /\/api\/v1\/shots\/\$\{encodeURIComponent\(shot\.shot_id\)\}\/icon/);
  assert.match(style, /\[hidden\][^{]*\{[^}]*display: none !important;/s);
});

test("the workspace is a compact app rail, one intent surface, and one honest phone preview", () => {
  assert.match(style, /grid-template-columns: minmax\(160px, 190px\)/);
  assert.match(style, /\.app-card[^}]*width: 56px[^}]*min-height: 70px/s);
  assert.match(style, /\.app-status-dot/);
  assert.match(style, /\.app-icon-name/);
  assert.match(style, /height: 100dvh/);
  assert.match(style, /overflow-y: auto/);
  assert.match(html, /id="preview-panel"/);
  assert.match(script, /\/api\/v1\/shots\/\$\{encodeURIComponent\(shot\.shot_id\)\}\/preview/);
  assert.match(html, /Latest accepted first screen/);
  assert.doesNotMatch(html, /pipeline|build log|factory control/i);
});

test("creation preserves one exact intent and up to eight images", () => {
  assert.match(html, /id="reference-input"[^>]*accept="[^"]*image\/png[^"]*"[^>]*multiple/);
  assert.match(script, /const MAX_REFERENCES = 8;/);
  assert.match(script, /api\("\/api\/v1\/shots",\s*\{\s*method: "POST"/);
  for (const field of ["command_id", "name", "intention", "pending_intention_id", "references"]) {
    assert.match(script, new RegExp(`${field}:`));
  }
  for (const field of ["filename", "media_type", "origin", "bytes_base64url"]) {
    assert.match(script, new RegExp(`${field}:`));
  }
  assert.match(script, /origin: entry\.origin \|\| `studio-file:\$\{entry\.file\.name\}`/);
  assert.match(script, /bytesToBase64\(bytes, true\)/);
});

test("evolution binds the exact accepted base at submission without a picker", () => {
  assert.match(script, /base_expression_id: shot\.expression_id/);
  assert.match(script, /base_version_id: shot\.latest_version_id/);
  assert.match(script, /base_version_ordinal: shot\.latest_version_ordinal/);
  assert.match(script, /error\.code === "stale_base"/);
  assert.match(script, /This app changed while this request was waiting/);
  // No exact-version picker, no rebind control, no separate feedback ceremony.
  assert.doesNotMatch(script, /Use current Version|feedbackOptions|selectedFeedbackActions/);
  assert.match(script, /selected_feedback_actions: \[\]/);
});

test("human state comes from the service projection, not from a local phase table", () => {
  assert.match(script, /shot\.presentation\.state/);
  assert.match(script, /presentation\.headline/);
  for (const presented of [
    "waiting",
    "building",
    "ready_for_phone",
    "installing",
    "installed",
    "failed",
  ]) {
    assert.ok(script.includes(presented), `missing presented state ${presented}`);
  }
  // Internal phases are no longer interpreted or rendered by the browser.
  assert.doesNotMatch(script, /const PIPELINE|conception:|materializing:|repairing:/);
  assert.doesNotMatch(script, /executionLabel|renderExecution|pipeline-step/);
});

test("waiting for the iPhone offers no extra button", () => {
  assert.match(script, /ui\.stateRetry\.hidden = presentation\.state !== "failed"/);
  assert.match(script, /ui\.stateEvolve\.hidden = presentation\.state !== "installed"/);
  const buttons = [...html.matchAll(/<button id="([a-z-]+)"/g)].map((match) => match[1]);
  for (const forbidden of ["state-install", "state-resume", "state-continue", "state-deliver"]) {
    assert.ok(!buttons.includes(forbidden), `${forbidden} must not exist`);
  }
});

test("Studio uses only the Local Workspace Service API contract", () => {
  for (const endpoint of [
    "/api/v1/health",
    "/api/v1/studio-session",
    "/api/v1/workspace",
    "/api/v1/factory-defaults",
    "/api/v1/shots",
    "/api/v1/events",
    "/api/v1/companion/devices",
    "/api/v1/companion/status",
    "/api/v1/entitlement",
    "/api/v1/genesis",
  ]) {
    assert.ok(script.includes(endpoint), `missing ${endpoint}`);
  }
  assert.match(script, /\/api\/v1\/shots\/\$\{encodeURIComponent\(shot\.shot_id\)\}\/evolutions/);
  assert.match(script, /\/api\/v1\/pending-intentions\/\$\{encodeURIComponent\(pendingId\)\}/);
  // The Studio-only feedback and marketing endpoints were removed with their UI.
  assert.doesNotMatch(script, /\/feedback|\/marketing/);
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

test("routing covers apps, creation, one app, and settings", () => {
  assert.match(script, /route\.pathname === "\/create"/);
  assert.match(script, /route\.pathname === "\/settings"/);
  assert.match(script, /route\.pathname\.startsWith\("\/shots\/"\)/);
  assert.match(script, /route\.searchParams\.get\("name"\)/);
  assert.match(script, /route\.searchParams\.get\("pending"\)/);
  assert.match(script, /ui\.composeIntent\.readOnly = true/);
  assert.match(script, /references\.setLocked\(true\)/);
});

test("Mac-to-iPhone genesis is cable-first and QR is absent", () => {
  assert.doesNotMatch(html, /CONNECT IPHONE/);
  assert.match(html, />Settings</);
  assert.doesNotMatch(html + script, /pairing-qr|qr_svg|Scan the code|Add iPhone/);
  assert.match(script, /\/api\/v1\/genesis\/actions/);
  assert.match(script, /Install TOHSENO/);
  assert.match(script, /method: "DELETE"/);
});

test("workspace changes stream over SSE instead of polling", () => {
  assert.match(script, /new EventSource\("\/api\/v1\/events"\)/);
  assert.match(script, /addEventListener\("workspace\.changed", scheduleRefresh\)/);
  assert.match(script, /addEventListener\("workspace\.reconcile", scheduleRefresh\)/);
  assert.doesNotMatch(script, /setInterval\([^)]*refreshWorkspace/);
});

test("Studio keeps private implementation material off the browser", () => {
  assert.match(html, /Private harness output stays on this Mac/);
  assert.doesNotMatch(html, /chat interface/i);
  assert.doesNotMatch(html, /model selector|choose model|source browser|build logs/i);
  assert.doesNotMatch(script, /harness_output|source_code|recovery_phrase|private_key|seed_phrase/i);
  assert.doesNotMatch(script, /tohseno-node|\/api\/v1\/public/i);
});

test("the collapsed surface stays small", () => {
  // These bounds are a ratchet against the dashboard returning, not a ban on
  // capability. They were re-baselined once, for the Details-only execution
  // receipt; the normal-path vocabulary test above is what actually guards
  // the product surface.
  const scriptLines = script.split("\n").length;
  const styleLines = style.split("\n").length;
  const htmlLines = html.split("\n").length;
  assert.ok(scriptLines < 1_180, `app.js grew back to ${scriptLines} lines`);
  assert.ok(styleLines < 960, `style.css grew back to ${styleLines} lines`);
  assert.ok(htmlLines < 200, `index.html grew back to ${htmlLines} lines`);
});

test("Studio documentation describes the collapsed product surface", () => {
  const prose = readme.replace(/\s+/g, " ");
  for (const phrase of [
    "App → Intent → App on your iPhone",
    "persistent Local Workspace Service",
    "`/create?name=paper`",
    "up to eight validated reference images",
    "anti-CSRF",
    "Settings",
    "presentation",
    "Raw harness output",
  ]) {
    assert.ok(prose.includes(phrase), `README is missing ${phrase}`);
  }
});
