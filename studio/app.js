"use strict";

// TOHSENO Studio.
//
// One product law: App → Intent → App on your iPhone. This script renders four
// views over the Local Workspace Service and nothing else. The factory's
// internal phases, identities, and lineage never reach the normal path; the
// service publishes one human `presentation` per app and Studio shows it.

const ui = {
  shell: document.querySelector("#shell"),
  home: document.querySelector("#home"),
  openSettings: document.querySelector("#open-settings"),
  appsLoading: document.querySelector("#apps-loading"),
  appsLoadingMessage: document.querySelector("#apps-loading-message"),
  appGrid: document.querySelector("#app-grid"),
  appsEmpty: document.querySelector("#apps-empty"),
  newApp: document.querySelector("#new-app"),
  selectionEmpty: document.querySelector("#selection-empty"),
  composeForm: document.querySelector("#compose-form"),
  composeName: document.querySelector("#compose-name"),
  composeTitle: document.querySelector("#compose-title"),
  composeQuestion: document.querySelector("#compose-question"),
  composeIntent: document.querySelector("#compose-intent"),
  composeStatus: document.querySelector("#compose-status"),
  composeSubmit: document.querySelector("#compose-submit"),
  referenceInput: document.querySelector("#reference-input"),
  referenceDrop: document.querySelector("#reference-drop"),
  referenceList: document.querySelector("#reference-list"),
  stateKicker: document.querySelector("#state-kicker"),
  stateHeadline: document.querySelector("#state-headline"),
  stateDetail: document.querySelector("#state-detail"),
  statePulse: document.querySelector("#state-pulse"),
  stateRetry: document.querySelector("#state-retry"),
  stateEvolve: document.querySelector("#state-evolve"),
  stateDetails: document.querySelector("#state-details"),
  detailFacts: document.querySelector("#detail-facts"),
  previewPanel: document.querySelector("#preview-panel"),
  previewImage: document.querySelector("#preview-image"),
  previewFallback: document.querySelector("#preview-fallback"),
  previewIcon: document.querySelector("#preview-icon"),
  previewName: document.querySelector("#preview-name"),
  previewMessage: document.querySelector("#preview-message"),
  previewStatus: document.querySelector("#preview-status"),
  deviceSummary: document.querySelector("#device-summary"),
  deviceList: document.querySelector("#device-list"),
  diagnostics: document.querySelector("#diagnostics"),
  gateKicker: document.querySelector("#gate-kicker"),
  gateHeadline: document.querySelector("#gate-headline"),
  gateDetail: document.querySelector("#gate-detail"),
  gatePrimary: document.querySelector("#gate-primary"),
  gateBack: document.querySelector("#gate-back"),
  gatePrices: document.querySelector("#gate-prices"),
  proMonthly: document.querySelector("#pro-monthly"),
  proYearly: document.querySelector("#pro-yearly"),
  proNotNow: document.querySelector("#pro-not-now"),
  toast: document.querySelector("#toast"),
};

const MAX_REFERENCES = 8;
const MAX_REFERENCE_BYTES = 64 * 1024 * 1024;
const MAX_REFERENCE_TOTAL_BYTES = 160 * 1024 * 1024;

const state = {
  csrfToken: null,
  sessionInstanceId: null,
  sessionPromise: null,
  health: null,
  factory: null,
  workspace: null,
  shots: [],
  devices: [],
  companion: null,
  entitlement: null,
  genesis: null,
  gateAction: null,
  view: "apps",
  selectedShotId: null,
  composeMode: "create",
  pendingIntentionId: null,
  // Retained only in this browser tab so a failed command can be retried
  // without retyping. Nothing is written to disk from here.
  drafts: new Map(),
  commandId: null,
  eventSource: null,
  refreshTimer: null,
  toastTimer: null,
};

class ApiError extends Error {
  constructor(message, status, code) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

function element(tag, className, text) {
  const value = document.createElement(tag);
  if (className) value.className = className;
  if (text !== undefined) value.textContent = text;
  return value;
}

function fact(list, term, value, title) {
  if (!value) return;
  const row = element("div");
  row.append(element("dt", null, term));
  const detail = element("dd", null, value);
  if (title) detail.title = title;
  row.append(detail);
  list.append(row);
}

/* -------------------------------------------------------------- references */

class ReferencePicker {
  constructor({ input, drop, list }) {
    this.input = input;
    this.drop = drop;
    this.list = list;
    this.files = [];
    this.locked = false;
    input.addEventListener("change", () => {
      this.add(input.files);
      input.value = "";
    });
    drop.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        input.click();
      }
    });
    for (const name of ["dragenter", "dragover"]) {
      drop.addEventListener(name, (event) => {
        event.preventDefault();
        drop.classList.add("dragging");
      });
    }
    for (const name of ["dragleave", "drop"]) {
      drop.addEventListener(name, (event) => {
        event.preventDefault();
        drop.classList.remove("dragging");
      });
    }
    drop.addEventListener("drop", (event) => this.add(event.dataTransfer?.files));
  }

  add(values, originForFile = null, force = false) {
    if (this.locked && !force) return;
    let message = "";
    for (const file of Array.from(values || [])) {
      const mediaType = referenceMediaType(file);
      const total = this.files.reduce((sum, entry) => sum + entry.file.size, 0);
      if (this.files.length >= MAX_REFERENCES) {
        message = "Only eight images can be attached.";
        break;
      }
      if (!mediaType) message = `${file.name || "That file"} is not a supported image.`;
      else if (!file.name || file.name.includes("/") || file.name.includes("\\")) {
        message = "Image filenames must not contain a path.";
      } else if (file.size < 1 || file.size > MAX_REFERENCE_BYTES) {
        message = `${file.name} must contain 1 byte to 64 MB.`;
      } else if (this.files.some((entry) => entry.file.name === file.name)) {
        message = `${file.name} is already attached.`;
      } else if (total + file.size > MAX_REFERENCE_TOTAL_BYTES) {
        message = "Those images exceed the 160 MB combined limit.";
        break;
      } else {
        this.files.push({
          file,
          mediaType,
          origin: typeof originForFile === "function" ? originForFile(file) : null,
        });
        continue;
      }
    }
    if (message) ui.composeStatus.textContent = message;
    state.commandId = null;
    this.render();
  }

  clear(force = false) {
    if (this.locked && !force) return;
    this.files = [];
    this.render();
  }

  setLocked(locked) {
    this.locked = Boolean(locked);
    this.input.disabled = this.locked;
    this.drop.setAttribute("aria-disabled", String(this.locked));
    this.render();
  }

  render() {
    this.list.replaceChildren();
    this.files.forEach((entry, index) => {
      const row = element("div", "reference-item");
      row.append(element("span", null, entry.file.name));
      const remove = element("button", null, "×");
      remove.type = "button";
      remove.disabled = this.locked;
      remove.setAttribute("aria-label", `Remove ${entry.file.name}`);
      remove.addEventListener("click", () => {
        this.files.splice(index, 1);
        state.commandId = null;
        this.render();
      });
      row.append(remove);
      this.list.append(row);
    });
  }

  async descriptors() {
    const descriptors = [];
    for (const entry of this.files) {
      const bytes = new Uint8Array(await entry.file.arrayBuffer());
      if (bytes.byteLength !== entry.file.size) {
        throw new Error(`${entry.file.name} changed while Studio was reading it.`);
      }
      descriptors.push({
        filename: entry.file.name,
        media_type: entry.mediaType,
        origin: entry.origin || `studio-file:${entry.file.name}`,
        bytes_base64url: bytesToBase64(bytes, true),
      });
    }
    return descriptors;
  }
}

const references = new ReferencePicker({
  input: ui.referenceInput,
  drop: ui.referenceDrop,
  list: ui.referenceList,
});

function referenceMediaType(file) {
  const declared = String(file.type || "").toLowerCase();
  if (["image/png", "image/jpeg", "image/heic", "image/webp"].includes(declared)) return declared;
  return {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    heic: "image/heic",
    webp: "image/webp",
  }[file.name.split(".").pop()?.toLowerCase()] || null;
}

function bytesToBase64(bytes, urlSafe = false) {
  const chunks = [];
  for (let offset = 0; offset < bytes.length; offset += 32 * 1024) {
    chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 32 * 1024)));
  }
  const encoded = btoa(chunks.join(""));
  return urlSafe ? encoded.replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "") : encoded;
}

function base64UrlToBytes(value) {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) throw new Error("A prefilled image is not canonical base64url.");
  const padded = value.replaceAll("-", "+").replaceAll("_", "/") + "=".repeat((4 - (value.length % 4)) % 4);
  const bytes = Uint8Array.from(atob(padded), (character) => character.charCodeAt(0));
  if (bytesToBase64(bytes, true) !== value) throw new Error("A prefilled image is not canonical base64url.");
  return bytes;
}

/* --------------------------------------------------------------------- api */

async function refreshStudioSession(force = false) {
  if (!force && state.csrfToken) return;
  if (!force && state.sessionPromise) return state.sessionPromise;
  state.sessionPromise = (async () => {
    const response = await fetch("/api/v1/studio-session", {
      cache: "no-store",
      credentials: "same-origin",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) throw new Error(`Studio session bootstrap failed (${response.status}).`);
    const session = await response.json();
    if (
      session.schema !== "tohseno.local-studio-session/1" ||
      session.origin !== window.location.origin ||
      typeof session.csrf_token !== "string" ||
      session.csrf_token.length < 32
    ) {
      throw new Error("Your TOHSENO returned an invalid Studio session.");
    }
    state.csrfToken = session.csrf_token;
    state.sessionInstanceId = session.instance_id;
  })();
  try {
    await state.sessionPromise;
  } finally {
    state.sessionPromise = null;
  }
}

async function api(path, options = {}, retryCsrf = true) {
  const method = String(options.method || "GET").toUpperCase();
  const mutation = ["POST", "PUT", "PATCH", "DELETE"].includes(method);
  if (mutation) await refreshStudioSession();
  const headers = new Headers(options.headers || {});
  headers.set("Accept", "application/json");
  if (mutation) {
    headers.set("Content-Type", "application/json");
    headers.set("X-Tohseno-CSRF", state.csrfToken);
  }
  const response = await fetch(path, {
    ...options,
    method,
    body: mutation && options.body === undefined ? "{}" : options.body,
    cache: "no-store",
    credentials: "same-origin",
    headers,
  });
  if (response.status === 204) return null;
  const body = await response.json().catch(() => null);
  if (!response.ok) {
    if (mutation && retryCsrf && response.status === 403 && body?.code === "csrf_rejected") {
      state.csrfToken = null;
      await refreshStudioSession(true);
      return api(path, options, false);
    }
    throw new ApiError(
      body?.message || body?.error || `Studio request failed (${response.status}).`,
      response.status,
      body?.code,
    );
  }
  return body;
}

/* ------------------------------------------------------------------- state */

function selectedShot() {
  return state.shots.find((shot) => shot.shot_id === state.selectedShotId) || null;
}

async function refreshWorkspace() {
  const snapshot = await api("/api/v1/workspace");
  if (snapshot.schema !== "tohseno.companion-workspace-snapshot/1" || !Array.isArray(snapshot.shots)) {
    throw new Error("This Studio does not support the workspace snapshot it received.");
  }
  state.workspace = snapshot;
  state.shots = [...snapshot.shots].sort((left, right) => {
    if (left.sort_index !== right.sort_index) return left.sort_index - right.sort_index;
    return left.display_name.localeCompare(right.display_name);
  });
  render();
}

async function refreshDevices() {
  const [devices, companion] = await Promise.all([
    api("/api/v1/companion/devices"),
    api("/api/v1/companion/status"),
  ]);
  state.devices = Array.isArray(devices.devices) ? devices.devices : [];
  state.companion = companion;
  if (state.view === "settings") renderSettings();
}

async function refreshProductBoundary() {
  const entitlement = await api("/api/v1/entitlement");
  state.entitlement = entitlement;
  if (entitlement.phase === "genesis_incomplete") {
    state.genesis = await api("/api/v1/genesis");
    state.view = "gate";
  } else if (["trial_qualified", "trial_expired", "pro_lapsed"].includes(entitlement.phase)) {
    state.view = "gate";
  } else if (state.view === "gate") {
    state.view = "apps";
  }
  render();
}

function scheduleRefresh() {
  window.clearTimeout(state.refreshTimer);
  state.refreshTimer = window.setTimeout(() => {
    refreshProductBoundary()
      .then(() => {
        if (state.view !== "gate") return refreshWorkspace();
      })
      .catch((error) => showToast(error.message, true));
    if (state.view === "settings") refreshDevices().catch(() => {});
  }, 200);
}

function connectEvents() {
  state.eventSource?.close();
  const source = new EventSource("/api/v1/events");
  state.eventSource = source;
  source.addEventListener("open", scheduleRefresh);
  source.addEventListener("workspace.changed", scheduleRefresh);
  source.addEventListener("workspace.reconcile", scheduleRefresh);
  source.addEventListener("error", () => ui.shell.setAttribute("data-offline", "true"));
}

/* ------------------------------------------------------------------ render */

function render() {
  ui.shell.dataset.view = state.view;
  const normal = ["apps", "compose", "state"].includes(state.view);
  for (const [name, node] of [
    ["apps", document.querySelector("#apps-view")],
    ["compose", document.querySelector("#compose-view")],
    ["state", document.querySelector("#state-view")],
    ["settings", document.querySelector("#settings-view")],
    ["gate", document.querySelector("#gate-view")],
  ]) {
    node.hidden = name === "apps" ? !normal : state.view !== name;
  }
  ui.selectionEmpty.hidden = state.view !== "apps";
  ui.previewPanel.hidden = !normal;
  if (normal) {
    renderApps();
    renderPreview();
  }
  if (state.view === "state") renderAppState();
  if (state.view === "settings") renderSettings();
  if (state.view === "gate") renderGate();
}

function renderGate() {
  ui.gatePrices.hidden = true;
  ui.gatePrimary.hidden = true;
  ui.gateBack.hidden = true;
  state.gateAction = null;
  const entitlement = state.entitlement;
  if (!entitlement) return;
  if (entitlement.phase === "genesis_incomplete") {
    const genesis = state.genesis;
    if (!genesis) return;
    ui.gateKicker.textContent = "TOHSENO on your iPhone";
    ui.gateHeadline.textContent = genesis.instruction;
    ui.gateDetail.textContent = genesis.detail || "";
    if (genesis.primary_action) {
      state.gateAction = genesis.primary_action;
      ui.gatePrimary.textContent = {
        begin: "Begin",
        continue: "Continue",
        open_app_store: "Open the App Store",
        open_xcode_accounts: "Open Xcode",
        install_companion: "Install TOHSENO",
        create_app: "Take a Shot",
      }[genesis.primary_action] || "Continue";
      ui.gatePrimary.hidden = false;
    }
    ui.gateBack.hidden = !genesis.can_go_back;
    return;
  }
  if (entitlement.phase === "trial_expired") {
    ui.gateKicker.textContent = "";
    ui.gateHeadline.textContent = "Your TOHSENO trial has ended.";
    ui.gateDetail.textContent = "Everything you made is still here. Only people who completed five successful days are qualified to purchase Pro.";
    return;
  }
  ui.gateKicker.textContent = "TOHSENO Pro";
  ui.gateHeadline.textContent = entitlement.phase === "pro_lapsed"
    ? "Continue with TOHSENO Pro."
    : "You made software on five different days.";
  ui.gateDetail.textContent = entitlement.phase === "pro_lapsed"
    ? "Your apps and everything you made are still here."
    : "Continue with TOHSENO Pro.";
  ui.gatePrices.hidden = false;
}

function renderApps() {
  ui.appGrid.replaceChildren();
  const loading = state.workspace === null;
  ui.appsLoading.hidden = !loading;
  ui.appGrid.hidden = loading;
  ui.newApp.hidden = loading;
  if (loading) {
    ui.appsEmpty.hidden = true;
    return;
  }
  ui.appsEmpty.hidden = state.shots.length > 0;
  for (const shot of state.shots) {
    const card = element("button", "app-card");
    card.type = "button";
    const status = cardStatus(shot);
    card.setAttribute("aria-label", `Open ${shot.display_name}, ${status}`);
    card.title = `${shot.display_name} · ${status}`;
    card.dataset.state = shot.presentation.state;
    card.dataset.selected = String(shot.shot_id === state.selectedShotId);
    const icon = element("img", "app-icon");
    icon.alt = "";
    icon.loading = "lazy";
    icon.decoding = "async";
    icon.src = `/api/v1/shots/${encodeURIComponent(shot.shot_id)}/icon?revision=${encodeURIComponent(shot.icon.revision)}`;
    icon.addEventListener("error", () => {
      if (!icon.src.endsWith("/tohseno-logo.png")) icon.src = "/tohseno-logo.png";
    });
    const dot = element("span", "app-status-dot");
    dot.setAttribute("aria-hidden", "true");
    const iconShell = element("span", "app-icon-shell");
    iconShell.append(icon, dot);
    card.append(iconShell, element("span", "app-icon-name", shot.display_name));
    card.addEventListener("click", () => openApp(shot.shot_id));
    ui.appGrid.append(card);
  }
}

function renderPreview() {
  const shot = selectedShot();
  ui.previewImage.hidden = true;
  ui.previewFallback.hidden = false;
  ui.previewImage.onload = null;
  ui.previewImage.removeAttribute("src");
  if (!shot) {
    ui.previewStatus.textContent = state.composeMode === "create" && state.view === "compose" ? "New app" : "";
    ui.previewName.textContent = state.view === "compose" ? (ui.composeName.value || "New app") : "Choose an app";
    ui.previewMessage.textContent = state.view === "compose"
      ? "Your app preview will appear after its first accepted build."
      : "Its latest accepted screen will appear here.";
    ui.previewIcon.src = "/tohseno-logo.png";
    return;
  }
  ui.previewName.textContent = shot.display_name;
  ui.previewMessage.textContent = shot.presentation.detail || shot.presentation.headline;
  ui.previewStatus.textContent = cardStatus(shot);
  ui.previewIcon.src = `/api/v1/shots/${encodeURIComponent(shot.shot_id)}/icon?revision=${encodeURIComponent(shot.icon.revision)}`;
  if (shot.latest_version_id) {
    ui.previewImage.alt = `${shot.display_name} latest accepted screen`;
    ui.previewImage.onload = () => {
      ui.previewImage.hidden = false;
      ui.previewFallback.hidden = true;
    };
    ui.previewImage.src = `/api/v1/shots/${encodeURIComponent(shot.shot_id)}/preview?revision=${encodeURIComponent(shot.latest_version_id)}`;
  }
}

function cardStatus(shot) {
  if (shot.kind === "recording_only") return "Recording folder";
  return {
    waiting: "Waiting",
    building: "Building",
    ready_for_phone: "Ready for your iPhone",
    installing: "Installing",
    installed: "On your iPhone",
    failed: "Failed",
  }[shot.presentation.state] || "Waiting";
}

function renderAppState() {
  const shot = selectedShot();
  if (!shot) {
    // The snapshot has not caught up with a just-admitted command yet.
    ui.stateKicker.textContent = "";
    ui.stateHeadline.textContent = "Waiting to build…";
    ui.stateDetail.textContent = "";
    ui.statePulse.hidden = false;
    ui.stateRetry.hidden = true;
    ui.stateEvolve.hidden = true;
    ui.detailFacts.replaceChildren();
    return;
  }
  const presentation = shot.presentation;
  ui.stateKicker.textContent = presentation.state === "installed" ? "" : shot.display_name;
  ui.stateHeadline.textContent = presentation.headline;
  ui.stateDetail.textContent = presentation.state === "failed" ? "" : presentation.detail || "";
  ui.statePulse.hidden = !["waiting", "building", "installing"].includes(presentation.state);
  ui.stateRetry.hidden = presentation.state !== "failed";
  ui.stateEvolve.hidden = presentation.state !== "installed" || shot.kind !== "factory_shot";
  ui.stateEvolve.textContent = `Evolve ${shot.display_name}`;
  renderDetails(shot);
}

function renderDetails(shot) {
  ui.detailFacts.replaceChildren();
  const execution = shot.execution;
  fact(ui.detailFacts, "Status", shot.presentation.state.replaceAll("_", " "));
  if (execution) {
    fact(ui.detailFacts, "Execution phase", execution.state.replaceAll("_", " "));
    fact(ui.detailFacts, "Execution", abbreviate(execution.execution_id), execution.execution_id);
    fact(ui.detailFacts, "Execution elapsed", formatDuration(execution.elapsed_seconds));
    fact(ui.detailFacts, "Started", formatTimestamp(execution.started_at), execution.started_at);
    fact(ui.detailFacts, "Updated", formatTimestamp(execution.updated_at), execution.updated_at);
    if (execution.state_transition) {
      fact(
        ui.detailFacts,
        "Persistent state",
        execution.state_transition.persistent_state.replaceAll("_", " "),
      );
      fact(ui.detailFacts, "State transition", execution.state_transition.summary);
      if (execution.state_transition.migrations.length) {
        fact(ui.detailFacts, "Migrations", execution.state_transition.migrations.join(", "));
      }
    }
  }
  fact(ui.detailFacts, "App", abbreviate(shot.shot_id), shot.shot_id);
  if (shot.expression_id) fact(ui.detailFacts, "Expression", abbreviate(shot.expression_id), shot.expression_id);
  if (shot.latest_version_id) {
    fact(
      ui.detailFacts,
      "Accepted version",
      `#${shot.latest_version_ordinal} · ${abbreviate(shot.latest_version_id)}`,
      shot.latest_version_id,
    );
  }
  if (shot.latest_version_created_at) {
    fact(ui.detailFacts, "Accepted at", formatTimestamp(shot.latest_version_created_at));
  }
  if (shot.bundle_identifier) fact(ui.detailFacts, "Bundle", shot.bundle_identifier);
  if (state.factory) {
    fact(ui.detailFacts, "Coding harness", state.factory.harness_label);
    fact(
      ui.detailFacts,
      "Inference route",
      [state.factory.model_label || state.factory.model_id, state.factory.route_label || state.factory.route_id]
        .filter(Boolean)
        .join(" · "),
    );
  }
  if (shot.failure_reason) fact(ui.detailFacts, "Reason", shot.failure_reason);
}

function renderSettings() {
  const active = state.devices.filter((device) => !device.revoked);
  ui.deviceSummary.textContent = active.length
    ? `${active.length} iPhone${active.length === 1 ? "" : "s"} can create and evolve apps on this Mac.`
    : "No iPhone connected.";
  ui.deviceList.replaceChildren();
  for (const device of active) {
    const row = element("div", "device-row");
    row.append(element("strong", null, device.display_name));
    const revoke = element("button", "quiet", "Revoke");
    revoke.type = "button";
    revoke.addEventListener("click", () => revokeDevice(device));
    row.append(revoke);
    ui.deviceList.append(row);
  }
  ui.diagnostics.replaceChildren();
  fact(ui.diagnostics, "TOHSENO", state.health?.service_version);
  fact(ui.diagnostics, "Local address", state.health?.origin);
  fact(ui.diagnostics, "Workspace", abbreviate(state.health?.workspace_id), state.health?.workspace_id);
  fact(ui.diagnostics, "Coding harness", state.factory?.harness_label || "None available");
  fact(
    ui.diagnostics,
    "Inference route",
    [state.factory?.model_label || state.factory?.model_id, state.factory?.route_label || state.factory?.route_id]
      .filter(Boolean)
      .join(" · ") || "None available",
  );
  fact(ui.diagnostics, "Private channel", String(state.companion?.relay_connection || "configuration required").replaceAll("_", " "));
}

/* ---------------------------------------------------------------- composer */

function openCompose({ mode, name = "", shot = null, intent = "", updateLocation = true }) {
  state.view = "compose";
  state.composeMode = mode;
  state.commandId = null;
  ui.composeStatus.textContent = "";
  ui.composeIntent.value = intent;
  references.clear(true);
  references.setLocked(false);
  if (mode === "create") {
    state.selectedShotId = null;
    ui.composeName.hidden = false;
    ui.composeTitle.hidden = true;
    ui.composeName.value = name;
    ui.composeQuestion.textContent = "What do you want this app to be?";
    ui.composeIntent.placeholder = "Write or paste your intent here…";
    ui.composeSubmit.textContent = "Create App";
    if (updateLocation) {
      history.replaceState(null, "", name ? `/create?name=${encodeURIComponent(name)}` : "/create");
    }
  } else {
    state.selectedShotId = shot.shot_id;
    ui.composeName.hidden = true;
    ui.composeTitle.hidden = false;
    ui.composeTitle.textContent = shot.display_name;
    ui.composeQuestion.textContent = "What should change?";
    ui.composeIntent.placeholder = "Describe what should become different…";
    ui.composeSubmit.textContent = "Evolve App";
    if (updateLocation) history.replaceState(null, "", `/shots/${encodeURIComponent(shot.shot_id)}`);
  }
  render();
  window.setTimeout(() => (mode === "create" && !name ? ui.composeName : ui.composeIntent).focus(), 0);
}

function openApp(shotId, updateLocation = true) {
  const shot = state.shots.find((candidate) => candidate.shot_id === shotId);
  state.selectedShotId = shotId;
  if (updateLocation) history.replaceState(null, "", `/shots/${encodeURIComponent(shotId)}`);
  // An installed app opens straight into "what should change?" — that is the
  // whole reason to open an app. Anything else shows its one current state.
  if (shot && shot.kind === "factory_shot" && shot.presentation.state === "installed") {
    openCompose({ mode: "evolve", shot, updateLocation: false });
    return;
  }
  state.view = "state";
  render();
}

function commandId(kind) {
  if (!state.commandId) {
    state.commandId = `studio_${kind}_${crypto.randomUUID().replaceAll("-", "")}`;
  }
  return state.commandId;
}

async function submitCompose(event) {
  event.preventDefault();
  if (ui.composeForm.dataset.busy === "true") return;
  const intent = ui.composeIntent.value;
  if (!intent.trim()) {
    ui.composeStatus.textContent = "Describe the app before continuing.";
    ui.composeIntent.focus();
    return;
  }
  ui.composeForm.dataset.busy = "true";
  ui.composeSubmit.disabled = true;
  try {
    if (state.composeMode === "create") await submitCreate(intent);
    else await submitEvolve(intent);
  } catch (error) {
    ui.composeStatus.textContent = error.message;
    showToast(error.message, true);
  } finally {
    ui.composeForm.dataset.busy = "false";
    ui.composeSubmit.disabled = false;
  }
}

async function submitCreate(intent) {
  const name = normalizeAppName(ui.composeName.value);
  if (!name) {
    ui.composeStatus.textContent = "Use lowercase letters, numbers, and hyphens for the app name.";
    ui.composeName.focus();
    throw new Error("The app name is not usable.");
  }
  ui.composeStatus.textContent = "Sending…";
  const receipt = await api("/api/v1/shots", {
    method: "POST",
    body: JSON.stringify({
      command_id: state.pendingIntentionId
        ? `studio_pending_${state.pendingIntentionId}`
        : commandId("create"),
      name,
      intention: intent,
      pending_intention_id: state.pendingIntentionId,
      references: await references.descriptors(),
    }),
  });
  state.drafts.set(String(receipt.shot_id), { mode: "create", name, intent });
  state.pendingIntentionId = null;
  ui.composeIntent.readOnly = false;
  references.setLocked(false);
  state.commandId = null;
  ui.composeStatus.textContent = "";
  state.selectedShotId = String(receipt.shot_id);
  state.view = "state";
  history.replaceState(null, "", `/shots/${encodeURIComponent(receipt.shot_id)}`);
  render();
  await refreshWorkspace();
}

async function submitEvolve(intent) {
  const shot = selectedShot();
  if (!shot) throw new Error("That app is no longer in this workspace.");
  if (!shot.expression_id || !shot.latest_version_id || !shot.latest_version_ordinal) {
    throw new Error("This app has no accepted version to evolve from yet.");
  }
  ui.composeStatus.textContent = "Sending…";
  try {
    await api(`/api/v1/shots/${encodeURIComponent(shot.shot_id)}/evolutions`, {
      method: "POST",
      body: JSON.stringify({
        command_id: commandId("evolve"),
        // The exact accepted base is bound here, at submission, so nobody has
        // to reason about versions. A base that moved is refused, never
        // silently rebased.
        base_expression_id: shot.expression_id,
        base_version_id: shot.latest_version_id,
        base_version_ordinal: shot.latest_version_ordinal,
        intention: intent,
        selected_feedback_actions: [],
        references: await references.descriptors(),
      }),
    });
  } catch (error) {
    if (error instanceof ApiError && error.code === "stale_base") {
      ui.composeStatus.textContent = "This app changed while this request was waiting. Review your request and try again.";
      state.commandId = null;
      await refreshWorkspace();
      throw new Error("This app changed while this request was waiting.");
    }
    throw error;
  }
  state.drafts.set(shot.shot_id, { mode: "evolve", intent });
  state.commandId = null;
  ui.composeStatus.textContent = "";
  state.view = "state";
  render();
  await refreshWorkspace();
}

function retry() {
  const shot = selectedShot();
  const draft = state.drafts.get(state.selectedShotId) || {};
  if (shot && shot.kind === "factory_shot" && shot.latest_version_id) {
    openCompose({ mode: "evolve", shot, intent: draft.intent || "" });
    return;
  }
  openCompose({ mode: "create", name: draft.name || shot?.display_name || "", intent: draft.intent || "" });
}

function normalizeAppName(value) {
  const normalized = value.trim().toLowerCase();
  return /^[a-z0-9][a-z0-9-]{0,62}$/.test(normalized) ? normalized : null;
}

async function loadPendingIntention(pendingId, prefilledName) {
  if (!/^[a-f0-9]{32}$/.test(pendingId)) throw new Error("That prefilled intent reference is malformed.");
  const pending = await api(`/api/v1/pending-intentions/${encodeURIComponent(pendingId)}`);
  if (
    pending.schema !== "tohseno.local-pending-intention-view/1" ||
    pending.pending_intention_id !== pendingId ||
    typeof pending.intention !== "string" ||
    !Array.isArray(pending.references)
  ) {
    throw new Error("Studio received an invalid prefilled intent.");
  }
  openCompose({
    mode: "create",
    name: prefilledName || String(pending.suggested_name || ""),
    intent: pending.intention,
    updateLocation: false,
  });
  state.pendingIntentionId = pendingId;
  // The bytes were imported exactly; Create App submits them unchanged.
  ui.composeIntent.readOnly = true;
  for (const reference of pending.references) {
    if (
      typeof reference.filename !== "string" ||
      typeof reference.media_type !== "string" ||
      typeof reference.origin !== "string" ||
      typeof reference.bytes_base64url !== "string"
    ) {
      throw new Error("Studio received an invalid prefilled image.");
    }
    const file = new File([base64UrlToBytes(reference.bytes_base64url)], reference.filename, {
      type: reference.media_type,
    });
    references.add([file], () => reference.origin, true);
  }
  references.setLocked(true);
}

/* ---------------------------------------------------------------- settings */

async function revokeDevice(device) {
  if (!window.confirm(`Revoke ${device.display_name}? It immediately loses access to this Mac.`)) return;
  try {
    await api(`/api/v1/companion/devices/${encodeURIComponent(device.device_id)}`, { method: "DELETE" });
    await refreshDevices();
    showToast(`${device.display_name} was revoked.`);
  } catch (error) {
    showToast(error.message, true);
  }
}

/* ------------------------------------------------------------------ pieces */

function abbreviate(value, visible = 8) {
  const text = String(value || "");
  if (text.length <= visible * 2 + 1) return text;
  return `${text.slice(0, visible)}…${text.slice(-visible)}`;
}

function formatTimestamp(value) {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "Unknown";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(date);
}

function formatDuration(value) {
  const seconds = Math.max(0, Number(value) || 0);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function showToast(message, error = false) {
  window.clearTimeout(state.toastTimer);
  ui.toast.textContent = message;
  ui.toast.classList.toggle("error", error);
  ui.toast.hidden = false;
  state.toastTimer = window.setTimeout(() => {
    ui.toast.hidden = true;
  }, error ? 7_000 : 3_500);
}

function openApps(updateLocation = true) {
  state.view = "apps";
  state.selectedShotId = null;
  if (updateLocation) history.replaceState(null, "", "/");
  render();
}

function bind() {
  ui.home.addEventListener("click", (event) => {
    event.preventDefault();
    openApps();
  });
  ui.newApp.addEventListener("click", () => openCompose({ mode: "create" }));
  ui.composeName.addEventListener("input", renderPreview);
  ui.openSettings.addEventListener("click", () => {
    state.view = "settings";
    history.replaceState(null, "", "/settings");
    render();
    refreshDevices().catch((error) => showToast(error.message, true));
  });
  ui.composeForm.addEventListener("submit", submitCompose);
  ui.composeForm.addEventListener("input", () => {
    state.commandId = null;
  });
  ui.composeName.addEventListener("change", () => {
    const normalized = normalizeAppName(ui.composeName.value);
    if (normalized) ui.composeName.value = normalized;
  });
  ui.stateRetry.addEventListener("click", retry);
  ui.stateEvolve.addEventListener("click", () => {
    const shot = selectedShot();
    if (shot) openCompose({ mode: "evolve", shot });
  });
  ui.gatePrimary.addEventListener("click", async () => {
    if (!state.gateAction) return;
    if (state.gateAction === "create_app") {
      await refreshProductBoundary();
      openCompose({ mode: "create" });
      return;
    }
    try {
      state.genesis = await api(`/api/v1/genesis/actions/${encodeURIComponent(state.gateAction)}`, {
        method: "POST",
      });
      render();
    } catch (error) {
      showToast(error.message, true);
    }
  });
  ui.gateBack.addEventListener("click", () => history.back());
  for (const [button, plan] of [[ui.proMonthly, "monthly"], [ui.proYearly, "yearly"]]) {
    button.addEventListener("click", async () => {
      try {
        const continuation = await api("/api/v1/billing/checkout", {
          method: "POST",
          body: JSON.stringify({ plan }),
        });
        const checkout = new URL(continuation.checkout_url);
        if (checkout.protocol !== "https:" || checkout.hostname !== "checkout.stripe.com") {
          throw new Error("TOHSENO refused an untrusted checkout continuation.");
        }
        window.location.assign(checkout.href);
      } catch (error) {
        showToast(error.message, true);
      }
    });
  }
  ui.proNotNow.addEventListener("click", () => window.close());
  window.addEventListener("focus", () => {
    if (!["trial_qualified", "pro_lapsed"].includes(state.entitlement?.phase)) return;
    api("/api/v1/billing/refresh", { method: "POST" })
      .then(refreshProductBoundary)
      .catch(() => {});
  });
  window.addEventListener("popstate", () => applyRoute().catch((error) => showToast(error.message, true)));
}

async function applyRoute() {
  const route = new URL(window.location.href);
  if (route.pathname === "/settings") {
    state.view = "settings";
    render();
    await refreshDevices();
    return;
  }
  if (route.pathname === "/create") {
    const requested = (route.searchParams.get("name") || "").toLowerCase();
    const name = normalizeAppName(requested) || "";
    const pending = route.searchParams.get("pending");
    if (pending !== null) await loadPendingIntention(pending, name);
    else openCompose({ mode: "create", name, updateLocation: false });
    if (requested && !name) {
      ui.composeStatus.textContent = "Choose a lowercase name made of letters, numbers, and hyphens.";
    }
    return;
  }
  if (route.pathname.startsWith("/shots/")) {
    let requested = "";
    try {
      requested = decodeURIComponent(route.pathname.slice("/shots/".length));
    } catch {
      requested = "";
    }
    if (state.shots.some((shot) => shot.shot_id === requested)) {
      openApp(requested, false);
      return;
    }
    showToast("That app is not in this workspace.", true);
  }
  openApps(false);
}

async function bootstrap() {
  bind();
  try {
    await refreshStudioSession();
    const [health, factory] = await Promise.all([
      api("/api/v1/health"),
      api("/api/v1/factory-defaults"),
      refreshWorkspace(),
      refreshProductBoundary(),
    ]);
    if (
      health.schema !== "tohseno.local-workspace-health/1" ||
      health.origin !== window.location.origin ||
      health.instance_id !== state.sessionInstanceId
    ) {
      throw new Error("Studio could not verify your TOHSENO.");
    }
    state.health = health;
    state.factory = factory;
    if (state.view !== "gate") await applyRoute();
    connectEvents();
  } catch (error) {
    ui.shell.setAttribute("data-offline", "true");
    ui.appsLoading.classList.add("failed");
    ui.appsLoadingMessage.textContent = "Your apps could not be loaded.";
    showToast(error.message, true);
  }
}

bootstrap();
