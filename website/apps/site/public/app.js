import { buildIntentPackage, mediaTypeForFilename, prepareReferences, sha256Hex, utf8ByteLength } from "./modules/intent-package.js";
import { claimToken, createEncryptedEnvelope } from "./modules/intent-crypto.js";
import { openDraftStore } from "./modules/draft-store.js";
import { insertText, resolvePromptFile, transferStateLabel } from "./modules/draft-logic.js";
import { cancelTransfer, capabilities, relayStatus, uploadEnvelope } from "./modules/relay-client.js";
import "./modules/obsolete-worker-cleanup.js";

const ui = Object.freeze({
  installerCommand: document.querySelector("#installer-command"), copyInstaller: document.querySelector("#copy-installer"),
  prompt: document.querySelector("#prompt"), promptPicker: document.querySelector("#prompt-picker"),
  pickPrompt: document.querySelector("#pick-prompt"), promptChoice: document.querySelector("#prompt-import-choice"),
  promptChoiceLabel: document.querySelector("#prompt-import-label"), imagePicker: document.querySelector("#image-picker"),
  pickImages: document.querySelector("#pick-images"), referenceZone: document.querySelector("#reference-zone"),
  referenceCount: document.querySelector("#reference-count"), referenceList: document.querySelector("#reference-list"),
  validation: document.querySelector("#validation"), takeShot: document.querySelector("#take-shot"),
  persistence: document.querySelector("#persistence-status"), download: document.querySelector("#download-package"),
  draftView: document.querySelector("#draft-view"), transferView: document.querySelector("#transfer-view"),
  transferStage: document.querySelector("#transfer-stage"), transferTitle: document.querySelector("#transfer-title"),
  commandBlock: document.querySelector("#claim-command-block"), command: document.querySelector("#claim-command"),
  copyCommand: document.querySelector("#copy-command"), transferWaiting: document.querySelector("#transfer-waiting"),
  transferPrivacy: document.querySelector("#transfer-privacy"), transferDownload: document.querySelector("#transfer-download"),
  cancelTransfer: document.querySelector("#cancel-transfer"), another: document.querySelector("#another"),
  retryTransfer: document.querySelector("#retry-transfer"),
  unavailable: document.querySelector("#unavailable"), unavailableReason: document.querySelector("#unavailable-reason"),
});

const createdAt = new Date().toISOString();
let draft = { version: 1, prompt: "", references: [], createdAt, updatedAt: createdAt, transfer: null, lastTransferred: null };
let store = null;
let relayCapabilities = null;
let promptCandidate = null;
let saveTimer = 0;
let objectUrls = [];
let polling = false;

const showError = (message) => {
  ui.validation.textContent = message;
  ui.validation.hidden = !message;
};

const persistenceFailed = () => {
  ui.persistence.textContent = "Could not save locally — download a copy";
  ui.persistence.dataset.state = "failed";
};

const saveNow = async () => {
  window.clearTimeout(saveTimer);
  if (!store) { persistenceFailed(); return false; }
  ui.persistence.textContent = "Saving…";
  delete ui.persistence.dataset.state;
  draft.updatedAt = new Date().toISOString();
  try {
    await store.save(draft);
    ui.persistence.textContent = "Saved on this device";
    ui.persistence.dataset.state = "saved";
    return true;
  } catch { persistenceFailed(); return false; }
};

const scheduleSave = () => {
  ui.persistence.textContent = "Saving…";
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(saveNow, 450);
};

const syncPrompt = () => {
  draft.prompt = ui.prompt.value;
  ui.takeShot.disabled = draft.prompt.trim().length === 0;
  scheduleSave();
};

const renderReferences = () => {
  for (const url of objectUrls) URL.revokeObjectURL(url);
  objectUrls = [];
  ui.referenceList.replaceChildren();
  ui.referenceCount.textContent = `${draft.references.length} / 8`;
  for (const [index, reference] of draft.references.entries()) {
    const item = document.createElement("li");
    item.className = "reference-item";
    const preview = document.createElement("div");
    preview.className = "reference-preview";
    if (["image/png", "image/jpeg", "image/webp"].includes(reference.mimeType)) {
      const image = document.createElement("img");
      const url = URL.createObjectURL(reference.blob); objectUrls.push(url);
      image.src = url; image.alt = ""; preview.append(image);
    } else {
      const fallback = document.createElement("span"); fallback.textContent = reference.originalFilename.split(".").at(-1)?.toUpperCase() || "IMAGE"; preview.append(fallback);
    }
    const name = document.createElement("span"); name.className = "reference-name"; name.textContent = reference.originalFilename;
    const controls = document.createElement("div"); controls.className = "reference-controls";
    for (const [action, label, disabled] of [["up", "Move earlier", index === 0], ["down", "Move later", index === draft.references.length - 1], ["remove", "Remove", false]]) {
      const button = document.createElement("button"); button.type = "button"; button.dataset.referenceAction = action;
      button.dataset.index = String(index); button.textContent = action === "up" ? "↑" : action === "down" ? "↓" : "×";
      button.setAttribute("aria-label", `${label} ${reference.originalFilename}`); button.disabled = disabled; controls.append(button);
    }
    item.append(preview, name, controls); ui.referenceList.append(item);
  }
};

const addImages = async (files) => {
  showError("");
  const candidates = [...files].filter((file) => mediaTypeForFilename(file.name));
  if (candidates.length !== files.length) { showError("Use PNG, JPEG, HEIC, or WebP reference images."); return; }
  if (draft.references.length + candidates.length > 8) { showError("An intention accepts at most eight reference images."); return; }
  try {
    const incoming = candidates.map((file) => ({ blob: file, originalFilename: file.name, mimeType: mediaTypeForFilename(file.name), digest: "", order: 0 }));
    const prepared = await prepareReferences([...draft.references, ...incoming]);
    draft.references = prepared.map((reference, order) => ({ blob: reference.blob, originalFilename: reference.originalFilename, mimeType: reference.mediaType, digest: reference.digest, order }));
    renderReferences(); scheduleSave();
  } catch (error) { showError(error instanceof Error ? error.message : "Reference could not be added."); }
};

const readPromptFile = async (file) => {
  if (!/\.(?:md|markdown|txt)$/i.test(file.name) || file.size > 1024 * 1024) { showError("Prompt files must be .md, .markdown, or .txt and no larger than 1 MiB."); return; }
  let text;
  try { text = new TextDecoder("utf-8", { fatal: true }).decode(await file.arrayBuffer()); }
  catch { showError("Prompt files must contain valid UTF-8 text."); return; }
  if (!ui.prompt.value) { ui.prompt.value = text; syncPrompt(); return; }
  promptCandidate = { name: file.name, text };
  ui.promptChoiceLabel.textContent = `${file.name} is ready. Replace the current intention or append it?`;
  ui.promptChoice.hidden = false;
};

const packageBytes = () => buildIntentPackage({ createdAt: draft.createdAt, prompt: draft.prompt, references: draft.references });

const downloadPackage = async () => {
  showError("");
  try {
    const bytes = await packageBytes();
    const url = URL.createObjectURL(new Blob([bytes], { type: "application/vnd.tohseno.intent-package" }));
    const anchor = document.createElement("a"); anchor.href = url; anchor.download = "tohseno-intention.tohseno-intent"; anchor.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  } catch (error) { showError(error instanceof Error ? error.message : "The private package could not be created."); }
};

const renderTransfer = () => {
  const transfer = draft.transfer;
  if (!transfer) { ui.draftView.hidden = false; ui.transferView.hidden = true; return; }
  ui.draftView.hidden = true; ui.transferView.hidden = false;
  ui.transferStage.textContent = transferStateLabel(transfer.state);
  ui.transferTitle.textContent = ["ready", "waiting", "leased", "completed"].includes(transfer.state) ? "Your intention is ready." : transferStateLabel(transfer.state);
  const commandReady = Boolean(transfer.command && ["ready", "waiting", "leased"].includes(transfer.state));
  ui.commandBlock.hidden = !commandReady; ui.command.textContent = commandReady ? transfer.command : "";
  ui.transferPrivacy.hidden = !commandReady;
  ui.transferWaiting.textContent = transfer.state === "completed" ? "Found.\nOpening TOHSENO Studio…" : transferStateLabel(transfer.state === "ready" ? "waiting" : transfer.state);
  ui.cancelTransfer.hidden = !["uploading", "ready", "waiting"].includes(transfer.state);
  ui.retryTransfer.hidden = !["failed", "expired", "cancelled", "corrupt"].includes(transfer.state);
  ui.another.hidden = transfer.state !== "completed";
};

const beginTransfer = async () => {
  showError("");
  if (!draft.prompt.trim()) return;
  if (!store) {
    ui.unavailableReason.textContent = "IndexedDB is unavailable, so this browser cannot durably preserve an encrypted transfer while TOHSENO installs. Your current draft remains in memory.";
    ui.unavailable.hidden = false;
    showError("Download the private intent package before leaving this page, then open it locally with TOHSENO.");
    return;
  }
  if (!globalThis.crypto?.subtle || !crypto.getRandomValues) {
    ui.unavailableReason.textContent = "Web Crypto is unavailable, so this browser cannot create an encrypted handoff. No plaintext was uploaded.";
    ui.unavailable.hidden = false;
    showError("Download the private intent package, install TOHSENO, then open it locally.");
    return;
  }
  if (!relayCapabilities?.available) {
    ui.unavailableReason.textContent = relayCapabilities
      ? "Encrypted one-command handoff is not activated yet. Your draft remains only in this browser."
      : "The encrypted relay could not be reached. Your draft remains only in this browser.";
    ui.unavailable.hidden = false;
    showError("Download the private intent package, install TOHSENO, then open it locally.");
    return;
  }
  ui.takeShot.disabled = true;
  try {
    draft.transfer = { version: 1, state: "preparing", createdAt: new Date().toISOString() }; renderTransfer();
    if (!await saveNow()) throw new Error("The encrypted transfer was not started because its private state could not be saved locally.");
    const bytes = await packageBytes();
    draft.transfer.packageSha256 = await sha256Hex(bytes); draft.transfer.state = "encrypting"; renderTransfer();
    const envelope = await createEncryptedEnvelope(bytes);
    draft.transfer = {
      ...draft.transfer, state: "uploading", encryptedSnapshot: new Blob([envelope.ciphertext]),
      ciphertextSha256: envelope.ciphertextSha256, nonce: envelope.nonce, key: envelope.key,
      capabilities: envelope.capabilities, verifiers: envelope.verifiers, associatedData: envelope.associatedData,
    };
    if (!await saveNow()) throw new Error("The encrypted transfer was not started because its immutable snapshot could not be saved locally.");
    renderTransfer();
    const created = await uploadEnvelope(envelope, (current, total) => {
      ui.transferWaiting.textContent = `Uploading encrypted chunk ${current} of ${total}…`;
    }, async (record) => {
      draft.transfer.relayId = record.relay_id;
      draft.transfer.expiresAt = record.expires_at;
      if (!await saveNow()) throw new Error("The relay record was created, but its recovery state could not be saved locally.");
    });
    const token = claimToken(created.relay_id, envelope.capabilities.claim, envelope.key);
    draft.transfer = {
      ...draft.transfer, state: "ready", relayId: created.relay_id, expiresAt: created.expires_at,
      statusCapability: envelope.capabilities.status, uploadCapability: envelope.capabilities.upload,
      command: `curl -fsSL ${relayCapabilities.installer_url} | bash -s -- --claim '${token}'`,
    };
    if (!await saveNow()) {
      await cancelTransfer(created.relay_id, envelope.capabilities.upload).catch(() => {});
      throw new Error("The encrypted upload was cancelled because its claim state could not be saved locally.");
    }
    renderTransfer();
    draft.transfer.state = "waiting";
    await saveNow(); renderTransfer(); pollStatus();
  } catch (error) {
    const active = draft.transfer;
    const uploadCapability = active?.uploadCapability || active?.capabilities?.upload;
    if (active?.relayId && uploadCapability) {
      await cancelTransfer(active.relayId, uploadCapability).catch(() => {});
    }
    draft.transfer = { version: 1, state: "failed", errorClass: error instanceof Error ? error.constructor.name : "Error" };
    await saveNow(); renderTransfer(); showError(error instanceof Error ? error.message : "Encrypted handoff failed. The browser draft remains safe.");
  } finally { ui.takeShot.disabled = !draft.prompt.trim(); }
};

const pollStatus = async () => {
  if (polling || !draft.transfer?.relayId || !draft.transfer.statusCapability) return;
  polling = true;
  let delay = 1_500;
  while (draft.transfer?.relayId && draft.transfer.statusCapability) {
    if (document.hidden) { await wait(Math.max(delay, 10_000)); continue; }
    try {
      const status = await relayStatus(draft.transfer.relayId, draft.transfer.statusCapability);
      const next = status.state === "ready" ? "waiting" : status.state;
      draft.transfer.state = next; renderTransfer(); await saveNow();
      if (["completed", "expired", "cancelled", "corrupt"].includes(status.state)) {
        if (status.state === "completed") {
          draft.lastTransferred = { packageSha256: draft.transfer.packageSha256, completedAt: new Date().toISOString() };
          draft.transfer = { version: 1, state: "completed" };
          await saveNow(); renderTransfer();
        } else {
          draft.transfer = { version: 1, state: status.state };
          await saveNow(); renderTransfer();
        }
        break;
      }
      delay = Math.min(Math.round(delay * 1.5), 15_000);
    } catch { delay = Math.min(Math.round(delay * 1.7), 30_000); }
    await wait(delay);
  }
  polling = false;
};

const restore = async () => {
  try { store = await openDraftStore(); const saved = await store.load(); if (saved?.version === 1) draft = saved; }
  catch { persistenceFailed(); }
  ui.prompt.value = draft.prompt || ""; renderReferences(); ui.takeShot.disabled = !draft.prompt?.trim();
  if (store) await saveNow();
  if (draft.transfer) {
    renderTransfer();
    if (["ready", "waiting", "leased"].includes(draft.transfer.state)) pollStatus();
    if (draft.transfer.state === "uploading" && draft.transfer.relayId && draft.transfer.encryptedSnapshot) resumeUpload();
  }
};

const resumeUpload = async () => {
  try {
    relayCapabilities ||= await capabilities();
    const transfer = draft.transfer;
    const envelope = {
      ciphertext: new Uint8Array(await transfer.encryptedSnapshot.arrayBuffer()),
      ciphertextSha256: transfer.ciphertextSha256, nonce: transfer.nonce, key: transfer.key,
      capabilities: transfer.capabilities, verifiers: transfer.verifiers, associatedData: transfer.associatedData,
    };
    const created = await uploadEnvelope(envelope, null, null, { relay_id: transfer.relayId, expires_at: transfer.expiresAt });
    const token = claimToken(created.relay_id, envelope.capabilities.claim, envelope.key);
    draft.transfer = { ...transfer, state: "waiting", statusCapability: envelope.capabilities.status,
      uploadCapability: envelope.capabilities.upload,
      command: `curl -fsSL ${relayCapabilities.installer_url} | bash -s -- --claim '${token}'` };
    await saveNow(); renderTransfer(); pollStatus();
  } catch (error) {
    draft.transfer = { version: 1, state: "failed", errorClass: error instanceof Error ? error.constructor.name : "Error" };
    await saveNow(); renderTransfer();
  }
};

ui.prompt.addEventListener("input", syncPrompt);
ui.pickPrompt.addEventListener("click", () => ui.promptPicker.click());
ui.promptPicker.addEventListener("change", () => { const file = ui.promptPicker.files?.[0]; if (file) readPromptFile(file); ui.promptPicker.value = ""; });
ui.pickImages.addEventListener("click", (event) => { event.stopPropagation(); ui.imagePicker.click(); });
ui.referenceZone.addEventListener("click", (event) => { if (event.target !== ui.pickImages) ui.imagePicker.click(); });
ui.referenceZone.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); ui.imagePicker.click(); } });
ui.imagePicker.addEventListener("change", () => { if (ui.imagePicker.files) addImages(ui.imagePicker.files); ui.imagePicker.value = ""; });

for (const target of [ui.prompt, ui.referenceZone]) {
  target.addEventListener("dragover", (event) => { event.preventDefault(); ui.referenceZone.dataset.dragging = "true"; });
  target.addEventListener("dragleave", () => delete ui.referenceZone.dataset.dragging);
  target.addEventListener("drop", (event) => {
    event.preventDefault(); delete ui.referenceZone.dataset.dragging;
    const files = [...(event.dataTransfer?.files || [])];
    const promptFiles = files.filter((file) => /\.(?:md|markdown|txt)$/i.test(file.name));
    const images = files.filter((file) => mediaTypeForFilename(file.name));
    if (promptFiles[0]) readPromptFile(promptFiles[0]); if (images.length) addImages(images);
    if (promptFiles.length + images.length !== files.length) showError("Some dropped files were not supported.");
  });
}

ui.prompt.addEventListener("paste", (event) => {
  const file = [...(event.clipboardData?.items || [])].find((item) => item.kind === "file")?.getAsFile();
  if (file && /\.(?:md|markdown|txt)$/i.test(file.name)) { event.preventDefault(); readPromptFile(file); }
});

ui.promptChoice.addEventListener("click", (event) => {
  const action = event.target instanceof HTMLButtonElement ? event.target.dataset.promptAction : null;
  if (!action) return;
  if (promptCandidate && action !== "cancel") { ui.prompt.value = resolvePromptFile(ui.prompt.value, promptCandidate.text, action); syncPrompt(); }
  promptCandidate = null; ui.promptChoice.hidden = true;
});

ui.referenceList.addEventListener("click", (event) => {
  const button = event.target instanceof HTMLButtonElement ? event.target : null;
  const index = Number(button?.dataset.index); const action = button?.dataset.referenceAction;
  if (!Number.isInteger(index) || !action) return;
  if (action === "remove") draft.references.splice(index, 1);
  if (action === "up" && index > 0) [draft.references[index - 1], draft.references[index]] = [draft.references[index], draft.references[index - 1]];
  if (action === "down" && index + 1 < draft.references.length) [draft.references[index + 1], draft.references[index]] = [draft.references[index], draft.references[index + 1]];
  draft.references.forEach((reference, order) => { reference.order = order; }); renderReferences(); scheduleSave();
});

for (const example of document.querySelectorAll("[data-example]")) {
  example.addEventListener("click", () => {
    const result = insertText(ui.prompt.value, example.textContent, ui.prompt.selectionStart, ui.prompt.selectionEnd);
    ui.prompt.value = result.value; ui.prompt.focus(); ui.prompt.setSelectionRange(result.cursor, result.cursor); syncPrompt();
  });
}

ui.takeShot.addEventListener("click", beginTransfer);
ui.download.addEventListener("click", downloadPackage); ui.transferDownload.addEventListener("click", downloadPackage);
ui.copyInstaller.addEventListener("click", async () => { try { await navigator.clipboard.writeText(ui.installerCommand.textContent); ui.copyInstaller.textContent = "COPIED"; window.setTimeout(() => { ui.copyInstaller.textContent = "COPY"; }, 1800); } catch { ui.installerCommand.focus(); } });
ui.copyCommand.addEventListener("click", async () => { try { await navigator.clipboard.writeText(ui.command.textContent); ui.copyCommand.textContent = "COPIED"; window.setTimeout(() => { ui.copyCommand.textContent = "COPY"; }, 1800); } catch { ui.command.focus(); } });
ui.cancelTransfer.addEventListener("click", async () => {
  const transfer = draft.transfer;
  try { if (transfer?.relayId && transfer.uploadCapability) await cancelTransfer(transfer.relayId, transfer.uploadCapability); }
  catch { /* expiry and already-claimed states remain visible through polling */ }
  draft.transfer = { version: 1, state: "cancelled" }; await saveNow(); renderTransfer();
});
ui.retryTransfer.addEventListener("click", async () => {
  draft.transfer = null; await saveNow(); renderTransfer(); ui.prompt.focus();
});
ui.another.addEventListener("click", async () => {
  const now = new Date().toISOString(); draft = { version: 1, prompt: "", references: [], createdAt: now, updatedAt: now, transfer: null, lastTransferred: null };
  ui.prompt.value = ""; ui.unavailable.hidden = true; showError(""); renderReferences(); renderTransfer(); ui.takeShot.disabled = true; await saveNow(); ui.prompt.focus();
});

document.addEventListener("visibilitychange", () => { if (document.hidden) saveNow(); });
window.addEventListener("pagehide", () => {
  for (const url of objectUrls) URL.revokeObjectURL(url);
  objectUrls = [];
  if (saveTimer) saveNow();
});

const wait = (milliseconds) => new Promise((resolve) => window.setTimeout(resolve, milliseconds));
capabilities().then((value) => { relayCapabilities = value; }, () => { relayCapabilities = null; });
restore();
