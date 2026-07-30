import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const tests = dirname(fileURLToPath(import.meta.url));
const studio = resolve(tests, "..");
const scriptPath = resolve(studio, "app.js");
const html = readFileSync(resolve(studio, "index.html"), "utf8");
const script = readFileSync(scriptPath, "utf8");

test("browser script is valid JavaScript", () => {
  const checked = spawnSync(process.execPath, ["--check", scriptPath], {
    encoding: "utf8",
  });
  assert.equal(checked.status, 0, checked.stderr);
});

test("every queried Studio element exists exactly once", () => {
  const queried = [...script.matchAll(/querySelector\("#([^"]+)"\)/g)]
    .map((match) => match[1]);
  const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]);
  assert.equal(new Set(ids).size, ids.length, "HTML contains a duplicate id");
  for (const id of queried) {
    assert.equal(
      ids.filter((candidate) => candidate === id).length,
      1,
      `#${id} must exist exactly once`,
    );
  }
});

test("ontology rendering is text-only and preserves absent legacy facts", () => {
  assert.equal(script.includes(".innerHTML"), false);
  assert.match(script, /exactIntentionText/);
  assert.match(script, /does not contain the original coherent intention/);
  assert.match(script, /does not contain an accepted Shot genome/);
  assert.match(html, /Original intention/);
  assert.match(html, /Accepted genome/);
  assert.match(html, /Token association \(v2\)/);
  assert.match(script, /relationship only; never Shot identity or ownership/);
});

test("feedback is private and bound to one exact version ordinal", () => {
  assert.match(script, /fetch\("\/api\/feedback"/);
  assert.match(script, /version_ordinal: binding\.ordinal/);
  assert.match(script, /ordinal !== selectedShot/);
  assert.match(script, /version\.expression_id !== expression\.expression_id/);
  assert.match(html, /Save private feedback/);
  assert.match(script, /will not accept floating Shot-level feedback/);
  assert.match(script, /saved\.action_commitment/);
  assert.match(script, /selected_feedback_actions: selected/);
  assert.match(html, /Select this signed observation for the next evolution/);
  assert.match(script, /"x-tohseno-studio": "1"/);
});

test("creation reviews the exact proposed Genome before an explicit acceptance", () => {
  assert.match(script, /fetch\("\/api\/plan"/);
  assert.match(script, /plan\.genome_markdown/);
  assert.match(script, /accept_genome: composerMode === "create"/);
  assert.match(script, /reviewedInitialPlan\.prompt !== prompt/);
  assert.match(html, /PROPOSED · NOT COMMITTED/);
  assert.match(html, /Accepting it establishes\s+the first Genome/);
});

test("Shot preparation selects a native harness and follows durable execution events", () => {
  assert.match(html, /Coding harness/);
  assert.match(html, /Inference \/ payment route/);
  assert.match(html, /PREPARE SHOT/);
  assert.match(script, /fetch\("\/api\/harnesses"/);
  assert.match(script, /harness: ui\.harness\.value/);
  assert.match(script, /model: ui\.model\.value/);
  assert.match(script, /route: ui\.route\.value/);
  assert.match(script, /\/api\/executions\//);
  assert.match(script, /SHOT IN FLIGHT/);
  assert.match(script, /SHOT LANDED/);
  assert.doesNotMatch(script, /WebSocket|stream-json|output-format/);
});

test("first-run onboarding uses authoritative Mac and harness readiness", () => {
  assert.match(html, /FIRST SHOT/);
  assert.match(html, /Give the factory its Apple tools/);
  assert.match(html, /https:\/\/chatgpt\.com\/codex\/install\.sh/);
  assert.match(html, /https:\/\/claude\.ai\/install\.sh/);
  assert.match(script, /BEGIN FIRST SHOT/);
  assert.match(script, /fetch\("\/api\/onboarding"/);
  assert.match(script, /onboardingFacts\.xcode\.ready/);
  assert.match(script, /onboardingFacts\.apple_signing\.ready/);
  assert.match(script, /onboardingFacts\?\.harness_ready/);
  assert.match(script, /route\.billing === "subscription"/);
  assert.match(script, /route\.estimated_additional_cost_usd === 0/);
  assert.doesNotMatch(script, /localStorage\.setItem\([^)]*(?:xcode|signing|harness_ready)/);
});

test("node participation is optional and cannot block local Studio startup", () => {
  assert.match(script, /response\.status === 404/);
  assert.match(script, /configured: false/);
  assert.match(script, /Studio remains fully local/);
  assert.match(html, /Studio does not require a node/);
});

test("contract definition is visibly inactive and Studio has no retired public surface", () => {
  assert.match(html, /Contract definition/);
  assert.match(html, /id="generation-status"[^>]*>Inactive</);
  assert.match(script, /active_generation: activeGeneration/);
  assert.match(script, /No public witness generation is active/);
  assert.match(script, /no deployment or broadcast path/);
  assert.doesNotMatch(html, /ShotRelations|Pairing target|Experimental publish/);
  assert.doesNotMatch(html, /id="(?:handle|appcoin|registry|published)-status"/);
  assert.doesNotMatch(script, /pairing|claimHandle|attestAppStore|ShotRelations/);
});

test("Bankr launch belongs to one selected Shot and is separately confirmed", () => {
  const selection = html.match(/<section id="selection"[\s\S]*?<\/section>/)?.[0] || "";
  const globalActions = html.slice(0, html.indexOf('<div class="library-scroll">'));
  assert.match(selection, /id="launch-token"/);
  assert.match(selection, /Launch \$TOHSENO for this Shot/);
  assert.doesNotMatch(globalActions, /id="launch-token"/);
  assert.match(html, /id="bankr-shot-id"/);
  assert.match(html, /jpfraneto\.eth/);
  assert.match(html, /Bankr’s wallet—not\s+          jpfraneto\.eth—will be the on-chain deployer/);
  assert.match(script, /fetch\("\/api\/bankr\/launch\/simulate"/);
  assert.match(script, /fetch\("\/api\/bankr\/launch\/deploy"/);
  assert.match(script, /app_name: ui\.bankrDialog\.dataset\.appName/);
  assert.match(script, /version_ordinal: Number\(ui\.bankrDialog\.dataset\.versionOrdinal\)/);
  assert.match(script, /approval_id: approval\.approval_id/);
  assert.match(script, /shot: approval\.shot/);
  assert.match(script, /ui\.bankrConfirmation\.value === bankrApproval\.confirmation_phrase/);
  assert.match(script, /token_association\?\.status === "associated"/);
  assert.match(script, /Do not click deploy again/);
  assert.match(html, /private Token Association is not\s+          a Shot publication/);
  assert.match(script, /private signed association recorded for Shot/);
  assert.match(script, /no Shot registry transaction was sent/);
  assert.doesNotMatch(html, /id="bankr-api-key"|name="bankr_api_key"/);
  assert.doesNotMatch(script, /localStorage\.(?:setItem|getItem)\([^)]*BANKR_API_KEY/);
});
