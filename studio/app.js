"use strict";

const ui = {
  shell: document.querySelector("#app-shell"),
  serviceStatus: document.querySelector("#service-status"),
  connectIphone: document.querySelector("#connect-iphone"),
  shotCount: document.querySelector("#shot-count"),
  newShot: document.querySelector("#new-shot"),
  shotList: document.querySelector("#shot-list"),
  emptyShots: document.querySelector("#empty-shots"),
  workspaceId: document.querySelector("#workspace-id"),
  snapshotTime: document.querySelector("#snapshot-time"),
  createSurface: document.querySelector("#create-surface"),
  createForm: document.querySelector("#create-form"),
  createName: document.querySelector("#create-name"),
  createIntention: document.querySelector("#create-intention"),
  createImages: document.querySelector("#create-images"),
  createReferenceDrop: document.querySelector("#create-reference-drop"),
  createReferenceList: document.querySelector("#create-reference-list"),
  createReferenceCount: document.querySelector("#create-reference-count"),
  createReferenceStatus: document.querySelector("#create-reference-status"),
  createStatus: document.querySelector("#create-status"),
  takeShot: document.querySelector("#take-shot"),
  factoryHarness: document.querySelector("#factory-harness"),
  factoryRoute: document.querySelector("#factory-route"),
  shotSurface: document.querySelector("#shot-surface"),
  selectedShotMark: document.querySelector("#selected-shot-mark"),
  selectedShotKind: document.querySelector("#selected-shot-kind"),
  selectedShotName: document.querySelector("#selected-shot-name"),
  selectedShotVersion: document.querySelector("#selected-shot-version"),
  returnCreate: document.querySelector("#return-create"),
  activityState: document.querySelector("#activity-state"),
  activityList: document.querySelector("#activity-list"),
  factoryActions: document.querySelector("#factory-actions"),
  recordingOnlyNote: document.querySelector("#recording-only-note"),
  feedbackForm: document.querySelector("#feedback-form"),
  feedbackBinding: document.querySelector("#feedback-binding"),
  feedbackBody: document.querySelector("#feedback-body"),
  feedbackCommitmentList: document.querySelector("#feedback-commitment-list"),
  feedbackRebind: document.querySelector("#feedback-rebind"),
  feedbackStatus: document.querySelector("#feedback-status"),
  submitFeedback: document.querySelector("#submit-feedback"),
  marketingForm: document.querySelector("#marketing-form"),
  marketingBody: document.querySelector("#marketing-body"),
  marketingStatus: document.querySelector("#marketing-status"),
  submitMarketing: document.querySelector("#submit-marketing"),
  evolveForm: document.querySelector("#evolve-form"),
  evolveBinding: document.querySelector("#evolve-binding"),
  evolveIntention: document.querySelector("#evolve-intention"),
  feedbackSelection: document.querySelector("#feedback-selection"),
  feedbackOptions: document.querySelector("#feedback-options"),
  evolveImages: document.querySelector("#evolve-images"),
  evolveReferenceDrop: document.querySelector("#evolve-reference-drop"),
  evolveReferenceList: document.querySelector("#evolve-reference-list"),
  evolveReferenceCount: document.querySelector("#evolve-reference-count"),
  evolveReferenceStatus: document.querySelector("#evolve-reference-status"),
  evolveRebind: document.querySelector("#evolve-rebind"),
  evolveStatus: document.querySelector("#evolve-status"),
  submitEvolution: document.querySelector("#submit-evolution"),
  currentEmpty: document.querySelector("#current-empty"),
  currentShot: document.querySelector("#current-shot"),
  currentShotMark: document.querySelector("#current-shot-mark"),
  currentShotName: document.querySelector("#current-shot-name"),
  currentBundle: document.querySelector("#current-bundle"),
  currentShotId: document.querySelector("#current-shot-id"),
  currentExpressionId: document.querySelector("#current-expression-id"),
  currentVersionId: document.querySelector("#current-version-id"),
  currentAcceptedAt: document.querySelector("#current-accepted-at"),
  currentIconRevision: document.querySelector("#current-icon-revision"),
  currentShotStatus: document.querySelector("#current-shot-status"),
  executionState: document.querySelector("#execution-state"),
  executionDetail: document.querySelector("#execution-detail"),
  executionPipeline: document.querySelector("#execution-pipeline"),
  deviceCount: document.querySelector("#device-count"),
  companionSummary: document.querySelector("#companion-summary"),
  deviceList: document.querySelector("#device-list"),
  pairingDialog: document.querySelector("#pairing-dialog"),
  pairingClose: document.querySelector("#pairing-close"),
  pairingSeal: document.querySelector("#pairing-seal"),
  pairingQr: document.querySelector("#pairing-qr"),
  pairingSpinner: document.querySelector("#pairing-spinner"),
  pairingState: document.querySelector("#pairing-state"),
  pairingCountdown: document.querySelector("#pairing-countdown"),
  pairingDevice: document.querySelector("#pairing-device"),
  pairingCancel: document.querySelector("#pairing-cancel"),
  pairingNew: document.querySelector("#pairing-new"),
  pairingDone: document.querySelector("#pairing-done"),
  toast: document.querySelector("#toast"),
};

const MAX_REFERENCES = 8;
const MAX_REFERENCE_BYTES = 64 * 1024 * 1024;
const MAX_REFERENCE_TOTAL_BYTES = 160 * 1024 * 1024;
const PIPELINE = [
  "queued",
  "planning",
  "conception",
  "materializing",
  "building",
  "testing",
  "verifying",
  "repairing",
  "waiting_for_device",
  "installing",
  "launching",
  "accepted",
];

const state = {
  csrfToken: null,
  sessionInstanceId: null,
  sessionPromise: null,
  workspace: null,
  health: null,
  factoryDefaults: null,
  shots: [],
  selectedShotId: null,
  mode: "create",
  devices: [],
  companion: null,
  feedbackActions: new Map(),
  executionDetails: new Map(),
  lastExecutionByShot: new Map(),
  pendingSelection: null,
  pendingIntentionId: null,
  eventSource: null,
  refreshTimer: null,
  pairing: null,
  pairingCountdownTimer: null,
  pairingRefreshBusy: false,
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

class ReferencePicker {
  constructor({ input, drop, list, count, status, onChange }) {
    this.input = input;
    this.drop = drop;
    this.list = list;
    this.count = count;
    this.status = status;
    this.onChange = onChange;
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
    for (const eventName of ["dragenter", "dragover"]) {
      drop.addEventListener(eventName, (event) => {
        event.preventDefault();
        drop.classList.add("dragging");
      });
    }
    for (const eventName of ["dragleave", "drop"]) {
      drop.addEventListener(eventName, (event) => {
        event.preventDefault();
        drop.classList.remove("dragging");
      });
    }
    drop.addEventListener("drop", (event) => this.add(event.dataTransfer?.files));
    this.render();
  }

  add(values, originForFile = null, force = false) {
    if (this.locked && !force) return;
    const candidates = Array.from(values || []);
    let message = "";
    for (const file of candidates) {
      if (this.files.length >= MAX_REFERENCES) {
        message = "Only eight reference images can be attached.";
        break;
      }
      const mediaType = referenceMediaType(file);
      if (!mediaType) {
        message = `${file.name || "A file"} is not a supported image.`;
        continue;
      }
      if (!file.name || file.name.includes("/") || file.name.includes("\\")) {
        message = "Reference filenames must not contain a path.";
        continue;
      }
      if (file.size < 1 || file.size > MAX_REFERENCE_BYTES) {
        message = `${file.name} must contain 1 byte to 64 MB.`;
        continue;
      }
      if (this.files.some((entry) => entry.file.name === file.name)) {
        message = `${file.name} is already attached.`;
        continue;
      }
      const nextTotal = this.totalBytes() + file.size;
      if (nextTotal > MAX_REFERENCE_TOTAL_BYTES) {
        message = "Reference images exceed the 160 MB combined limit.";
        break;
      }
      this.files.push({
        file,
        mediaType,
        origin: typeof originForFile === "function" ? originForFile(file) : null,
      });
    }
    this.status.textContent = message;
    this.render();
    this.onChange();
  }

  totalBytes() {
    return this.files.reduce((total, entry) => total + entry.file.size, 0);
  }

  clear(force = false) {
    if (this.locked && !force) return;
    this.files = [];
    this.status.textContent = "";
    this.render();
    this.onChange();
  }

  setLocked(locked) {
    this.locked = Boolean(locked);
    this.input.disabled = this.locked;
    this.drop.setAttribute("aria-disabled", String(this.locked));
    this.render();
  }

  render() {
    this.count.textContent = `${this.files.length} / ${MAX_REFERENCES}`;
    this.list.replaceChildren();
    this.files.forEach((entry, index) => {
      const row = element("div", "reference-item");
      row.append(
        element("strong", null, entry.file.name),
        element("span", null, formatBytes(entry.file.size)),
      );
      const remove = element("button", null, "×");
      remove.type = "button";
      remove.disabled = this.locked;
      remove.setAttribute("aria-label", `Remove ${entry.file.name}`);
      remove.addEventListener("click", () => {
        this.files.splice(index, 1);
        this.status.textContent = "";
        this.render();
        this.onChange();
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

const createReferences = new ReferencePicker({
  input: ui.createImages,
  drop: ui.createReferenceDrop,
  list: ui.createReferenceList,
  count: ui.createReferenceCount,
  status: ui.createReferenceStatus,
  onChange: () => invalidateCommand(ui.createForm),
});

const evolveReferences = new ReferencePicker({
  input: ui.evolveImages,
  drop: ui.evolveReferenceDrop,
  list: ui.evolveReferenceList,
  count: ui.evolveReferenceCount,
  status: ui.evolveReferenceStatus,
  onChange: () => {
    invalidateCommand(ui.evolveForm);
    synchronizeBindings();
  },
});

function element(tag, className, text) {
  const value = document.createElement(tag);
  if (className) value.className = className;
  if (text !== undefined) value.textContent = text;
  return value;
}

function referenceMediaType(file) {
  const declared = String(file.type || "").toLowerCase();
  if (["image/png", "image/jpeg", "image/heic", "image/webp"].includes(declared)) {
    return declared;
  }
  const extension = file.name.split(".").pop()?.toLowerCase();
  return {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    heic: "image/heic",
    webp: "image/webp",
  }[extension] || null;
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.ceil(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function bytesToBase64(bytes, urlSafe = false) {
  const chunks = [];
  const chunkSize = 32 * 1024;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + chunkSize)));
  }
  const encoded = btoa(chunks.join(""));
  return urlSafe
    ? encoded.replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "")
    : encoded;
}

function base64UrlToBytes(value) {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) {
    throw new Error("A pending reference is not canonical base64url.");
  }
  const padded = value.replaceAll("-", "+").replaceAll("_", "/")
    + "=".repeat((4 - (value.length % 4)) % 4);
  const decoded = atob(padded);
  const bytes = Uint8Array.from(decoded, (character) => character.charCodeAt(0));
  if (bytesToBase64(bytes, true) !== value) {
    throw new Error("A pending reference is not canonical base64url.");
  }
  return bytes;
}

function isMutation(method) {
  return ["POST", "PUT", "PATCH", "DELETE"].includes(method);
}

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
      throw new Error("Local Workspace Service returned an invalid Studio session.");
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
  const mutation = isMutation(method);
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

function setConnection(mode, message) {
  ui.shell.dataset.connection = mode;
  ui.serviceStatus.textContent = message;
}

function scheduleRefresh() {
  window.clearTimeout(state.refreshTimer);
  state.refreshTimer = window.setTimeout(async () => {
    try {
      await Promise.all([refreshWorkspace(), refreshDevices(), refreshTrackedExecutions()]);
      if (state.pairing?.state === "waiting") await refreshPairingSession();
    } catch (error) {
      showToast(error.message, true);
    }
  }, 220);
}

function connectEvents() {
  state.eventSource?.close();
  const source = new EventSource("/api/v1/events");
  state.eventSource = source;
  source.addEventListener("open", () => {
    setConnection("connected", `Your TOHSENO is running · ${state.health?.service_version || "0.9.0"}`);
    // EventSource reconnects automatically, but a notification may have been
    // emitted while the browser was suspended. Reconcile once on every open;
    // subsequent updates stay event-driven rather than polling the workspace.
    scheduleRefresh();
  });
  source.addEventListener("workspace.changed", scheduleRefresh);
  source.addEventListener("workspace.reconcile", scheduleRefresh);
  source.addEventListener("error", () => {
    setConnection("offline", "Reconnecting · factory work continues on this Mac");
  });
}

async function refreshWorkspace() {
  const snapshot = await api("/api/v1/workspace");
  if (snapshot.schema !== "tohseno.companion-workspace-snapshot/1" || !Array.isArray(snapshot.shots)) {
    throw new Error("Workspace snapshot schema is not supported by this Studio.");
  }
  state.workspace = snapshot;
  state.shots = [...snapshot.shots].sort((left, right) => {
    if (left.sort_index !== right.sort_index) return left.sort_index - right.sort_index;
    return left.display_name.localeCompare(right.display_name);
  });
  for (const execution of snapshot.active_executions || []) {
    state.executionDetails.set(execution.execution_id, execution);
    state.lastExecutionByShot.set(execution.shot_id, execution.execution_id);
  }
  if (state.pendingSelection && state.shots.some((shot) => shot.shot_id === state.pendingSelection)) {
    state.selectedShotId = state.pendingSelection;
    state.pendingSelection = null;
    state.mode = "shot";
  }
  if (state.selectedShotId && !state.shots.some((shot) => shot.shot_id === state.selectedShotId)) {
    state.selectedShotId = null;
    if (state.mode === "shot") state.mode = "create";
  }
  if (state.selectedShotId) await refreshFeedbackActions(state.selectedShotId);
  renderWorkspace();
}

async function refreshFeedbackActions(shotId) {
  const shot = state.shots.find((candidate) => candidate.shot_id === shotId);
  if (!shot || shot.kind !== "factory_shot") {
    state.feedbackActions.delete(shotId);
    return;
  }
  const response = await api(`/api/v1/shots/${encodeURIComponent(shotId)}/feedback`);
  if (response.schema !== "tohseno.local-feedback-action-list/1" || !Array.isArray(response.actions)) {
    throw new Error("Studio received an invalid Feedback action list.");
  }
  state.feedbackActions.set(shotId, response.actions.map((action) => ({
    commitment: String(action.action_commitment),
    versionId: String(action.version_id),
    versionOrdinal: Number(action.version_ordinal),
  })));
}

async function refreshDevices() {
  const [devices, companion] = await Promise.all([
    api("/api/v1/companion/devices"),
    api("/api/v1/companion/status"),
  ]);
  state.devices = Array.isArray(devices.devices) ? devices.devices : [];
  state.companion = companion;
  renderDevices();
}

async function refreshFactoryDefaults() {
  const defaults = await api("/api/v1/factory-defaults");
  if (defaults.schema !== "tohseno.local-factory-defaults/1") {
    throw new Error("Studio received invalid local factory defaults.");
  }
  state.factoryDefaults = defaults;
  ui.factoryHarness.textContent = defaults.harness_label || "No local coding harness available";
  const model = defaults.model_label || defaults.model_id;
  const route = defaults.route_label || defaults.route_id;
  ui.factoryRoute.textContent = [model, route].filter(Boolean).join(" · ") || "No authenticated local route available";
}

async function refreshTrackedExecutions() {
  const identifiers = [...new Set(state.lastExecutionByShot.values())].slice(-24);
  await Promise.all(identifiers.map(async (executionId) => {
    try {
      const detail = await api(`/api/v1/executions/${encodeURIComponent(executionId)}`);
      state.executionDetails.set(executionId, detail);
    } catch (error) {
      if (!(error instanceof ApiError && error.status === 404)) throw error;
    }
  }));
  if (state.mode === "shot") renderSelectedShot();
}

function renderWorkspace() {
  ui.workspaceId.textContent = state.workspace
    ? `Workspace ${abbreviate(state.workspace.workspace_id, 12)}`
    : "Workspace unavailable";
  ui.snapshotTime.textContent = state.workspace
    ? `Updated ${relativeTime(state.workspace.generated_at)}`
    : "Waiting for workspace";
  renderShotList();
  if (state.mode === "shot" && selectedShot()) renderSelectedShot();
  else renderCreateSurface();
}

function renderShotList() {
  ui.shotCount.textContent = String(state.shots.length);
  ui.shotList.replaceChildren();
  ui.emptyShots.hidden = state.shots.length > 0;
  for (const shot of state.shots) {
    const button = element("button", "shot-row");
    button.type = "button";
    button.classList.toggle("selected", state.mode === "shot" && shot.shot_id === state.selectedShotId);
    button.setAttribute("aria-label", `Open ${shot.display_name}`);
    const mark = element("span", "shot-mark", initial(shot.display_name));
    mark.setAttribute("aria-hidden", "true");
    const copy = element("span", "shot-copy");
    copy.append(
      element("strong", null, shot.display_name),
      element("span", null, shotListDetail(shot)),
    );
    const dot = element("span", "shot-kind-dot");
    if (shot.kind === "recording_only") dot.classList.add("recording");
    if (shot.execution) dot.classList.add("running");
    dot.setAttribute("aria-hidden", "true");
    button.append(mark, copy, dot);
    button.addEventListener("click", () => selectShot(shot.shot_id));
    ui.shotList.append(button);
  }
}

function shotListDetail(shot) {
  if (shot.retired) return "Retired · local history preserved";
  if (shot.archived) return "Archived · local history preserved";
  if (shot.execution) return executionLabel(shot.execution.state);
  if (shot.kind === "recording_only") return "Recording layer · factory actions unavailable";
  if (shot.latest_version_ordinal) return `Accepted Version ${shot.latest_version_ordinal}`;
  return "Factory Shot · awaiting accepted Version";
}

function renderCreateSurface() {
  state.mode = "create";
  ui.createSurface.hidden = false;
  ui.shotSurface.hidden = true;
  ui.currentEmpty.hidden = false;
  ui.currentShot.hidden = true;
  ui.newShot.classList.add("active");
  renderShotList();
}

function selectShot(shotId, updateLocation = true) {
  const changed = state.selectedShotId !== shotId;
  state.selectedShotId = shotId;
  state.mode = "shot";
  if (changed) resetActionDrafts();
  if (updateLocation) history.replaceState(null, "", `/shots/${encodeURIComponent(shotId)}`);
  renderWorkspace();
  refreshFeedbackActions(shotId)
    .then(() => {
      if (state.selectedShotId === shotId) renderFeedbackActions(selectedShot());
    })
    .catch((error) => showToast(error.message, true));
}

function selectedShot() {
  return state.shots.find((shot) => shot.shot_id === state.selectedShotId) || null;
}

function renderSelectedShot() {
  const shot = selectedShot();
  if (!shot) {
    renderCreateSurface();
    return;
  }
  ui.createSurface.hidden = true;
  ui.shotSurface.hidden = false;
  ui.currentEmpty.hidden = true;
  ui.currentShot.hidden = false;
  ui.newShot.classList.remove("active");
  ui.selectedShotMark.textContent = initial(shot.display_name);
  ui.currentShotMark.textContent = initial(shot.display_name);
  ui.selectedShotName.textContent = shot.display_name;
  ui.currentShotName.textContent = shot.display_name;
  const lifecycle = shot.retired ? " · RETIRED" : shot.archived ? " · ARCHIVED" : "";
  ui.selectedShotKind.textContent = `${shot.kind === "recording_only" ? "RECORDING-ONLY FOLDER" : "FACTORY SHOT"}${lifecycle}`;
  ui.selectedShotVersion.textContent = versionDescription(shot);
  ui.currentBundle.textContent = shot.bundle_identifier || "No bundle identifier";
  ui.currentShotId.textContent = abbreviate(shot.shot_id);
  ui.currentShotId.title = shot.shot_id;
  ui.currentExpressionId.textContent = shot.expression_id ? abbreviate(shot.expression_id) : "—";
  ui.currentExpressionId.title = shot.expression_id || "";
  ui.currentVersionId.textContent = shot.latest_version_id
    ? `#${shot.latest_version_ordinal} · ${abbreviate(shot.latest_version_id)}`
    : "—";
  ui.currentVersionId.title = shot.latest_version_id || "";
  ui.currentAcceptedAt.textContent = shot.latest_version_created_at
    ? formatTimestamp(shot.latest_version_created_at)
    : "—";
  ui.currentIconRevision.textContent = shot.icon?.placeholder
    ? `Placeholder · ${abbreviate(shot.icon.revision)}`
    : abbreviate(shot.icon?.revision || "—");
  ui.currentIconRevision.title = shot.icon?.revision || "";
  ui.currentShotStatus.textContent = shot.retired ? "Retired" : shot.archived ? "Archived" : "Active";

  const factory = shot.kind === "factory_shot";
  ui.factoryActions.hidden = !factory;
  ui.recordingOnlyNote.hidden = factory;
  renderActivity(shot);
  renderExecution(shot);
  synchronizeBindings();
  renderFeedbackActions(shot);
  renderShotList();
}

function versionDescription(shot) {
  if (shot.kind === "recording_only") return "Explicit recording-layer compatibility · no factory lineage";
  if (!shot.latest_version_id) return "No accepted Version yet";
  return `Expression ${abbreviate(shot.expression_id)} · accepted Version ${shot.latest_version_ordinal} · ${abbreviate(shot.latest_version_id)}`;
}

function renderActivity(shot) {
  const executions = (state.workspace?.active_executions || [])
    .filter((execution) => execution.shot_id === shot.shot_id)
    .sort((left, right) => String(right.updated_at).localeCompare(String(left.updated_at)));
  const trackedId = state.lastExecutionByShot.get(shot.shot_id);
  const tracked = trackedId ? state.executionDetails.get(trackedId) : null;
  if (tracked && !executions.some((execution) => execution.execution_id === tracked.execution_id)) {
    executions.unshift(tracked);
  }
  ui.activityList.replaceChildren();
  const current = executions[0] || shot.execution || null;
  setStatePill(ui.activityState, current?.state || (shot.latest_version_id ? "accepted" : "idle"));
  if (executions.length === 0) {
    const message = shot.latest_version_id
      ? `Version ${shot.latest_version_ordinal} is the latest accepted local app.`
      : "No execution has been admitted for this Shot yet.";
    ui.activityList.append(element("p", "activity-empty", message));
    return;
  }
  for (const execution of executions.slice(0, 8)) {
    const row = element("div", "activity-row");
    row.append(
      element("span", "activity-dot"),
      element("strong", null, executionLabel(execution.state)),
      element("span", null, relativeTime(execution.updated_at)),
    );
    ui.activityList.append(row);
  }
}

function executionForShot(shot) {
  const executionId = state.lastExecutionByShot.get(shot.shot_id);
  return (executionId && state.executionDetails.get(executionId)) || shot.execution || null;
}

function renderExecution(shot) {
  const execution = executionForShot(shot);
  const executionState = execution?.state || (shot.latest_version_id ? "accepted" : "idle");
  setStatePill(ui.executionState, executionState);
  if (execution) {
    ui.executionDetail.textContent = `${executionLabel(execution.state)} · execution ${abbreviate(execution.execution_id)} · updated ${relativeTime(execution.updated_at)}`;
    ui.executionDetail.title = execution.execution_id;
  } else if (shot.latest_version_id) {
    ui.executionDetail.textContent = `Accepted Version ${shot.latest_version_ordinal} is current. No execution is active.`;
    ui.executionDetail.removeAttribute("title");
  } else {
    ui.executionDetail.textContent = "No active execution. Completion is never inferred from source files alone.";
    ui.executionDetail.removeAttribute("title");
  }
  ui.executionPipeline.replaceChildren();
  const activeIndex = PIPELINE.indexOf(executionState);
  for (const [index, phase] of PIPELINE.entries()) {
    const item = element("li", "pipeline-step", executionLabel(phase));
    if (executionState === "failed" && index === Math.max(0, activeIndex)) item.classList.add("failed");
    else if (activeIndex >= 0 && index < activeIndex) item.classList.add("complete");
    else if (activeIndex === index) item.classList.add("current");
    ui.executionPipeline.append(item);
  }
  if (executionState === "failed" || executionState === "cancelled") {
    const failure = element("li", "pipeline-step failed", executionLabel(executionState));
    ui.executionPipeline.append(failure);
  }
}

function setStatePill(node, phase) {
  node.dataset.state = phase;
  node.textContent = executionLabel(phase);
}

function executionLabel(value) {
  return {
    idle: "Idle",
    running: "Running",
    queued: "Queued",
    planning: "Planning",
    conception: "Conception",
    materializing: "Materializing",
    building: "Building",
    testing: "Testing",
    verifying: "Verifying",
    repairing: "Repairing",
    waiting_for_device: "Waiting for iPhone",
    installing: "Installing",
    launching: "Launching",
    accepted: "Accepted",
    completed: "Completed",
    failed: "Failed",
    cancelled: "Cancelled",
  }[value] || "Working";
}

function exactBinding(shot) {
  if (!shot?.expression_id || !shot.latest_version_id || !shot.latest_version_ordinal) return null;
  return {
    shotId: shot.shot_id,
    expressionId: String(shot.expression_id),
    versionId: String(shot.latest_version_id),
    versionOrdinal: Number(shot.latest_version_ordinal),
    key: `${shot.shot_id}:${shot.expression_id}:${shot.latest_version_id}:${shot.latest_version_ordinal}`,
  };
}

function storedBinding(form) {
  if (!form.dataset.bindingKey) return null;
  return {
    shotId: form.dataset.shotId,
    expressionId: form.dataset.expressionId,
    versionId: form.dataset.versionId,
    versionOrdinal: Number(form.dataset.versionOrdinal),
    key: form.dataset.bindingKey,
  };
}

function setBinding(form, binding) {
  for (const key of ["bindingKey", "shotId", "expressionId", "versionId", "versionOrdinal"]) {
    delete form.dataset[key];
  }
  if (!binding) return;
  form.dataset.bindingKey = binding.key;
  form.dataset.shotId = binding.shotId;
  form.dataset.expressionId = binding.expressionId;
  form.dataset.versionId = binding.versionId;
  form.dataset.versionOrdinal = String(binding.versionOrdinal);
  invalidateCommand(form);
}

function synchronizeBindings(force = false) {
  const shot = selectedShot();
  const current = exactBinding(shot);
  synchronizeFormBinding({
    form: ui.feedbackForm,
    node: ui.feedbackBinding,
    rebind: ui.feedbackRebind,
    submit: ui.submitFeedback,
    hasDraft: () => ui.feedbackBody.value.trim().length > 0,
    current,
    force,
  });
  synchronizeFormBinding({
    form: ui.evolveForm,
    node: ui.evolveBinding,
    rebind: ui.evolveRebind,
    submit: ui.submitEvolution,
    hasDraft: () => ui.evolveIntention.value.trim().length > 0 || evolveReferences.files.length > 0 || selectedFeedbackActions().length > 0,
    current,
    force,
  });
}

function synchronizeFormBinding({ form, node, rebind, submit, hasDraft, current, force }) {
  const stored = storedBinding(form);
  const stale = Boolean(current && stored && stored.key !== current.key && hasDraft() && !force);
  if (force || !stored || (!stale && current && stored.key !== current.key)) setBinding(form, current);
  const active = storedBinding(form);
  form.classList.toggle("stale", stale);
  node.classList.toggle("stale", stale);
  rebind.hidden = !stale;
  if (!current) {
    node.textContent = "NO ACCEPTED BASE";
    submit.disabled = true;
  } else if (stale) {
    node.textContent = "BASE CHANGED";
    submit.disabled = true;
  } else {
    node.textContent = `VERSION ${active?.versionOrdinal || current.versionOrdinal}`;
    submit.disabled = false;
  }
}

function resetActionDrafts() {
  ui.feedbackBody.value = "";
  ui.marketingBody.value = "";
  ui.evolveIntention.value = "";
  ui.feedbackStatus.textContent = "";
  ui.marketingStatus.textContent = "";
  ui.evolveStatus.textContent = "";
  evolveReferences.clear();
  setBinding(ui.feedbackForm, null);
  setBinding(ui.evolveForm, null);
  invalidateCommand(ui.feedbackForm);
  invalidateCommand(ui.marketingForm);
  invalidateCommand(ui.evolveForm);
}

function renderFeedbackActions(shot) {
  const actions = state.feedbackActions.get(shot.shot_id) || [];
  const selected = new Set(selectedFeedbackActions());
  ui.feedbackCommitmentList.replaceChildren();
  for (const action of actions.slice(-3).reverse()) {
    ui.feedbackCommitmentList.append(element(
      "div",
      "commitment-record",
      `Saved for Version ${action.versionOrdinal} · action ${abbreviate(action.commitment)}`,
    ));
  }
  const binding = storedBinding(ui.evolveForm);
  const eligible = actions.filter((action) => binding && action.versionId === binding.versionId);
  ui.feedbackOptions.replaceChildren();
  ui.feedbackSelection.hidden = eligible.length === 0;
  for (const action of eligible) {
    const label = element("label", "feedback-option");
    const input = document.createElement("input");
    input.type = "checkbox";
    input.value = action.commitment;
    input.checked = selected.has(action.commitment);
    input.addEventListener("change", () => {
      invalidateCommand(ui.evolveForm);
      synchronizeBindings();
    });
    label.append(input, document.createTextNode(`Version ${action.versionOrdinal} · ${abbreviate(action.commitment)}`));
    ui.feedbackOptions.append(label);
  }
}

function selectedFeedbackActions() {
  return [...ui.feedbackOptions.querySelectorAll('input[type="checkbox"]:checked')]
    .map((input) => input.value);
}

function invalidateCommand(form) {
  delete form.dataset.commandId;
}

function stableCommandId(form, kind) {
  if (!form.dataset.commandId) {
    form.dataset.commandId = `studio_${kind}_${crypto.randomUUID().replaceAll("-", "")}`;
  }
  return form.dataset.commandId;
}

async function submitWithBusy(form, button, task) {
  if (form.dataset.busy === "true") return;
  form.dataset.busy = "true";
  button.disabled = true;
  try {
    await task();
  } finally {
    form.dataset.busy = "false";
    synchronizeBindings();
    if (!button.closest("form")?.classList.contains("stale")) button.disabled = false;
  }
}

async function submitCreate(event) {
  event.preventDefault();
  if (!ui.createForm.reportValidity()) return;
  const name = normalizeShotName(ui.createName.value);
  if (!name) {
    ui.createStatus.textContent = "Use lowercase letters, numbers, and hyphens for the Shot name.";
    ui.createName.focus();
    return;
  }
  const exactIntention = ui.createIntention.value;
  if (!exactIntention.trim()) {
    ui.createStatus.textContent = "The exact intention must contain something beyond whitespace.";
    ui.createIntention.focus();
    return;
  }
  await submitWithBusy(ui.createForm, ui.takeShot, async () => {
    try {
      ui.createStatus.textContent = createReferences.files.length
        ? "Preserving exact intention and reference bytes…"
        : "Preserving exact intention…";
      const references = await createReferences.descriptors();
      const receipt = await api("/api/v1/shots", {
        method: "POST",
        body: JSON.stringify({
          command_id: state.pendingIntentionId
            ? `studio_pending_${state.pendingIntentionId}`
            : stableCommandId(ui.createForm, "create"),
          name,
          intention: exactIntention,
          pending_intention_id: state.pendingIntentionId,
          references,
        }),
      });
      rememberExecution(receipt);
      state.pendingSelection = String(receipt.shot_id);
      ui.createStatus.textContent = `Shot admitted · execution ${abbreviate(receipt.execution_id)}`;
      invalidateCommand(ui.createForm);
      state.pendingIntentionId = null;
      ui.createIntention.readOnly = false;
      createReferences.setLocked(false);
      ui.createName.value = "";
      ui.createIntention.value = "";
      createReferences.clear();
      await Promise.all([refreshWorkspace(), refreshTrackedExecutions()]);
      if (state.shots.some((shot) => shot.shot_id === String(receipt.shot_id))) {
        selectShot(String(receipt.shot_id));
      }
      showToast(`${name} entered the local factory.`);
    } catch (error) {
      ui.createStatus.textContent = error.message;
      throw error;
    }
  }).catch((error) => showToast(error.message, true));
}

async function submitFeedback(event) {
  event.preventDefault();
  if (!ui.feedbackForm.reportValidity()) return;
  const binding = storedBinding(ui.feedbackForm);
  if (!binding || binding.shotId !== state.selectedShotId) return;
  await submitWithBusy(ui.feedbackForm, ui.submitFeedback, async () => {
    try {
      ui.feedbackStatus.textContent = `Binding to accepted Version ${binding.versionOrdinal}…`;
      const receipt = await api(`/api/v1/shots/${encodeURIComponent(binding.shotId)}/feedback`, {
        method: "POST",
        body: JSON.stringify({
          command_id: stableCommandId(ui.feedbackForm, "feedback"),
          expression_id: binding.expressionId,
          version_id: binding.versionId,
          version_ordinal: binding.versionOrdinal,
          body: ui.feedbackBody.value,
        }),
      });
      const actions = state.feedbackActions.get(binding.shotId) || [];
      actions.push({
        commitment: String(receipt.action_commitment),
        versionId: String(receipt.version_id),
        versionOrdinal: Number(receipt.version_ordinal),
      });
      state.feedbackActions.set(binding.shotId, actions);
      ui.feedbackBody.value = "";
      invalidateCommand(ui.feedbackForm);
      ui.feedbackStatus.textContent = `Saved · action ${abbreviate(receipt.action_commitment)}`;
      renderFeedbackActions(selectedShot());
      showToast(`Feedback attached to Version ${receipt.version_ordinal}.`);
    } catch (error) {
      ui.feedbackStatus.textContent = error.message;
      throw error;
    }
  }).catch((error) => showToast(error.message, true));
}

async function submitMarketing(event) {
  event.preventDefault();
  if (!ui.marketingForm.reportValidity()) return;
  const shot = selectedShot();
  if (!shot || shot.kind !== "factory_shot") return;
  await submitWithBusy(ui.marketingForm, ui.submitMarketing, async () => {
    try {
      ui.marketingStatus.textContent = "Saving privately beside this Shot…";
      const receipt = await api(`/api/v1/shots/${encodeURIComponent(shot.shot_id)}/marketing`, {
        method: "POST",
        body: JSON.stringify({
          command_id: stableCommandId(ui.marketingForm, "marketing"),
          body: ui.marketingBody.value,
        }),
      });
      ui.marketingBody.value = "";
      invalidateCommand(ui.marketingForm);
      ui.marketingStatus.textContent = `Saved · note ${abbreviate(receipt.note_id)}`;
      showToast(`Private marketing note saved for ${shot.display_name}.`);
    } catch (error) {
      ui.marketingStatus.textContent = error.message;
      throw error;
    }
  }).catch((error) => showToast(error.message, true));
}

async function submitEvolution(event) {
  event.preventDefault();
  if (!ui.evolveForm.reportValidity()) return;
  const binding = storedBinding(ui.evolveForm);
  if (!binding || binding.shotId !== state.selectedShotId || ui.evolveForm.classList.contains("stale")) return;
  const exactIntention = ui.evolveIntention.value;
  const feedbackActions = selectedFeedbackActions();
  if (!exactIntention.trim() && feedbackActions.length === 0) {
    ui.evolveStatus.textContent = "Add an evolutionary intention or select an exact Feedback action.";
    ui.evolveIntention.focus();
    return;
  }
  await submitWithBusy(ui.evolveForm, ui.submitEvolution, async () => {
    try {
      ui.evolveStatus.textContent = `Admitting an exact-base evolution from Version ${binding.versionOrdinal}…`;
      const receipt = await api(`/api/v1/shots/${encodeURIComponent(binding.shotId)}/evolutions`, {
        method: "POST",
        body: JSON.stringify({
          command_id: stableCommandId(ui.evolveForm, "evolve"),
          base_expression_id: binding.expressionId,
          base_version_id: binding.versionId,
          base_version_ordinal: binding.versionOrdinal,
          intention: exactIntention,
          selected_feedback_actions: feedbackActions,
          references: await evolveReferences.descriptors(),
        }),
      });
      rememberExecution(receipt);
      ui.evolveIntention.value = "";
      evolveReferences.clear();
      invalidateCommand(ui.evolveForm);
      ui.evolveStatus.textContent = `Evolution admitted · execution ${abbreviate(receipt.execution_id)}`;
      await Promise.all([refreshWorkspace(), refreshTrackedExecutions()]);
      showToast(`Evolution began from exact Version ${binding.versionOrdinal}.`);
    } catch (error) {
      ui.evolveStatus.textContent = error.message;
      throw error;
    }
  }).catch((error) => showToast(error.message, true));
}

function rememberExecution(receipt) {
  const summary = {
    execution_id: String(receipt.execution_id),
    shot_id: String(receipt.shot_id),
    state: String(receipt.state || "queued"),
    updated_at: new Date().toISOString(),
  };
  state.executionDetails.set(summary.execution_id, summary);
  state.lastExecutionByShot.set(summary.shot_id, summary.execution_id);
}

function normalizeShotName(value) {
  const normalized = value.trim().toLowerCase();
  return /^[a-z0-9][a-z0-9-]{0,62}$/.test(normalized) ? normalized : null;
}

function clearPendingIntention() {
  if (!state.pendingIntentionId) return;
  state.pendingIntentionId = null;
  ui.createIntention.readOnly = false;
  createReferences.setLocked(false);
  createReferences.clear();
  ui.createIntention.value = "";
  ui.createStatus.textContent = "";
}

function openCreateSurface({ updateLocation = true, prefilledName = null, preservePending = false } = {}) {
  if (!preservePending) clearPendingIntention();
  state.mode = "create";
  if (prefilledName !== null) ui.createName.value = prefilledName;
  if (updateLocation) {
    const query = ui.createName.value ? `?name=${encodeURIComponent(ui.createName.value)}` : "";
    history.replaceState(null, "", `/create${query}`);
  }
  renderCreateSurface();
  window.setTimeout(() => (ui.createName.value ? ui.createIntention : ui.createName).focus(), 0);
}

async function loadPendingIntention(pendingId, prefilledName = null) {
  if (!/^[a-f0-9]{32}$/.test(pendingId)) {
    throw new Error("The local pending intention ID is malformed.");
  }
  const pending = await api(`/api/v1/pending-intentions/${encodeURIComponent(pendingId)}`);
  if (
    pending.schema !== "tohseno.local-pending-intention-view/1"
    || pending.pending_intention_id !== pendingId
    || typeof pending.intention !== "string"
    || !Array.isArray(pending.references)
  ) {
    throw new Error("Studio received an invalid pending intention.");
  }
  state.pendingIntentionId = pendingId;
  ui.createIntention.value = pending.intention;
  ui.createIntention.readOnly = true;
  createReferences.setLocked(false);
  createReferences.clear();
  for (const reference of pending.references) {
    if (
      typeof reference.filename !== "string"
      || typeof reference.media_type !== "string"
      || typeof reference.origin !== "string"
      || typeof reference.bytes_base64url !== "string"
    ) {
      throw new Error("Studio received an invalid pending reference.");
    }
    const bytes = base64UrlToBytes(reference.bytes_base64url);
    const file = new File([bytes], reference.filename, { type: reference.media_type });
    createReferences.add([file], () => reference.origin, true);
  }
  createReferences.setLocked(true);
  const suggested = prefilledName || String(pending.suggested_name || "");
  openCreateSurface({ updateLocation: false, prefilledName: suggested, preservePending: true });
  ui.createStatus.textContent = "Imported exactly and stored safely on this Mac. Choose a name, then TAKE THE SHOT.";
}

async function refreshDevicesAndNotify() {
  await refreshDevices();
  const active = state.devices.filter((device) => !device.revoked);
  if (active.length) showToast(`${active.at(-1).display_name} is paired with this workspace.`);
}

function renderDevices() {
  const active = state.devices.filter((device) => !device.revoked);
  ui.deviceCount.textContent = String(active.length);
  const relayState = String(state.companion?.relay_connection || "configuration_required")
    .replaceAll("_", " ");
  ui.companionSummary.textContent = active.length
    ? `${active.length} active ${active.length === 1 ? "device" : "devices"} · relay ${relayState}`
    : `No iPhone connected · relay ${relayState}`;
  ui.deviceList.replaceChildren();
  for (const device of state.devices) {
    const record = element("article", "device-record");
    record.classList.toggle("revoked", Boolean(device.revoked));
    const head = element("div", "device-record-head");
    const identity = element("div");
    identity.append(
      element("strong", null, device.display_name),
      element("small", null, device.device_id_abbreviation || abbreviate(device.device_id)),
    );
    head.append(identity);
    if (!device.revoked) {
      const revoke = element("button", "revoke-device", "REVOKE");
      revoke.type = "button";
      revoke.addEventListener("click", () => revokeDevice(device));
      head.append(revoke);
    } else {
      head.append(element("small", null, "REVOKED"));
    }
    const dates = element("div", "device-dates");
    const paired = element("span", null, `Connected ${formatTimestamp(device.paired_at)}`);
    paired.title = device.paired_at;
    const seen = element("span", null, `Last synchronized ${relativeTime(device.last_seen)}`);
    seen.title = device.last_seen;
    dates.append(paired, seen);
    const syncState = String(device.sync_state || (device.revoked ? "revoked" : "unknown"))
      .replaceAll("_", " ");
    record.append(
      head,
      element("span", "device-sync-state", `SYNC · ${syncState.toUpperCase()}`),
      element("div", "device-capabilities", capabilitySummary(device.capabilities || [])),
      dates,
    );
    ui.deviceList.append(record);
  }
}

function capabilitySummary(capabilities) {
  const values = new Set(capabilities);
  const actions = [];
  if (values.has("workspace.read") || values.has("execution.read")) actions.push("view");
  if (values.has("shot.create")) actions.push("create");
  if (values.has("shot.evolve")) actions.push("evolve");
  if (values.has("feedback.write")) actions.push("send feedback");
  if (values.has("marketing.write")) actions.push("send marketing notes");
  return actions.length ? `Can ${actions.join(", ")}` : "No active companion actions";
}

async function revokeDevice(device) {
  if (!window.confirm(`Revoke ${device.display_name}? This iPhone will immediately lose workspace capabilities.`)) return;
  try {
    await api(`/api/v1/companion/devices/${encodeURIComponent(device.device_id)}`, {
      method: "DELETE",
    });
    await refreshDevices();
    showToast(`${device.display_name} was revoked.`);
  } catch (error) {
    showToast(error.message, true);
  }
}

async function openPairing() {
  if (!ui.pairingDialog.open) ui.pairingDialog.showModal();
  await startPairingSession();
}

async function startPairingSession() {
  await cancelPairingSession(false);
  resetPairingView();
  try {
    const session = await api("/api/v1/companion/pairing-sessions", {
      method: "POST",
      body: "{}",
    });
    validatePairingSession(session);
    state.pairing = session;
    renderPairingSession();
    startPairingCountdown();
  } catch (error) {
    ui.pairingState.textContent = error.message;
    ui.pairingSpinner.classList.add("stopped");
    ui.pairingNew.hidden = false;
    ui.pairingCancel.hidden = true;
  }
}

async function resumePairingSession(sessionId) {
  resetPairingView();
  if (!ui.pairingDialog.open) ui.pairingDialog.showModal();
  if (!/^[A-Za-z0-9_-]{16,200}$/.test(sessionId)) {
    throw new Error("The requested pairing session is invalid.");
  }
  const session = await api(
    `/api/v1/companion/pairing-sessions/${encodeURIComponent(sessionId)}`,
  );
  validatePairingSession(session);
  state.pairing = session;
  renderPairingSession();
  startPairingCountdown();
}

function startPairingCountdown() {
  if (state.pairing?.state !== "waiting") return;
  state.pairingCountdownTimer = window.setInterval(updatePairingCountdown, 250);
}

function validatePairingSession(session) {
  if (
    session.schema !== "tohseno.studio-pairing-session/1" ||
    typeof session.session_id !== "string" ||
    typeof session.expires_at !== "string" ||
    typeof session.qr_svg !== "string" ||
    session.qr_svg.length > 1024 * 1024
  ) {
    throw new Error("Local Workspace Service returned an invalid pairing seal.");
  }
}

function resetPairingView() {
  stopPairingTimers();
  state.pairing = null;
  ui.pairingSeal.classList.remove("success");
  ui.pairingQr.removeAttribute("src");
  ui.pairingSpinner.classList.remove("stopped");
  ui.pairingState.textContent = "Preparing a one-use seal…";
  ui.pairingCountdown.textContent = "—";
  ui.pairingDevice.textContent = "";
  ui.pairingCancel.hidden = false;
  ui.pairingNew.hidden = true;
  ui.pairingDone.hidden = true;
}

function renderPairingSession() {
  const pairing = state.pairing;
  if (!pairing) return;
  if (!ui.pairingQr.hasAttribute("src")) {
    const svgBytes = new TextEncoder().encode(pairing.qr_svg);
    ui.pairingQr.src = `data:image/svg+xml;base64,${bytesToBase64(svgBytes)}`;
  }
  updatePairingCountdown();
  if (pairing.state === "paired") {
    stopPairingTimers();
    ui.pairingSeal.classList.add("success");
    ui.pairingSpinner.classList.add("stopped");
    ui.pairingState.textContent = "iPhone connected";
    ui.pairingCountdown.textContent = "PAIRED";
    ui.pairingDevice.textContent = pairing.device_name || "TOHSENO Companion";
    ui.pairingCancel.hidden = true;
    ui.pairingNew.hidden = true;
    ui.pairingDone.hidden = false;
    refreshDevicesAndNotify().catch((error) => showToast(error.message, true));
  } else if (pairing.state === "expired" || pairing.state === "cancelled") {
    stopPairingTimers();
    ui.pairingSpinner.classList.add("stopped");
    ui.pairingState.textContent = pairing.state === "expired" ? "Pairing seal expired" : "Pairing cancelled";
    ui.pairingCountdown.textContent = pairing.state.toUpperCase();
    ui.pairingCancel.hidden = true;
    ui.pairingNew.hidden = false;
  } else {
    ui.pairingState.textContent = "Waiting for iPhone…";
  }
}

function updatePairingCountdown() {
  if (!state.pairing || state.pairing.state !== "waiting") return;
  const remaining = Math.max(0, Date.parse(state.pairing.expires_at) - Date.now());
  const seconds = Math.ceil(remaining / 1000);
  ui.pairingCountdown.textContent = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  if (remaining === 0) refreshPairingSession().catch(() => {});
}

async function refreshPairingSession() {
  if (!state.pairing || state.pairingRefreshBusy) return;
  state.pairingRefreshBusy = true;
  try {
    const session = await api(`/api/v1/companion/pairing-sessions/${encodeURIComponent(state.pairing.session_id)}`);
    validatePairingSession(session);
    state.pairing = session;
    renderPairingSession();
  } finally {
    state.pairingRefreshBusy = false;
  }
}

async function cancelPairingSession(closeDialog = true) {
  stopPairingTimers();
  const pairing = state.pairing;
  state.pairing = null;
  if (pairing?.state === "waiting") {
    try {
      await api(`/api/v1/companion/pairing-sessions/${encodeURIComponent(pairing.session_id)}`, {
        method: "DELETE",
      });
    } catch (error) {
      if (!(error instanceof ApiError && [404, 410, 500].includes(error.status))) throw error;
    }
  }
  if (closeDialog && ui.pairingDialog.open) ui.pairingDialog.close();
  clearPairingRoute();
}

function stopPairingTimers() {
  window.clearInterval(state.pairingCountdownTimer);
  state.pairingCountdownTimer = null;
}

function clearPairingRoute() {
  const route = new URL(window.location.href);
  if (!route.searchParams.has("pair")) return;
  route.searchParams.delete("pair");
  window.history.replaceState({}, "", `${route.pathname}${route.search}${route.hash}`);
}

function initial(value) {
  return String(value || "T").trim().charAt(0).toUpperCase() || "T";
}

function abbreviate(value, visible = 9) {
  const text = String(value || "");
  if (text.length <= visible * 2 + 1) return text;
  return `${text.slice(0, visible)}…${text.slice(-visible)}`;
}

function formatTimestamp(value) {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "Unknown";
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(date);
}

function relativeTime(value) {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "unknown";
  const seconds = Math.round((date.getTime() - Date.now()) / 1000);
  const absolute = Math.abs(seconds);
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  if (absolute < 60) return formatter.format(seconds, "second");
  if (absolute < 3_600) return formatter.format(Math.round(seconds / 60), "minute");
  if (absolute < 86_400) return formatter.format(Math.round(seconds / 3_600), "hour");
  return formatter.format(Math.round(seconds / 86_400), "day");
}

function showToast(message, error = false) {
  window.clearTimeout(state.toastTimer);
  ui.toast.textContent = message;
  ui.toast.classList.toggle("error", error);
  ui.toast.hidden = false;
  state.toastTimer = window.setTimeout(() => {
    ui.toast.hidden = true;
  }, error ? 7_000 : 4_000);
}

function bindInteraction() {
  ui.newShot.addEventListener("click", () => openCreateSurface());
  ui.returnCreate.addEventListener("click", () => openCreateSurface());
  ui.createForm.addEventListener("submit", submitCreate);
  ui.feedbackForm.addEventListener("submit", submitFeedback);
  ui.marketingForm.addEventListener("submit", submitMarketing);
  ui.evolveForm.addEventListener("submit", submitEvolution);
  ui.connectIphone.addEventListener("click", () => openPairing().catch((error) => showToast(error.message, true)));
  ui.pairingClose.addEventListener("click", () => cancelPairingSession(true).catch((error) => showToast(error.message, true)));
  ui.pairingCancel.addEventListener("click", () => cancelPairingSession(true).catch((error) => showToast(error.message, true)));
  ui.pairingDone.addEventListener("click", () => cancelPairingSession(true).catch((error) => showToast(error.message, true)));
  ui.pairingNew.addEventListener("click", () => {
    startPairingSession().catch((error) => showToast(error.message, true));
  });
  ui.pairingDialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    cancelPairingSession(true).catch((error) => showToast(error.message, true));
  });
  ui.feedbackRebind.addEventListener("click", () => {
    synchronizeBindings(true);
    renderFeedbackActions(selectedShot());
  });
  ui.evolveRebind.addEventListener("click", () => {
    synchronizeBindings(true);
    renderFeedbackActions(selectedShot());
  });
  for (const form of [ui.createForm, ui.feedbackForm, ui.marketingForm, ui.evolveForm]) {
    form.addEventListener("input", () => invalidateCommand(form));
  }
  ui.feedbackBody.addEventListener("input", synchronizeBindings);
  ui.evolveIntention.addEventListener("input", synchronizeBindings);
  ui.createName.addEventListener("change", () => {
    const normalized = normalizeShotName(ui.createName.value);
    if (normalized) ui.createName.value = normalized;
  });
  window.addEventListener("popstate", () => {
    applyInitialRoute().catch((error) => showToast(error.message, true));
  });
  window.setInterval(renderDevices, 10_000);
}

async function applyInitialRoute() {
  const route = new URL(window.location.href);
  const pairingSession = route.searchParams.get("pair");
  if (pairingSession !== null && state.pairing?.session_id !== pairingSession) {
    try {
      await resumePairingSession(pairingSession);
    } catch (error) {
      ui.pairingState.textContent = error.message;
      ui.pairingSpinner.classList.add("stopped");
      ui.pairingCancel.hidden = true;
      ui.pairingNew.hidden = false;
      showToast(error.message, true);
    }
  }
  if (route.pathname === "/create") {
    const requested = route.searchParams.get("name");
    let prefilled = "";
    if (requested !== null) {
      prefilled = requested.toLowerCase();
      if (!/^[a-z0-9][a-z0-9-]{0,62}$/.test(prefilled)) {
        ui.createStatus.textContent = "The prefilled Shot name is invalid. Choose a lowercase name before taking the Shot.";
      }
    }
    const pending = route.searchParams.get("pending");
    if (pending !== null) {
      await loadPendingIntention(pending, prefilled);
    } else {
      openCreateSurface({ updateLocation: false, prefilledName: prefilled });
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
    const shot = state.shots.find((candidate) => candidate.shot_id === requested);
    if (shot) {
      selectShot(shot.shot_id, false);
      return;
    }
    showToast("That Shot is not present in this local workspace.", true);
  }
  const requestedShot = route.searchParams.get("shot");
  if (requestedShot !== null) {
    const shot = state.shots.find((candidate) => candidate.shot_id === requestedShot);
    if (shot) {
      selectShot(shot.shot_id, false);
      return;
    }
    showToast("That Shot is not present in this local workspace.", true);
  }
  const first = state.shots.find((shot) => shot.kind === "factory_shot") || state.shots[0];
  if (first) selectShot(first.shot_id, false);
  else openCreateSurface({ updateLocation: false });
}

async function bootstrap() {
  bindInteraction();
  setConnection("connecting", "Connecting to your TOHSENO…");
  try {
    await refreshStudioSession();
    const [health] = await Promise.all([
      api("/api/v1/health"),
      refreshWorkspace(),
      refreshDevices(),
      refreshFactoryDefaults(),
    ]);
    if (
      health.schema !== "tohseno.local-workspace-health/1" ||
      health.origin !== window.location.origin ||
      health.instance_id !== state.sessionInstanceId
    ) {
      throw new Error("Studio could not verify the Local Workspace Service identity.");
    }
    state.health = health;
    setConnection("connected", `Your TOHSENO is running · ${health.service_version}`);
    await applyInitialRoute();
    connectEvents();
  } catch (error) {
    setConnection("offline", "Local Workspace Service unavailable");
    showToast(error.message, true);
  }
}

bootstrap();
