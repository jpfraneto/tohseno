import "./modules/obsolete-worker-cleanup.js";
import { LIMITS } from "./modules/intent-limits.js";
import {
  buildIntentPackage,
  mediaTypeForFilename,
  utf8ByteLength,
} from "./modules/intent-package.js";
import { claimToken, createEncryptedEnvelope } from "./modules/intent-crypto.js";
import {
  capabilities as relayCapabilities,
  relayStatus,
  uploadEnvelope,
} from "./modules/relay-client.js";
import { openDraftStore } from "./modules/draft-store.js";
import { transferStateLabel } from "./modules/draft-logic.js";
import {
  appNameFromFilename,
  appNameProblem,
  claimCommand,
  classifyDroppedFile,
  COMMANDS,
  cycleIndex,
  DEFAULT_APP_NAME,
  demoHeadline,
  DEMO_STEPS,
  DOORS,
  endsWithBlankLine,
  FALLBACK_LINKS,
  formatBytes,
  intentSummary,
  joinPromptText,
  resolveCommand,
  sanitizeAppName,
  toSafeReferenceName,
} from "./modules/terminal.js";

const SUPPORTED_IMAGE_TYPES = new Set([
  "image/png",
  "image/jpeg",
  "image/heic",
  "image/webp",
]);

// The headlines come from the Mac verbatim, and its Installed sentence already
// ends in a check, so that row's marker stays out of the way.
const STATE_GLYPHS = Object.freeze({
  waiting: "·",
  building: "»",
  ready_for_phone: "✓",
  installing: "»",
  installed: " ",
  failed: "✗",
});

const MODES = Object.freeze({
  command: {
    sigil: "$",
    placeholder: "tohseno create [optional-name]",
    hint: "Describe your app, attach images, and experience it on your phone. Type help for commands",
  },
  compose: {
    sigil: ">",
    placeholder: "what should it do?",
    // Spelled out rather than drawn: the return glyph is missing from enough
    // monospace fonts that it lands as a blank box exactly where it matters.
    hint: "return twice to send · drop images or a .md file · esc to cancel",
  },
  choose: {
    sigil: " ",
    placeholder: "",
    hint: "↑ ↓ to choose · return to confirm · esc to go back",
  },
  busy: { sigil: " ", placeholder: "", hint: "working…" },
});

const terminal = document.querySelector("#terminal");
const screen = document.querySelector("#term-screen");
const stream = document.querySelector("#term-stream");
const form = document.querySelector("#term-form");
const input = document.querySelector("#terminal-input");
const sigil = document.querySelector("#term-sigil");
const hint = document.querySelector("#term-hint");
const veil = document.querySelector("#drop-veil");
const installCommand = terminal.dataset.installCommand;
const linkHrefs = new Map(
  [...document.querySelectorAll("[data-link]")].map((anchor) => [
    anchor.dataset.link,
    anchor.href,
  ]),
);
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

const state = {
  mode: "command",
  appName: null,
  prompt: "",
  references: [],
  choice: null,
  history: [],
  historyIndex: 0,
  poll: null,
  demoToken: 0,
  dragDepth: 0,
};

let draftStore = null;

/* ---------------------------------------------------------------- output */

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function emit(node) {
  stream.append(node);
  screen.scrollTop = screen.scrollHeight;
  return node;
}

function say(text = "", variant = "") {
  return emit(el("p", variant ? `line ${variant}` : "line", text));
}

function gap() {
  return emit(el("p", "line spacer"));
}

function echoCommand(text) {
  return emit(el("p", "line echo", text));
}

function stateLine(presentedState, text) {
  const row = el("p", "line state-line");
  row.dataset.state = presentedState;
  row.append(el("span", "glyph", STATE_GLYPHS[presentedState] ?? "·"));
  row.append(el("span", "label", text));
  return emit(row);
}

/// One command, rendered as its own copy target. The claim command carries a
/// single-use bearer capability, so retyping it by hand is never the path.
function pasteBlock(command, label = "CLICK TO COPY") {
  const button = el("button", "paste-block");
  button.type = "button";
  const body = el("span", "cmd", command);
  const note = el("span", "copy-hint", label);
  button.append(body, note);
  button.addEventListener("click", () => {
    navigator.clipboard.writeText(command).then(
      () => {
        note.textContent = "COPIED";
      },
      () => {
        const selection = window.getSelection();
        const range = document.createRange();
        range.selectNodeContents(body);
        selection.removeAllRanges();
        selection.addRange(range);
        note.textContent = "SELECT AND COPY";
      },
    );
  });
  return emit(button);
}

let choiceGroups = 0;

/// The list owns keyboard focus while it is open. Leaving focus in the hidden
/// prompt would route arrow keys to a control the person cannot see.
function openChoices(options, onPick) {
  const group = (choiceGroups += 1);
  const list = el("ul", "choices");
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", "Where should this go?");
  list.tabIndex = 0;
  const items = options.map((option, index) => {
    const item = el("li", "choice");
    item.id = `choice-${group}-${index}`;
    item.setAttribute("role", "option");
    item.append(el("span", "choice-label", option.label));
    if (option.note) item.append(el("span", "choice-note", option.note));
    item.addEventListener("click", () => {
      if (state.choice?.list !== list) return;
      state.choice.index = index;
      paintChoices();
      commitChoice();
    });
    list.append(item);
    return item;
  });
  list.addEventListener("keydown", handleChoiceKey);
  state.choice = { options, items, list, index: 0, onPick };
  paintChoices();
  emit(list);
  setMode("choose");
  list.focus();
}

function paintChoices() {
  const { items, index, list } = state.choice;
  items.forEach((item, position) =>
    item.setAttribute("aria-selected", String(position === index)),
  );
  list.setAttribute("aria-activedescendant", items[index].id);
}

function handleChoiceKey(event) {
  if (state.mode !== "choose" || !state.choice) return;
  const { options } = state.choice;
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    state.choice.index = cycleIndex(
      state.choice.index,
      event.key === "ArrowDown" ? 1 : -1,
      options.length,
    );
    paintChoices();
    return;
  }
  if (/^[1-9]$/.test(event.key) && Number(event.key) <= options.length) {
    event.preventDefault();
    state.choice.index = Number(event.key) - 1;
    paintChoices();
    commitChoice();
    return;
  }
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    commitChoice();
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    cancelChoices();
  }
}

function commitChoice() {
  const { options, index, onPick, list } = state.choice;
  state.choice = null;
  retireChoices(list);
  setMode("busy");
  onPick(options[index]);
}

function cancelChoices() {
  if (!state.choice) return;
  retireChoices(state.choice.list);
  state.choice = null;
  setMode("command");
}

/// A committed list stays in the transcript as a record of what was chosen,
/// but it is no longer a control.
function retireChoices(list) {
  list.dataset.state = "done";
  list.removeAttribute("tabindex");
  list.removeEventListener("keydown", handleChoiceKey);
}

/* ----------------------------------------------------------------- modes */

function setMode(mode) {
  state.mode = mode;
  const shape = MODES[mode];
  sigil.textContent = shape.sigil;
  input.placeholder = shape.placeholder;
  hint.textContent = shape.hint;
  const typing = mode === "command" || mode === "compose";
  form.hidden = !typing;
  if (typing) {
    autosize();
    input.focus();
  } else {
    // Chromium keeps focus on a display:none textarea, which would swallow
    // every key meant for the surface that replaced it.
    input.blur();
  }
}

function autosize() {
  input.rows = 1;
  const rows = Math.min(12, input.value.split("\n").length);
  input.rows = rows;
}

function wait(ms) {
  return new Promise((resolve) =>
    window.setTimeout(resolve, reducedMotion.matches ? Math.min(ms, 250) : ms),
  );
}

/* -------------------------------------------------------------- commands */

function printHelp() {
  gap();
  for (const command of COMMANDS) {
    say(`  ${command.usage.padEnd(24, " ")}${command.summary}`, "muted");
  }
}

function printInstaller() {
  gap();
  say("  one command. it installs a verified, immutable release.", "muted");
  pasteBlock(installCommand);
  say(
    "  needs macOS, Git, and Codex or Claude Code. Tohseno brings its own Bun.",
    "muted",
  );
  say("  iOS is the only implemented app platform.", "muted");
}

/// Prints the destination and, for another origin, opens it in a new tab so
/// the terminal a person is working in is never taken away from them.
function openLink(name) {
  const href = linkHrefs.get(name) ?? FALLBACK_LINKS[name];
  const external =
    /^https?:/.test(href) && !href.startsWith(`${window.location.origin}/`);
  gap();
  const row = el("p", "line");
  const anchor = el("a", "", href);
  anchor.href = href;
  if (external) {
    anchor.rel = "noopener noreferrer";
    anchor.target = "_blank";
  }
  row.append(document.createTextNode("  "), anchor);
  emit(row);
  if (external) window.open(href, "_blank", "noopener");
}

function beginComposition(requested) {
  if (!requested.trim()) {
    openComposer(DEFAULT_APP_NAME, "");
    gap();
    say("  what should it do?", "accent");
    say("  leave naming to Tohseno, or provide a name after create.", "muted");
    say("  be exact. the first version is built from these words alone.", "muted");
    say("  drop a .md file to fill this in. images too — up to eight.", "muted");
    gap();
    setMode("compose");
    saveDraft();
    return;
  }
  const name = sanitizeAppName(requested);
  const problem = appNameProblem(name);
  if (problem) {
    gap();
    say(`  ${problem}`, "warn");
    return;
  }
  openComposer(name, "");
  gap();
  if (name !== requested.trim()) say(`  → ${name}`, "muted");
  say(`  ${name} — what should it do?`, "accent");
  say("  be exact. the first version is built from these words alone.", "muted");
  say("  drop a .md file to fill this in. images too — up to eight.", "muted");
  gap();
  setMode("compose");
  saveDraft();
}

/// A sentence typed at the command prompt is what this page asked for. It
/// becomes the intention itself rather than an error, and the app name — which
/// is only a local label and never travels in the package — defaults quietly.
function beginIntention(text) {
  openComposer(state.appName ?? "my-app", text);
  gap();
  say("  that is an intention, not a command. even better.", "accent");
  say("  keep going, or send it. drag images in to attach them.", "muted");
  gap();
  setMode("compose");
  input.setSelectionRange(text.length, text.length);
  saveDraft();
}

function openComposer(name, prefill) {
  state.appName = name;
  state.references = [];
  state.prompt = "";
  input.value = prefill;
}

async function run(rawLine) {
  echoCommand(rawLine);
  state.history.push(rawLine);
  state.historyIndex = state.history.length;
  const intent = resolveCommand(rawLine);
  switch (intent.kind) {
    case "create":
      beginComposition(intent.argument);
      return;
    case "intention":
      beginIntention(intent.text);
      return;
    case "evolve":
      gap();
      say("  evolve runs where your apps live, not here.", "warn");
      say(
        `  on your Mac: tohseno evolve ${intent.argument.trim() || "<name>"}`,
        "muted",
      );
      say("  or from the Tohseno app, once it ships.", "muted");
      return;
    case "demo":
      await runDemo(state.appName ?? "my-app");
      return;
    case "install":
      printInstaller();
      return;
    case "link":
      openLink(intent.link);
      return;
    case "help":
      printHelp();
      return;
    case "clear":
      stopPolling();
      stream.replaceChildren();
      return;
    case "bare":
      gap();
      say("  tohseno create [name], or help.", "muted");
      return;
    case "empty":
      return;
    default:
      gap();
      say(`  command not found: ${intent.typed}`, "warn");
      say("  type help — or describe your app in a sentence to start one.", "muted");
  }
}

/* --------------------------------------------------------------- sending */

function submitComposition() {
  const prompt = input.value.replace(/\s+$/, "");
  if (!prompt.trim()) return;
  state.prompt = prompt;
  input.value = "";
  autosize();
  emit(el("p", "line indent muted", prompt));
  gap();
  say(`  ✓ ${intentSummary(prompt, state.references.length)}`, "accent");
  gap();
  say("  where should this go?");
  openChoices(DOORS, (door) => {
    if (door.id === "mac") return sendToMac();
    if (door.id === "app") return explainCompanion();
    return runDemo(state.appName);
  });
}

function offerRemainingDoors() {
  gap();
  openChoices(
    DOORS.filter((door) => door.id !== "app"),
    (door) => (door.id === "mac" ? sendToMac() : runDemo(state.appName)),
  );
}

function explainCompanion() {
  gap();
  say("  not yet — the Tohseno app for iPhone is not published.", "warn");
  gap();
  say("  what it will be, so you can judge it now:", "muted");
  say("  · you scan one code and this browser is linked to the app.", "muted");
  say("    like WhatsApp Web. there is no account, and never was one.", "muted");
  say("  · the phone holds the identity and does the signing. this site", "muted");
  say("    never holds a key, a hidden session, or your plaintext.", "muted");
  say("  · you unlink this browser from the phone, any time.", "muted");
  say("  · the app hands your intent to the Mac it is paired with. a phone", "muted");
  say("    is a remote control for your factory, not the factory.", "muted");
  gap();
  say("  until then, the Mac is the whole path:", "muted");
  offerRemainingDoors();
}

async function sendToMac() {
  stopPolling();
  gap();
  try {
    const limits = await relayCapabilities();
    say("  packaging…", "muted");
    const packageBytes = await buildIntentPackage({
      createdAt: new Date().toISOString(),
      prompt: state.prompt,
      references: state.references,
    });
    if (!limits.available) {
      offlineFallback(packageBytes);
      return;
    }
    say("  encrypting on this device…", "muted");
    const envelope = await createEncryptedEnvelope(packageBytes);
    const progress = say("  uploading…", "muted");
    const record = await uploadEnvelope(envelope, (done, total) => {
      const filled = "█".repeat(done);
      const empty = "░".repeat(Math.max(0, total - done));
      progress.textContent = `  uploading ${filled}${empty} ${done}/${total}`;
    });
    const token = claimToken(
      record.relay_id,
      envelope.capabilities.claim,
      envelope.key,
    );
    const days = Math.round(limits.relay_lifetime_seconds / 86_400);
    gap();
    say("  ✓ encrypted and waiting.", "accent");
    say(
      `  the relay stores ciphertext it cannot read. the key is inside this`,
      "muted",
    );
    say(
      `  command — one use, ${days} days. paste it on your Mac:`,
      "muted",
    );
    pasteBlock(claimCommand(installCommand, token));
    await clearDraft();
    followTransfer(record.relay_id, envelope.capabilities.status);
  } catch (error) {
    gap();
    say(`  ${error.message}`, "warn");
    say("  nothing was uploaded. edit and try again, or see a demo.", "muted");
    offerRemainingDoors();
  }
}

function offlineFallback(packageBytes) {
  const filename = `${state.appName}.tohseno-intent`;
  const url = URL.createObjectURL(
    new Blob([packageBytes], { type: "application/octet-stream" }),
  );
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 30_000);
  gap();
  say("  the encrypted handoff is closed right now.", "warn");
  say(`  downloaded ${filename} instead — private, but not encrypted.`, "muted");
  say("  install Tohseno, then open it:", "muted");
  pasteBlock(`tohseno intent open ~/Downloads/${filename}`);
  setMode("command");
}

function followTransfer(relayId, statusCapability) {
  gap();
  const row = say("  waiting for Tohseno…", "muted");
  setMode("command");
  const deadline = Date.now() + 30 * 60_000;
  const tick = async () => {
    try {
      const status = await relayStatus(relayId, statusCapability);
      // "ready" on the relay means uploaded and unclaimed, which the line
      // above already said. Here the only useful fact is who we are waiting on.
      row.textContent =
        status.state === "ready"
          ? "  waiting for Tohseno on your Mac…"
          : `  ${transferStateLabel(status.state)}`;
      if (status.state === "completed") {
        row.className = "line accent";
        row.textContent = "  ✓ imported on your Mac. Studio is open — finish it there.";
        stopPolling();
        return;
      }
      if (status.state === "expired" || status.state === "cancelled") {
        row.className = "line warn";
        stopPolling();
        return;
      }
    } catch {
      // A transient status failure is not a transfer failure; the record is
      // durable on the relay and the copied command still claims it.
    }
    if (Date.now() > deadline) {
      row.textContent = "  still waiting. the copied command works until it expires.";
      stopPolling();
    }
  };
  state.poll = window.setInterval(tick, 4_000);
  tick();
}

function stopPolling() {
  if (state.poll === null) return;
  window.clearInterval(state.poll);
  state.poll = null;
}

/* ------------------------------------------------------------------ demo */

async function runDemo(appName) {
  const token = (state.demoToken += 1);
  setMode("busy");
  gap();
  say("  replay — this is what your Mac shows. nothing is installed here.", "muted");
  say("  esc to skip.", "muted");
  gap();
  for (const step of DEMO_STEPS) {
    if (token !== state.demoToken) return;
    stateLine(step.state, demoHeadline(step, appName));
    if (step.detail) say(`    ${step.detail}`, "muted");
    await wait(step.holdMs);
  }
  if (token !== state.demoToken) return;
  gap();
  say("  that is the loop. describe a change, the update installs the same way.", "muted");
  say(`  on your Mac: tohseno evolve ${appName}`, "muted");
  gap();
  say("  run it for real:", "muted");
  pasteBlock(installCommand);
  setMode("command");
}

/* ------------------------------------------------------------ references */

/// Dropping or pasting onto the page is the whole gesture, from anywhere and
/// in any mode. A written document becomes the intention itself; images stay
/// what they are — reference material carried alongside it.
async function attachFiles(files) {
  const images = [];
  const documents = [];
  for (const file of [...files]) {
    const kind = classifyDroppedFile(file.name ?? "", file.type ?? "");
    if (kind === "image") images.push(file);
    else if (kind === "text") documents.push(file);
  }
  if (images.length === 0 && documents.length === 0) {
    gap();
    say("  images or a .md file. that one is neither.", "warn");
    return;
  }
  if (state.mode !== "compose") openDroppedComposition(documents[0]);
  for (const file of documents) await absorbDocument(file);
  for (const file of images) attachImage(file);
  autosize();
  saveDraft();
}

/// Opens a composer around what was dropped, taking the app name from the
/// document's own filename when it carries a usable one.
function openDroppedComposition(written) {
  cancelChoices();
  state.demoToken += 1;
  const named = written ? appNameFromFilename(written.name ?? "") : null;
  const name = named ?? state.appName ?? DEFAULT_APP_NAME;
  openComposer(name, "");
  gap();
  if (written) {
    say(`  ${name} — this is the intention now.`, "accent");
    say("  edit it here, then send it.", "muted");
  } else {
    say(`  ${name} — what should it do?`, "accent");
    say("  the images are attached. the app is still written in words.", "muted");
  }
  setMode("compose");
}

async function absorbDocument(file) {
  if (file.size > LIMITS.promptBytes) {
    say(`  ${file.name} is over 1 MiB of text. trim it, or open it on your Mac.`, "warn");
    return;
  }
  let text = "";
  try {
    text = await file.text();
  } catch {
    say(`  ${file.name} could not be read.`, "warn");
    return;
  }
  const merged = joinPromptText(input.value, text);
  if (!merged.trim()) {
    say(`  ${file.name} is empty.`, "warn");
    return;
  }
  if (utf8ByteLength(merged) > LIMITS.promptBytes) {
    say(`  ${file.name} would push this past 1 MiB of text.`, "warn");
    return;
  }
  input.value = merged;
  say(`  + ${file.name} · ${intentSummary(text, 0)}`, "muted");
}

function attachImage(file) {
  if (state.references.length >= LIMITS.references) {
    say(`  eight images is the limit — ${file.name} was not attached.`, "warn");
    return;
  }
  // Finder and some phone browsers hand over an image with no media type at
  // all. The filename decides it there, exactly as the package builder does.
  const mediaType = file.type || mediaTypeForFilename(file.name ?? "");
  if (!SUPPORTED_IMAGE_TYPES.has(mediaType)) {
    say(`  PNG, JPEG, HEIC, or WebP — ${file.name} was not attached.`, "warn");
    return;
  }
  if (file.size > LIMITS.referenceBytes) {
    say(`  ${file.name} is over 16 MiB. add it later on your Mac.`, "warn");
    return;
  }
  const originalFilename = toSafeReferenceName(
    file.name || "pasted-image",
    mediaType,
    state.references.length,
    state.references.map((reference) => reference.originalFilename),
  );
  state.references.push({ blob: file, originalFilename });
  say(`  + ${originalFilename} · ${formatBytes(file.size)}`, "muted");
}

/* ----------------------------------------------------------------- draft */

let draftTimer = null;

function saveDraft() {
  if (!draftStore || state.mode !== "compose") return;
  window.clearTimeout(draftTimer);
  draftTimer = window.setTimeout(() => {
    draftStore
      ?.save({
        appName: state.appName,
        prompt: input.value,
        references: state.references,
      })
      .catch(() => {});
  }, 400);
}

/// Cancels any queued write first: a save still in flight would otherwise
/// resurrect an intention the person just sent or cancelled.
async function clearDraft() {
  window.clearTimeout(draftTimer);
  state.references = [];
  state.prompt = "";
  await draftStore?.clear().catch(() => {});
}

async function restoreDraft() {
  const draft = await draftStore.load();
  if (!draft?.appName || !draft.prompt?.trim()) return;
  state.appName = draft.appName;
  state.references = Array.isArray(draft.references) ? draft.references : [];
  input.value = draft.prompt;
  gap();
  say(`  you left an unsent intent for ${draft.appName}. it is still here.`, "accent");
  for (const reference of state.references) {
    say(`  + ${reference.originalFilename}`, "muted");
  }
  setMode("compose");
}

/* ----------------------------------------------------------------- input */

form.addEventListener("submit", (event) => {
  event.preventDefault();
  if (state.mode === "compose") {
    submitComposition();
    return;
  }
  const line = input.value.trim();
  if (!line) return;
  input.value = "";
  autosize();
  run(line);
});

input.addEventListener("input", () => {
  autosize();
  saveDraft();
});

input.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    if (state.mode !== "compose") return;
    event.preventDefault();
    input.value = "";
    autosize();
    clearDraft();
    gap();
    say("  cancelled.", "muted");
    setMode("command");
    return;
  }
  if (event.key === "Enter") {
    if (event.shiftKey) return;
    if (state.mode === "command") {
      event.preventDefault();
      form.requestSubmit();
      return;
    }
    if (event.metaKey || event.ctrlKey || endsWithBlankLine(input.value)) {
      event.preventDefault();
      form.requestSubmit();
    }
    return;
  }
  if (state.mode !== "command" || state.history.length === 0) return;
  if (event.key === "ArrowUp") {
    event.preventDefault();
    state.historyIndex = Math.max(0, state.historyIndex - 1);
    input.value = state.history[state.historyIndex] ?? "";
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    state.historyIndex = Math.min(state.history.length, state.historyIndex + 1);
    input.value = state.history[state.historyIndex] ?? "";
  }
});

document.addEventListener("keydown", (event) => {
  if (state.mode !== "busy" || event.key !== "Escape") return;
  state.demoToken += 1;
  setMode("command");
});

document.addEventListener("paste", (event) => {
  const files = [...(event.clipboardData?.files ?? [])];
  if (files.length === 0) return;
  event.preventDefault();
  attachFiles(files);
});

document.addEventListener("dragenter", (event) => {
  event.preventDefault();
  state.dragDepth += 1;
  document.body.classList.add("dropping");
});

document.addEventListener("dragover", (event) => event.preventDefault());

document.addEventListener("dragleave", () => {
  state.dragDepth = Math.max(0, state.dragDepth - 1);
  if (state.dragDepth === 0) document.body.classList.remove("dropping");
});

document.addEventListener("drop", (event) => {
  event.preventDefault();
  state.dragDepth = 0;
  document.body.classList.remove("dropping");
  attachFiles(event.dataTransfer?.files ?? []);
});

/// A tap anywhere on the page is a request to write, and on a phone that is
/// the only way the keyboard opens: iOS raises it for a focus that happens
/// inside a real gesture, and ignores one that does not.
document.addEventListener("click", (event) => {
  if (form.hidden) return;
  if (window.getSelection()?.toString()) return;
  if (event.target.closest("a, button, textarea, .choices")) return;
  input.focus();
});

veil.addEventListener("click", () => {
  document.body.classList.remove("dropping");
  state.dragDepth = 0;
});

/// The software keyboard covers the bottom of the window without changing the
/// layout viewport, so the terminal is told how much of itself is behind it
/// and gives that space back. Otherwise the prompt writes into the keyboard.
const viewport = window.visualViewport;
if (viewport) {
  const fitAboveKeyboard = () => {
    const covered = window.innerHeight - viewport.height - viewport.offsetTop;
    // Only a keyboard takes this much of the window. Anything smaller is a
    // pinch zoom or a retreating address bar, and the layout ignores it.
    const keyboard = covered > 120 ? Math.round(covered) : 0;
    document.documentElement.style.setProperty("--keyboard", `${keyboard}px`);
    screen.scrollTop = screen.scrollHeight;
  };
  viewport.addEventListener("resize", fitAboveKeyboard);
  fitAboveKeyboard();
}

input.addEventListener("focus", () => {
  screen.scrollTop = screen.scrollHeight;
});

setMode("command");
openDraftStore().then(
  (store) => {
    draftStore = store;
    restoreDraft().catch(() => {});
  },
  () => {},
);
