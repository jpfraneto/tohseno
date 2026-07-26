(() => {
  "use strict";

  const SESSION_FRAGMENT_KEY = "tohseno-session";
  const SESSION_API_STORAGE_KEY = "tohseno-studio-api-base";
  let apiBase = "";

  const elements = {
    studioMain: document.querySelector("#studio-main"),
    contactView: document.querySelector("#contact-sheet-view"),
    detailView: document.querySelector("#shot-detail"),
    shotsGrid: document.querySelector("#shots-grid"),
    emptyState: document.querySelector("#empty-state"),
    shotCount: document.querySelector("#shot-count-number"),
    notice: document.querySelector("#studio-notice"),
    watchIndicator: document.querySelector("#watch-indicator"),
    createDialog: document.querySelector("#create-dialog"),
    createForm: document.querySelector("#create-form"),
    createError: document.querySelector("#create-error"),
    creationProgress: document.querySelector("#creation-progress"),
    progressTitle: document.querySelector("#progress-title"),
    progressEvents: document.querySelector("#progress-events"),
    viewCreatedShot: document.querySelector("#view-created-shot"),
    markdownInput: document.querySelector("#shot-markdown"),
    markdownSelection: document.querySelector("#markdown-selection"),
    referenceInput: document.querySelector("#shot-references"),
    referenceSelection: document.querySelector("#reference-selection"),
    referenceDrop: document.querySelector("#reference-drop"),
    intentionInput: document.querySelector("#shot-intention"),
    headerCreateAction: document.querySelector("#header-create-action"),
    detailRegister: document.querySelector("#detail-register"),
    detailSequence: document.querySelector("#detail-sequence"),
    detailTitle: document.querySelector("#detail-title"),
    detailStatus: document.querySelector("#detail-status"),
    detailImage: document.querySelector("#detail-image"),
    detailFallback: document.querySelector("#detail-fallback"),
    detailCaption: document.querySelector("#detail-caption"),
    detailCreated: document.querySelector("#detail-created"),
    detailLocation: document.querySelector("#detail-location"),
    detailShotId: document.querySelector("#detail-shot-id"),
    detailLifecycle: document.querySelector("#detail-lifecycle"),
    detailEvolution: document.querySelector("#detail-evolution"),
    detailCreationActivity: document.querySelector("#detail-creation-activity"),
    detailIntention: document.querySelector("#detail-intention"),
    detailReferences: document.querySelector("#detail-references"),
    noReferences: document.querySelector("#no-references"),
    livePreview: document.querySelector("#live-preview"),
    previewStatus: document.querySelector("#preview-status"),
    simulatorFrame: document.querySelector("#simulator-frame"),
    closePreview: document.querySelector("#close-preview"),
    previewAction: document.querySelector("[data-shot-action='preview']"),
  };

  const state = {
    currentSlug: null,
    activeJobSource: null,
    workspaceSource: null,
    refreshTimer: 0,
    jobFrame: null,
    lastFocusedElement: null,
    createRequestPending: false,
  };

  const progressLabels = {
    allocated: "Shot allocated",
    preparing: "Preparing inputs",
    planning: "Interpreting the intention",
    "plan-ready": "Composition plan ready",
    "preparing-release": "Preparing the pinned factory release",
    "preparing-shot": "Composing the native app",
    "provenance-written": "Private provenance saved locally",
    "manifest-validated": "Manifest validated",
    "baseline-committed": "Neutral baseline committed",
    "repository-created": "Independent repository created",
    "agent-started": "Coding agent started",
    "agent-completed": "Coding agent completed",
    building: "Building the app",
    verifying: "Verifying the shot",
    "simulator-launching": "Launching Simulator",
    "screenshot-captured": "Simulator screenshot captured",
    "preview-unavailable": "Interactive preview unavailable",
    completed: "Shot creation completed",
    failed: "Creation failed",
    interrupted: "Creation interrupted",
  };

  const contactShotKeys = [
    "slug",
    "name",
    "createdAt",
    "sequence",
    "status",
    "shotId",
    "lifecycle",
    "evolution",
    "screenshotUrl",
  ];
  const detailShotKeys = [
    ...contactShotKeys,
    "intention",
    "references",
    "creation",
    "factory",
  ];
  const progressTypes = new Set([
    "allocated",
    "planning",
    "plan-ready",
    "preparing-release",
    "preparing-shot",
    "provenance-written",
    "manifest-validated",
    "baseline-committed",
    "repository-created",
    "agent-started",
    "agent-completed",
    "verifying",
    "building",
    "simulator-launching",
    "screenshot-captured",
    "preview-unavailable",
    "completed",
    "interrupted",
    "failed",
  ]);
  const referenceMediaTypes = new Set([
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/gif",
    "image/heic",
    "image/avif",
  ]);

  function isRecord(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  function asNonEmptyString(value) {
    return typeof value === "string" && value.trim() ? value.trim() : null;
  }

  function hasExactKeys(value, required, optional = []) {
    if (!isRecord(value)) return false;
    const allowed = new Set([...required, ...optional]);
    return required.every((key) => Object.hasOwn(value, key)) &&
      Object.keys(value).every((key) => allowed.has(key));
  }

  function canonicalTimestamp(value) {
    return typeof value === "string" &&
      Number.isFinite(Date.parse(value)) &&
      new Date(value).toISOString() === value;
  }

  function canonicalDisplayText(value, maximumLength) {
    return typeof value === "string" &&
      value.length >= 1 &&
      value.length <= maximumLength &&
      value.trim() === value &&
      !/[\u0000-\u001f\u007f-\u009f]/u.test(value);
  }

  function canonicalErrorMessage(value, fallback) {
    if (
      !hasExactKeys(value, ["error", "message"]) ||
      typeof value.error !== "string" ||
      !/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(value.error) ||
      typeof value.message !== "string" ||
      value.message.length === 0
    ) {
      return fallback;
    }
    return value.message;
  }

  async function readJson(response) {
    const contentType = response.headers.get("content-type") ?? "";
    if (!contentType.includes("application/json")) return null;
    try {
      return await response.json();
    } catch {
      return null;
    }
  }

  async function requestJson(url, options = {}) {
    const response = await fetch(url, {
      ...options,
      headers: {
        Accept: "application/json",
        ...(options.headers ?? {}),
      },
    });
    const payload = await readJson(response);
    if (!response.ok) {
      throw new Error(
        canonicalErrorMessage(
          payload,
          `Local request failed (${response.status}).`,
        ),
      );
    }
    return payload;
  }

  function apiUrl(path) {
    if (
      !/^\/__tohseno\/[a-f0-9]{32}\/api$/.test(apiBase) ||
      !path.startsWith("/")
    ) {
      throw new Error("The private Studio session is unavailable.");
    }
    return `${apiBase}${path}`;
  }

  function mutationHeaders() {
    return {};
  }

  function consumeFragmentToken() {
    const parameters = new URLSearchParams(window.location.hash.slice(1));
    const token = parameters.get(SESSION_FRAGMENT_KEY);
    if (token === null || !/^[A-Za-z0-9_-]{43}$/.test(token)) return null;
    parameters.delete(SESSION_FRAGMENT_KEY);
    const suffix = parameters.toString();
    window.history.replaceState(
      null,
      "",
      `${window.location.pathname}${window.location.search}${
        suffix === "" ? "" : `#${suffix}`
      }`,
    );
    return token;
  }

  async function establishStudioSession() {
    const token = consumeFragmentToken();
    if (token !== null) {
      const response = await fetch("/api/session", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          Accept: "application/json",
          "X-Tohseno-Session": token,
        },
      });
      const payload = await readJson(response);
      if (!response.ok) {
        throw new Error(
          canonicalErrorMessage(
            payload,
            "Studio could not establish its private local session.",
          ),
        );
      }
      if (
        !hasExactKeys(payload, ["apiBase"]) ||
        typeof payload.apiBase !== "string" ||
        !/^\/__tohseno\/[a-f0-9]{32}\/api$/.test(payload.apiBase)
      ) {
        throw new Error("Studio received a non-canonical session response.");
      }
      apiBase = payload.apiBase;
      try {
        window.sessionStorage.setItem(SESSION_API_STORAGE_KEY, apiBase);
      } catch {
        // The current page remains usable when session storage is unavailable.
      }
      return;
    }

    let stored = "";
    try {
      stored = window.sessionStorage.getItem(SESSION_API_STORAGE_KEY) ?? "";
    } catch {
      stored = "";
    }
    if (!/^\/__tohseno\/[a-f0-9]{32}\/api$/.test(stored)) {
      throw new Error(
        "This page has no private Studio session. Reopen the local Studio launcher.",
      );
    }
    apiBase = stored;
  }

  function setNotice(message, kind = "status") {
    if (!(elements.notice instanceof HTMLElement)) return;
    elements.notice.textContent = message;
    elements.notice.dataset.kind = kind;
  }

  function formatDate(value, includeTime = false) {
    const raw = asNonEmptyString(value);
    if (!raw) return "UNKNOWN";
    const date = new Date(raw);
    if (Number.isNaN(date.getTime())) return raw;
    const options = includeTime
      ? {
          year: "numeric",
          month: "short",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
        }
      : {
          year: "2-digit",
          month: "2-digit",
          day: "2-digit",
        };
    return new Intl.DateTimeFormat(undefined, options).format(date);
  }

  function formatSequence(value) {
    if (!Number.isSafeInteger(value) || value < 1) {
      throw new Error("Studio received an invalid Shot sequence.");
    }
    return String(value).padStart(3, "0");
  }

  function safePathSegment(value) {
    return typeof value === "string" &&
        /^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(value)
      ? value
      : null;
  }

  function sameOriginUrl(value) {
    const raw = asNonEmptyString(value);
    if (!raw) return null;
    try {
      const url = new URL(raw, window.location.origin);
      if (url.origin !== window.location.origin) return null;
      if (url.protocol !== "http:" && url.protocol !== "https:") return null;
      return url.href;
    } catch {
      return null;
    }
  }

  function localPreviewUrl(value) {
    const raw = asNonEmptyString(value);
    if (!raw) return null;
    try {
      const url = new URL(raw);
      if (
        url.protocol !== "http:" ||
        url.hostname !== "127.0.0.1" ||
        !/^\d{1,5}$/.test(url.port) ||
        Number(url.port) < 1 ||
        Number(url.port) > 65535 ||
        !/^\/_tohseno\/live\/[A-Za-z0-9_-]{43,128}\/?$/.test(url.pathname) ||
        url.search !== "" ||
        url.hash !== ""
      ) {
        return null;
      }
      return url.href;
    } catch {
      return null;
    }
  }

  function canonicalActionPayload(value, action) {
    if (!isRecord(value)) return null;
    const required = action === "preview" ? ["url"] : [];
    if (!hasExactKeys(value, required, ["message"])) return null;
    if (
      Object.hasOwn(value, "message") &&
      (
        typeof value.message !== "string" ||
        value.message.length > 500
      )
    ) {
      return null;
    }
    if (action !== "preview") return value;
    const url = localPreviewUrl(value.url);
    return url === null
      ? null
      : {
          ...(Object.hasOwn(value, "message") ? { message: value.message } : {}),
          url,
        };
  }

  function currentRoute() {
    const segments = window.location.pathname.split("/").filter(Boolean);
    if (segments.length === 0) return { type: "contact" };
    if (segments[0] !== "shots" || (segments.length !== 2 && segments.length !== 3)) {
      return { type: "not-found" };
    }
    let decodedSlug;
    try {
      decodedSlug = decodeURIComponent(segments[1]);
    } catch {
      return { type: "not-found" };
    }
    const slug = safePathSegment(decodedSlug);
    if (!slug || (segments.length === 3 && segments[2] !== "live")) {
      return { type: "not-found" };
    }
    return { type: "detail", slug, live: segments[2] === "live" };
  }

  function makeElement(tagName, className, text) {
    const element = document.createElement(tagName);
    if (className) element.className = className;
    if (typeof text === "string") element.textContent = text;
    return element;
  }

  function shotHref(slug, suffix = "") {
    return `/shots/${encodeURIComponent(slug)}${suffix}`;
  }

  function canonicalContactShot(value) {
    if (!hasExactKeys(value, contactShotKeys)) return null;
    const slug = safePathSegment(value.slug);
    const screenshotUrl = value.screenshotUrl === null
      ? null
      : sameOriginUrl(value.screenshotUrl);
    if (
      slug === null ||
      !canonicalDisplayText(value.name, 80) ||
      !canonicalTimestamp(value.createdAt) ||
      !Number.isSafeInteger(value.sequence) ||
      value.sequence < 1 ||
      (
        value.status !== "CREATING" &&
        value.status !== "INTERRUPTED" &&
        value.status !== "READY"
      ) ||
      typeof value.shotId !== "string" ||
      !/^shot_[A-Za-z0-9_-]{32}$/u.test(value.shotId) ||
      value.lifecycle !== "EVOLVING" ||
      !Number.isSafeInteger(value.evolution) ||
      value.evolution < 0 ||
      (value.screenshotUrl !== null && screenshotUrl === null)
    ) {
      return null;
    }
    return {
      slug,
      name: value.name,
      createdAt: value.createdAt,
      sequence: value.sequence,
      status: value.status,
      shotId: value.shotId,
      lifecycle: value.lifecycle,
      evolution: value.evolution,
      screenshotUrl,
    };
  }

  function appendExposure(container, shot) {
    if (shot.screenshotUrl) {
      const image = document.createElement("img");
      image.src = shot.screenshotUrl;
      image.alt = `Latest Simulator capture of ${shot.name}`;
      image.loading = "lazy";
      image.decoding = "async";
      image.addEventListener("error", () => {
        const fallback = makeElement("div", "exposure-fallback");
        fallback.append(makeElement("span", null, "NO SIMULATOR CAPTURE"));
        image.replaceWith(fallback);
      });
      container.append(image);
      return;
    }
    const fallback = makeElement("div", "exposure-fallback");
    fallback.append(makeElement("span", null, "NO SIMULATOR CAPTURE"));
    container.append(fallback);
  }

  function createShotFrame(shot) {
    const item = makeElement("li", "shot-frame");
    const link = makeElement("a", "shot-link");
    link.href = shotHref(shot.slug);

    const top = makeElement("div", "frame-topline");
    top.append(makeElement("span", "frame-number", `EXP ${formatSequence(shot.sequence)}`));
    top.append(
      makeElement(
        "span",
        "frame-status",
        shot.lifecycle,
      ),
    );

    const exposure = makeElement("div", "frame-exposure");
    appendExposure(exposure, shot);

    const bottom = makeElement("div", "frame-bottomline");
    bottom.append(makeElement("h2", "frame-name", shot.name));
    bottom.append(makeElement("time", "frame-date", formatDate(shot.createdAt)));
    const time = bottom.querySelector("time");
    if (time && shot.createdAt) time.dateTime = shot.createdAt;

    link.append(top, exposure, bottom);
    item.append(link);
    return item;
  }

  function compareNewestFirst(left, right) {
    const leftTime = new Date(left.createdAt).getTime();
    const rightTime = new Date(right.createdAt).getTime();
    if (leftTime !== rightTime) return rightTime - leftTime;
    return right.sequence - left.sequence;
  }

  function renderShots(payload) {
    if (!(elements.shotsGrid instanceof HTMLOListElement)) return;
    if (
      !hasExactKeys(payload, ["count", "shots"]) ||
      !Number.isSafeInteger(payload.count) ||
      payload.count < 0 ||
      !Array.isArray(payload.shots) ||
      payload.count !== payload.shots.length
    ) {
      throw new Error("The local API returned a non-canonical Shot list.");
    }
    const shots = payload.shots.map(canonicalContactShot);
    if (
      shots.some((shot) => shot === null) ||
      new Set(shots.map((shot) => shot.slug)).size !== shots.length ||
      new Set(shots.map((shot) => shot.shotId)).size !== shots.length ||
      new Set(shots.map((shot) => shot.sequence)).size !== shots.length ||
      shots.some(
        (shot, index) =>
          index > 0 && compareNewestFirst(shots[index - 1], shot) > 0,
      )
    ) {
      throw new Error("The local API returned a non-canonical Shot list.");
    }

    const fragment = document.createDocumentFragment();
    if (state.jobFrame) fragment.append(state.jobFrame);
    for (const shot of shots) fragment.append(createShotFrame(shot));
    elements.shotsGrid.replaceChildren(fragment);

    if (elements.shotCount instanceof HTMLElement) {
      elements.shotCount.textContent = String(payload.count);
    }
    if (elements.emptyState instanceof HTMLElement) {
      elements.emptyState.hidden = shots.length > 0 || Boolean(state.jobFrame);
    }
  }

  async function loadShots({ quiet = false } = {}) {
    if (elements.contactView instanceof HTMLElement) {
      elements.contactView.setAttribute("aria-busy", "true");
    }
    try {
      const payload = await requestJson(apiUrl("/shots"));
      renderShots(payload);
      if (!quiet) setNotice("");
      return payload;
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Could not read /shots.", "error");
      return null;
    } finally {
      if (elements.contactView instanceof HTMLElement) {
        elements.contactView.removeAttribute("aria-busy");
      }
    }
  }

  function scheduleShotRefresh() {
    window.clearTimeout(state.refreshTimer);
    state.refreshTimer = window.setTimeout(() => {
      void loadShots({ quiet: true });
      if (state.currentSlug) void loadDetail(state.currentSlug, { quiet: true });
    }, 160);
  }

  function connectWorkspaceEvents() {
    if (!("EventSource" in window)) {
      if (elements.watchIndicator instanceof HTMLElement) {
        elements.watchIndicator.textContent = "REFRESH TO UPDATE /SHOTS";
        elements.watchIndicator.classList.add("disconnected");
      }
      return;
    }
    state.workspaceSource?.close();
    const source = new EventSource(apiUrl("/events"));
    state.workspaceSource = source;
    source.addEventListener("open", () => {
      if (!(elements.watchIndicator instanceof HTMLElement)) return;
      elements.watchIndicator.textContent = "WATCHING /SHOTS";
      elements.watchIndicator.classList.remove("disconnected");
    });
    source.addEventListener("error", () => {
      if (!(elements.watchIndicator instanceof HTMLElement)) return;
      elements.watchIndicator.textContent = "RECONNECTING /SHOTS";
      elements.watchIndicator.classList.add("disconnected");
    });
    source.addEventListener("message", scheduleShotRefresh);
    for (const eventName of [
      "shots-changed",
      "shot-created",
      "shot-changed",
      "shot-removed",
      "screenshot-captured",
      "completed",
    ]) {
      source.addEventListener(eventName, scheduleShotRefresh);
    }
  }

  function setDetailStatus(message, kind = "status") {
    if (!(elements.detailStatus instanceof HTMLElement)) return;
    elements.detailStatus.textContent = message;
    elements.detailStatus.dataset.kind = kind;
  }

  function canonicalReference(value) {
    if (
      !hasExactKeys(
        value,
        ["originalFilename", "mediaType", "url", "imageUrl"],
      ) ||
      typeof value.originalFilename !== "string" ||
      value.originalFilename.trim().length === 0 ||
      /[\u0000-\u001f\u007f-\u009f]/u.test(value.originalFilename) ||
      !referenceMediaTypes.has(value.mediaType)
    ) {
      return null;
    }
    const url = sameOriginUrl(value.url);
    const imageUrl = sameOriginUrl(value.imageUrl);
    if (url === null || imageUrl === null || url !== imageUrl) return null;
    return {
      name: value.originalFilename,
      url,
      imageUrl,
    };
  }

  function canonicalCreationDetails(value, referenceCount) {
    // Ejection may intentionally remove gitignored private provenance while
    // the canonical Shot metadata keeps its original reference count.
    if (
      !hasExactKeys(
        value,
        ["door", "inputDigest", "referenceCount", "options"],
      ) ||
      (value.door !== "cli" && value.door !== "studio") ||
      typeof value.inputDigest !== "string" ||
      !/^[a-f0-9]{64}$/u.test(value.inputDigest) ||
      !Number.isSafeInteger(value.referenceCount) ||
      value.referenceCount < 0 ||
      value.referenceCount > 8 ||
      (referenceCount > 0 && value.referenceCount !== referenceCount) ||
      !hasExactKeys(
        value.options,
        ["selectedAgent", "agentMode", "verifyAfterAgent", "runAfterCreate"],
      ) ||
      (
        value.options.selectedAgent !== null &&
        value.options.selectedAgent !== "codex" &&
        value.options.selectedAgent !== "claude"
      ) ||
      (
        value.options.agentMode !== "none" &&
        value.options.agentMode !== "interactive" &&
        value.options.agentMode !== "automated"
      ) ||
      typeof value.options.verifyAfterAgent !== "boolean" ||
      typeof value.options.runAfterCreate !== "boolean"
    ) {
      return false;
    }
    return true;
  }

  function canonicalFactoryDetails(value) {
    return hasExactKeys(value, ["releaseId", "cliVersion", "templateVersion"]) &&
      typeof value.releaseId === "string" &&
      /^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u
        .test(value.releaseId) &&
      value.cliVersion === "0.5.0" &&
      value.templateVersion === "ios-kernel-v1";
  }

  function canonicalDetailShot(value) {
    if (!hasExactKeys(value, detailShotKeys)) return null;
    const contact = canonicalContactShot(
      Object.fromEntries(contactShotKeys.map((key) => [key, value[key]])),
    );
    if (
      contact === null ||
      (
        value.intention !== null &&
        (
          typeof value.intention !== "string" ||
          value.intention.trim().length === 0
        )
      ) ||
      !Array.isArray(value.references) ||
      value.references.length > 8
    ) {
      return null;
    }
    const references = value.references.map(canonicalReference);
    if (
      references.some((reference) => reference === null) ||
      new Set(references.map((reference) => reference.url)).size !==
        references.length ||
      !canonicalCreationDetails(value.creation, references.length) ||
      !canonicalFactoryDetails(value.factory)
    ) {
      return null;
    }
    return {
      ...contact,
      intention: value.intention,
      references,
      creation: value.creation,
      factory: value.factory,
    };
  }

  function renderReferences(references) {
    if (!(elements.detailReferences instanceof HTMLUListElement)) return;
    const fragment = document.createDocumentFragment();
    for (const reference of references) {
      const item = document.createElement("li");
      const link = makeElement("a", null);
      link.href = reference.url;
      link.target = "_blank";
      link.rel = "noopener";
      if (reference.imageUrl) {
        const image = document.createElement("img");
        image.src = reference.imageUrl;
        image.alt = "";
        image.loading = "lazy";
        image.decoding = "async";
        link.append(image);
      }
      link.append(makeElement("span", null, reference.name));
      item.append(link);
      fragment.append(item);
    }
    elements.detailReferences.replaceChildren(fragment);
    if (elements.noReferences instanceof HTMLElement) {
      elements.noReferences.hidden = references.length > 0;
    }
  }

  function renderDetail(value) {
    const shot = canonicalDetailShot(value);
    if (!shot) {
      throw new Error("The local API returned a non-canonical Shot detail.");
    }
    state.currentSlug = shot.slug;
    document.title = `${shot.name} — TOHSENO STUDIO`;

    const sequence = formatSequence(shot.sequence);
    if (elements.detailRegister instanceof HTMLElement) {
      elements.detailRegister.textContent = `SHOT / ${sequence}`;
    }
    if (elements.detailSequence instanceof HTMLElement) {
      elements.detailSequence.textContent = `SHOT / ${sequence}`;
    }
    if (elements.detailTitle instanceof HTMLElement) elements.detailTitle.textContent = shot.name;
    if (elements.detailCreated instanceof HTMLElement) {
      elements.detailCreated.textContent = formatDate(shot.createdAt, true);
    }
    if (elements.detailLocation instanceof HTMLElement) {
      elements.detailLocation.textContent = `/shots/${shot.slug}`;
    }
    if (elements.detailShotId instanceof HTMLElement) {
      elements.detailShotId.textContent = shot.shotId;
    }
    if (elements.detailLifecycle instanceof HTMLElement) {
      elements.detailLifecycle.textContent = shot.lifecycle;
    }
    if (elements.detailEvolution instanceof HTMLElement) {
      elements.detailEvolution.textContent = String(shot.evolution);
    }
    if (elements.detailCreationActivity instanceof HTMLElement) {
      elements.detailCreationActivity.textContent = shot.status;
    }
    if (elements.detailIntention instanceof HTMLElement) {
      elements.detailIntention.textContent =
        shot.intention ?? "No intention was found in this shot's provenance.";
    }
    renderReferences(shot.references);

    if (
      elements.detailImage instanceof HTMLImageElement &&
      elements.detailFallback instanceof HTMLElement &&
      elements.detailCaption instanceof HTMLElement
    ) {
      elements.detailImage.hidden = true;
      elements.detailImage.removeAttribute("src");
      elements.detailImage.alt = "";
      elements.detailFallback.hidden = false;
      elements.detailCaption.textContent = "LATEST CAPTURE / NOT AVAILABLE";
      if (shot.screenshotUrl) {
        elements.detailImage.src = shot.screenshotUrl;
        elements.detailImage.alt = `Latest Simulator capture of ${shot.name}`;
        elements.detailImage.hidden = false;
        elements.detailFallback.hidden = true;
        elements.detailCaption.textContent = "LATEST SIMULATOR CAPTURE";
      }
    }
  }

  async function loadDetail(slug, { quiet = false } = {}) {
    if (elements.detailView instanceof HTMLElement) {
      elements.detailView.setAttribute("aria-busy", "true");
    }
    try {
      if (!quiet) setDetailStatus("READING SHOT…");
      const payload = await requestJson(
        apiUrl(`/shots/${encodeURIComponent(slug)}`),
      );
      renderDetail(payload);
      if (!quiet) setDetailStatus("");
      return payload;
    } catch (error) {
      setDetailStatus(
        error instanceof Error ? error.message : "Could not read this shot.",
        "error",
      );
      return null;
    } finally {
      if (elements.detailView instanceof HTMLElement) {
        elements.detailView.removeAttribute("aria-busy");
      }
    }
  }

  function showLivePreview(message) {
    if (!(elements.livePreview instanceof HTMLElement)) return;
    elements.livePreview.hidden = false;
    if (elements.previewStatus instanceof HTMLElement) {
      elements.previewStatus.textContent = message;
      elements.previewStatus.dataset.kind = "status";
    }
    elements.livePreview.scrollIntoView({ block: "start" });
  }

  function closeLivePreview({ updateRoute = true } = {}) {
    if (elements.simulatorFrame instanceof HTMLIFrameElement) {
      elements.simulatorFrame.removeAttribute("src");
      elements.simulatorFrame.hidden = true;
    }
    if (elements.livePreview instanceof HTMLElement) elements.livePreview.hidden = true;
    if (updateRoute && state.currentSlug) {
      window.history.replaceState(null, "", shotHref(state.currentSlug));
    }
    if (elements.previewAction instanceof HTMLButtonElement) {
      elements.previewAction.focus();
    }
  }

  async function stopLivePreview() {
    const slug = state.currentSlug;
    try {
      if (slug) {
        const payload = await requestJson(
          apiUrl(`/shots/${encodeURIComponent(slug)}/stop-preview`),
          {
            method: "POST",
            headers: mutationHeaders(),
          },
        );
        if (canonicalActionPayload(payload, "stop-preview") === null) {
          throw new Error("The local API returned a non-canonical action response.");
        }
      }
    } catch (error) {
      setDetailStatus(
        error instanceof Error ? error.message : "The live preview could not be stopped.",
        "error",
      );
    } finally {
      closeLivePreview();
    }
  }

  async function runShotAction(button, action) {
    const slug = state.currentSlug;
    if (!slug) return;
    const labels = {
      run: "BUILDING AND LAUNCHING IN SIMULATOR…",
      preview: "STARTING INTERACTIVE SIMULATOR PREVIEW…",
      verify: "VERIFYING SHOT…",
      "open-xcode": "OPENING XCODE…",
      reveal: "REVEALING SHOT FOLDER…",
    };
    const doneLabels = {
      run: "SHOT IS RUNNING IN APPLE SIMULATOR.",
      verify: "SHOT VERIFIED.",
      "open-xcode": "XCODE OPENED.",
      reveal: "SHOT FOLDER REVEALED.",
    };

    button.disabled = true;
    setDetailStatus(labels[action] ?? "WORKING…");
    if (action === "preview") showLivePreview(labels.preview);
    try {
      const payload = await requestJson(
        apiUrl(
          `/shots/${encodeURIComponent(slug)}/${encodeURIComponent(action)}`,
        ),
        {
          method: "POST",
          headers: mutationHeaders(),
        },
      );
      const result = canonicalActionPayload(payload, action);
      if (result === null) {
        throw new Error("The local API returned a non-canonical action response.");
      }
      if (action === "preview") {
        if (elements.simulatorFrame instanceof HTMLIFrameElement) {
          elements.simulatorFrame.src = result.url;
          elements.simulatorFrame.hidden = false;
        }
        if (elements.previewStatus instanceof HTMLElement) {
          elements.previewStatus.textContent = "LIVE — INTERACTING WITH APPLE SIMULATOR ON THIS MAC.";
        }
        window.history.replaceState(null, "", shotHref(slug, "/live"));
        setDetailStatus("LIVE PREVIEW READY.");
      } else {
        setDetailStatus(
          Object.hasOwn(result, "message")
            ? result.message
            : doneLabels[action] ?? "DONE.",
        );
        if (action === "run" || action === "verify") scheduleShotRefresh();
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "The local action failed.";
      setDetailStatus(message, "error");
      if (action === "preview" && elements.previewStatus instanceof HTMLElement) {
        elements.previewStatus.textContent = message;
        elements.previewStatus.dataset.kind = "error";
      }
    } finally {
      button.disabled = false;
    }
  }

  function openCreateDialog() {
    if (!(elements.createDialog instanceof HTMLDialogElement)) return;
    state.lastFocusedElement =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!elements.createDialog.open) elements.createDialog.showModal();
    window.setTimeout(() => {
      if (
        elements.createForm instanceof HTMLFormElement &&
        !elements.createForm.hidden &&
        elements.intentionInput instanceof HTMLTextAreaElement
      ) {
        elements.intentionInput.focus();
      } else if (elements.progressTitle instanceof HTMLElement) {
        elements.progressTitle.focus();
      }
    }, 0);
  }

  function setCreateRequestPending(pending) {
    state.createRequestPending = pending;
    if (elements.createForm instanceof HTMLFormElement) {
      elements.createForm.toggleAttribute("aria-busy", pending);
    }
    for (const control of document.querySelectorAll("[data-close-create]")) {
      if (control instanceof HTMLButtonElement) control.disabled = pending;
    }
  }

  function isVisibleEnabledFocusTarget(element) {
    if (!(element instanceof HTMLElement) || !element.isConnected) return false;
    if (element.closest("[hidden], [inert], [aria-hidden='true']")) return false;
    if (element.matches(":disabled, [aria-disabled='true']")) return false;
    const style = window.getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden") return false;
    return element.getClientRects().length > 0;
  }

  function restoreCreateDialogFocus() {
    const requestedTarget = state.lastFocusedElement;
    state.lastFocusedElement = null;
    for (const candidate of [requestedTarget, elements.headerCreateAction]) {
      if (
        !(candidate instanceof HTMLElement) ||
        !candidate.matches("[data-open-create]") ||
        !isVisibleEnabledFocusTarget(candidate)
      ) {
        continue;
      }
      candidate.focus();
      if (document.activeElement === candidate) return;
    }
    for (const candidate of [elements.studioMain]) {
      if (!isVisibleEnabledFocusTarget(candidate)) continue;
      candidate.focus();
      if (document.activeElement === candidate) return;
    }
  }

  function closeCreateDialog() {
    if (!(elements.createDialog instanceof HTMLDialogElement)) return;
    if (state.createRequestPending) return;
    if (elements.createDialog.open) elements.createDialog.close();
  }

  function setCreateError(message, input) {
    if (elements.createError instanceof HTMLElement) {
      elements.createError.textContent = message;
      elements.createError.hidden = !message;
    }
    if (input instanceof HTMLElement) {
      input.setAttribute("aria-invalid", "true");
      input.focus();
    }
  }

  function clearCreateErrors() {
    if (elements.createError instanceof HTMLElement) {
      elements.createError.textContent = "";
      elements.createError.hidden = true;
    }
    for (const input of elements.createForm?.querySelectorAll("[aria-invalid='true']") ?? []) {
      input.removeAttribute("aria-invalid");
    }
    if (elements.referenceDrop instanceof HTMLElement) {
      delete elements.referenceDrop.dataset.invalid;
    }
  }

  function updateFileSelections() {
    if (
      elements.markdownInput instanceof HTMLInputElement &&
      elements.markdownSelection instanceof HTMLElement
    ) {
      const markdown = elements.markdownInput.files?.[0];
      elements.markdownSelection.textContent = markdown
        ? `SELECTED / ${markdown.name}`
        : "No file selected.";
    }
    if (
      elements.referenceInput instanceof HTMLInputElement &&
      elements.referenceSelection instanceof HTMLElement
    ) {
      const references = Array.from(elements.referenceInput.files ?? []);
      elements.referenceSelection.textContent =
        references.length === 0
          ? "No reference images selected."
          : `${references.length} SELECTED / ${references.map((file) => file.name).join(" · ")}`;
    }
  }

  function validateCreateForm() {
    clearCreateErrors();
    const intention =
      elements.intentionInput instanceof HTMLTextAreaElement
        ? elements.intentionInput.value.trim()
        : "";
    const markdown =
      elements.markdownInput instanceof HTMLInputElement
        ? elements.markdownInput.files?.[0]
        : null;
    if (!intention && !markdown) {
      setCreateError(
        "Write an intention or select one Markdown file before creating the shot.",
        elements.intentionInput,
      );
      return false;
    }
    if (markdown && !markdown.name.toLowerCase().endsWith(".md")) {
      setCreateError("The intention upload must be a .md file.", elements.markdownInput);
      return false;
    }
    const references =
      elements.referenceInput instanceof HTMLInputElement
        ? Array.from(elements.referenceInput.files ?? [])
        : [];
    const acceptedExtensions = /\.(png|jpe?g|webp|gif|heic|heif|avif)$/i;
    const invalidReference = references.find((file) => !acceptedExtensions.test(file.name));
    if (invalidReference) {
      if (elements.referenceDrop instanceof HTMLElement) {
        elements.referenceDrop.dataset.invalid = "true";
      }
      setCreateError(
        `${invalidReference.name} is not a supported image reference.`,
        elements.referenceInput,
      );
      return false;
    }
    return true;
  }

  function makeJobFrame() {
    const item = makeElement("li", "shot-frame");
    item.setAttribute("aria-live", "polite");
    const frame = makeElement("article", "job-frame");
    const top = makeElement("div", "frame-topline");
    top.append(makeElement("span", "frame-number", "EXP —"));
    top.append(makeElement("span", "frame-status", "CREATING"));
    const exposure = makeElement("div", "frame-exposure");
    exposure.append(makeElement("span", null, "DEVELOPING…"));
    const bottom = makeElement("div", "frame-bottomline");
    const nameInput = document.querySelector("#shot-name");
    const name = nameInput instanceof HTMLInputElement ? nameInput.value.trim() : "";
    bottom.append(makeElement("h2", "frame-name", name || "NEW SHOT"));
    bottom.append(makeElement("span", "frame-date", "NOW"));
    frame.append(top, exposure, bottom);
    item.append(frame);
    return item;
  }

  function updateJobFrame(event) {
    if (!(state.jobFrame instanceof HTMLElement)) return;
    const status = state.jobFrame.querySelector(".frame-status");
    const number = state.jobFrame.querySelector(".frame-number");
    const developing = state.jobFrame.querySelector(".frame-exposure span");
    const type = asNonEmptyString(event.type);
    if (type === null) throw new Error("Studio received invalid progress.");
    if (status) status.textContent = type.toUpperCase();
    if (developing) developing.textContent = progressLabels[type] ?? type.replaceAll("-", " ");
    if (type === "allocated" && number) {
      number.textContent = `EXP ${formatSequence(event.sequence)}`;
    }
  }

  function appendProgressEvent(event) {
    if (!(elements.progressEvents instanceof HTMLOListElement)) return;
    const type = asNonEmptyString(event.type);
    if (type === null) throw new Error("Studio received invalid progress.");
    const item = document.createElement("li");
    if (type === "failed" || type === "interrupted") {
      item.classList.add("progress-event-failed");
    }
    item.append(makeElement("span", "progress-event-type", type.replaceAll("-", " ")));
    item.append(
      makeElement(
        "span",
        "progress-event-message",
        asNonEmptyString(event.message) ?? progressLabels[type] ?? "Factory activity",
      ),
    );
    if (type === "plan-ready") {
      const plan = event.plan;
      const details = makeElement("dl", "progress-plan");
      for (const [label, value] of [
        ["APP", plan.appName],
        ["STARTING SHAPE", plan.template],
        [
          "SKILLS",
          plan.skills.length === 0
            ? "Neutral kernel only"
            : plan.skills.join(" · "),
        ],
        ["DATA", plan.dataStrategy],
        ["RUNTIME IDENTITY", plan.identityStrategy],
      ]) {
        details.append(
          makeElement("dt", null, label),
          makeElement("dd", null, value),
        );
      }
      details.append(
        makeElement("dt", null, "FIRST DEFINITION OF DONE"),
        makeElement("dd", null, plan.definitionOfDone.join(" · ")),
      );
      if (plan.fallback === true) {
        details.append(
          makeElement("dt", null, "PLAN STATUS"),
          makeElement("dd", null, "Safe Blank fallback"),
        );
      }
      item.append(details);
    }
    elements.progressEvents.append(item);
    item.scrollIntoView({ block: "nearest" });
    updateJobFrame(event);
  }

  function canonicalPlan(value) {
    if (
      !hasExactKeys(value, [
        "appName",
        "template",
        "skills",
        "dataStrategy",
        "identityStrategy",
        "definitionOfDone",
        "fallback",
      ]) ||
      !canonicalDisplayText(value.appName, 80) ||
      typeof value.template !== "string" ||
      !/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(value.template) ||
      !Array.isArray(value.skills) ||
      !value.skills.every((skill) =>
        typeof skill === "string" &&
        /^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(skill)
      ) ||
      new Set(value.skills).size !== value.skills.length ||
      (
        value.dataStrategy !== "local" &&
        value.dataStrategy !== "remote" &&
        value.dataStrategy !== "hybrid"
      ) ||
      (
        value.identityStrategy !== "none" &&
        value.identityStrategy !== "local-device" &&
        value.identityStrategy !== "wallet" &&
        value.identityStrategy !== "account"
      ) ||
      !Array.isArray(value.definitionOfDone) ||
      value.definitionOfDone.length < 1 ||
      !value.definitionOfDone.every((item) =>
        typeof item === "string" &&
        item.trim() === item &&
        item.length >= 1 &&
        item.length <= 240
      ) ||
      typeof value.fallback !== "boolean"
    ) {
      return null;
    }
    return value;
  }

  function canonicalProgressEvent(value, expectedJobId) {
    if (
      !hasExactKeys(
        value,
        ["schemaVersion", "jobId", "at", "type", "door"],
        ["slug", "sequence", "message", "plan"],
      ) ||
      value.schemaVersion !== 1 ||
      value.jobId !== expectedJobId ||
      !canonicalTimestamp(value.at) ||
      !progressTypes.has(value.type) ||
      value.door !== "studio" ||
      (
        Object.hasOwn(value, "slug") &&
        safePathSegment(value.slug) === null
      ) ||
      (
        Object.hasOwn(value, "sequence") &&
        (!Number.isSafeInteger(value.sequence) || value.sequence < 1)
      ) ||
      (
        Object.hasOwn(value, "message") &&
        (
          typeof value.message !== "string" ||
          new TextEncoder().encode(value.message).byteLength > 2_048 ||
          /[\u0000-\u001f\u007f]/u.test(value.message)
        )
      ) ||
      (
        Object.hasOwn(value, "plan") &&
        (
          value.type !== "plan-ready" ||
          canonicalPlan(value.plan) === null
        )
      ) ||
      (value.type === "plan-ready" && !Object.hasOwn(value, "plan")) ||
      (
        value.type === "allocated" &&
        (
          !Object.hasOwn(value, "slug") ||
          !Object.hasOwn(value, "sequence")
        )
      ) ||
      (
        value.type === "completed" &&
        (
          !Object.hasOwn(value, "slug") ||
          !Object.hasOwn(value, "sequence")
        )
      )
    ) {
      return null;
    }
    return value;
  }

  function parseEventPayload(event, expectedJobId) {
    if (
      !(event instanceof MessageEvent) ||
      typeof event.data !== "string"
    ) {
      throw new Error("Studio received a non-canonical factory progress event.");
    }
    let parsed;
    try {
      parsed = JSON.parse(event.data);
    } catch {
      throw new Error("Studio received a non-canonical factory progress event.");
    }
    const canonical = canonicalProgressEvent(parsed, expectedJobId);
    if (canonical === null) {
      throw new Error("Studio received a non-canonical factory progress event.");
    }
    return canonical;
  }

  function finishCreation(event, succeeded) {
    state.activeJobSource?.close();
    state.activeJobSource = null;
    const slug = safePathSegment(event.slug);
    if (succeeded && slug && elements.viewCreatedShot instanceof HTMLAnchorElement) {
      elements.viewCreatedShot.href = shotHref(slug);
      elements.viewCreatedShot.hidden = false;
    }
    if (!succeeded) {
      setCreateError(
        asNonEmptyString(event.message) ?? "Shot creation stopped before completion.",
      );
    }
    window.setTimeout(() => {
      state.jobFrame = null;
      void loadShots({ quiet: true });
    }, succeeded ? 300 : 0);
  }

  function connectJobEvents(jobId) {
    if (!("EventSource" in window)) {
      appendProgressEvent({
        type: "failed",
        message: "This browser cannot stream factory progress.",
      });
      finishCreation({ message: "This browser cannot stream factory progress." }, false);
      return;
    }
    const source = new EventSource(
      apiUrl(`/jobs/${encodeURIComponent(jobId)}/events`),
    );
    state.activeJobSource = source;
    const receive = (rawEvent) => {
      let event;
      try {
        event = parseEventPayload(rawEvent, jobId);
      } catch (error) {
        const message = error instanceof Error
          ? error.message
          : "Studio received a non-canonical factory progress event.";
        appendProgressEvent({ type: "failed", message });
        finishCreation({ message }, false);
        return;
      }
      appendProgressEvent(event);
      if (event.type === "completed") finishCreation(event, true);
      if (event.type === "failed" || event.type === "interrupted") finishCreation(event, false);
    };
    source.addEventListener("message", receive);
    source.addEventListener("error", () => {
      if (source.readyState !== EventSource.CLOSED) {
        appendProgressEvent({
          type: "activity",
          message: "Progress connection interrupted; reconnecting locally…",
        });
      }
    });
  }

  async function submitCreateForm(event) {
    event.preventDefault();
    if (
      state.createRequestPending ||
      !(elements.createForm instanceof HTMLFormElement) ||
      !validateCreateForm()
    ) {
      return;
    }
    const submit = elements.createForm.querySelector("button[type='submit']");
    if (submit instanceof HTMLButtonElement) submit.disabled = true;
    setCreateRequestPending(true);
    clearCreateErrors();

    try {
      const response = await fetch(apiUrl("/shots"), {
        method: "POST",
        headers: mutationHeaders(),
        body: new FormData(elements.createForm),
      });
      const payload = await readJson(response);
      if (!response.ok) {
        throw new Error(
          canonicalErrorMessage(
            payload,
            `The factory rejected this shot (${response.status}).`,
          ),
        );
      }
      if (
        !hasExactKeys(payload, ["jobId"]) ||
        typeof payload.jobId !== "string" ||
        !/^[A-Za-z0-9][A-Za-z0-9-]{7,79}$/u.test(payload.jobId)
      ) {
        throw new Error("The factory returned a non-canonical creation job.");
      }
      const jobId = payload.jobId;

      state.jobFrame = makeJobFrame();
      if (elements.shotsGrid instanceof HTMLOListElement) {
        elements.shotsGrid.prepend(state.jobFrame);
      }
      if (elements.emptyState instanceof HTMLElement) elements.emptyState.hidden = true;
      elements.createForm.hidden = true;
      if (elements.creationProgress instanceof HTMLElement) elements.creationProgress.hidden = false;
      if (elements.progressEvents instanceof HTMLOListElement) elements.progressEvents.replaceChildren();
      appendProgressEvent({ type: "preparing", message: "Inputs accepted by the local factory." });
      if (elements.progressTitle instanceof HTMLElement) elements.progressTitle.focus();
      connectJobEvents(jobId);
    } catch (error) {
      setCreateError(error instanceof Error ? error.message : "Could not start shot creation.");
      if (submit instanceof HTMLButtonElement) submit.disabled = false;
    } finally {
      setCreateRequestPending(false);
    }
  }

  function resetCreateDialog() {
    setCreateRequestPending(false);
    state.activeJobSource?.close();
    state.activeJobSource = null;
    if (elements.createForm instanceof HTMLFormElement) {
      elements.createForm.reset();
      elements.createForm.hidden = false;
      const submit = elements.createForm.querySelector("button[type='submit']");
      if (submit instanceof HTMLButtonElement) submit.disabled = false;
    }
    if (elements.creationProgress instanceof HTMLElement) elements.creationProgress.hidden = true;
    if (elements.progressEvents instanceof HTMLOListElement) elements.progressEvents.replaceChildren();
    if (elements.viewCreatedShot instanceof HTMLAnchorElement) {
      elements.viewCreatedShot.hidden = true;
      elements.viewCreatedShot.href = "/";
    }
    clearCreateErrors();
    updateFileSelections();
  }

  function installDragAndDrop() {
    if (
      !(elements.referenceDrop instanceof HTMLElement) ||
      !(elements.referenceInput instanceof HTMLInputElement)
    ) {
      return;
    }
    const stop = (event) => {
      event.preventDefault();
      event.stopPropagation();
    };
    for (const eventName of ["dragenter", "dragover"]) {
      elements.referenceDrop.addEventListener(eventName, (event) => {
        stop(event);
        elements.referenceDrop.dataset.dragging = "true";
      });
    }
    for (const eventName of ["dragleave", "drop"]) {
      elements.referenceDrop.addEventListener(eventName, (event) => {
        stop(event);
        delete elements.referenceDrop.dataset.dragging;
      });
    }
    elements.referenceDrop.addEventListener("drop", (event) => {
      const files = Array.from(event.dataTransfer?.files ?? []);
      if (files.length === 0) return;
      try {
        const transfer = new DataTransfer();
        for (const file of files) transfer.items.add(file);
        elements.referenceInput.files = transfer.files;
        updateFileSelections();
      } catch {
        setCreateError(
          "This browser could not attach the dropped files. Use CHOOSE FILES instead.",
          elements.referenceInput,
        );
      }
    });
  }

  function installEventHandlers() {
    for (const button of document.querySelectorAll("[data-open-create]")) {
      button.addEventListener("click", openCreateDialog);
    }
    for (const button of document.querySelectorAll("[data-close-create]")) {
      button.addEventListener("click", closeCreateDialog);
    }
    elements.createDialog?.addEventListener("click", (event) => {
      if (event.target === elements.createDialog) closeCreateDialog();
    });
    elements.createDialog?.addEventListener("cancel", (event) => {
      if (state.createRequestPending) event.preventDefault();
    });
    elements.createDialog?.addEventListener("close", () => {
      if (!state.activeJobSource) resetCreateDialog();
      restoreCreateDialogFocus();
    });
    elements.createForm?.addEventListener("submit", submitCreateForm);
    elements.markdownInput?.addEventListener("change", updateFileSelections);
    elements.referenceInput?.addEventListener("change", updateFileSelections);

    for (const button of document.querySelectorAll("[data-shot-action]")) {
      if (!(button instanceof HTMLButtonElement)) continue;
      const action = button.dataset.shotAction;
      if (!action) continue;
      button.addEventListener("click", () => void runShotAction(button, action));
    }
    elements.closePreview?.addEventListener("click", () => void stopLivePreview());
    elements.detailImage?.addEventListener("error", () => {
      if (!(elements.detailImage instanceof HTMLImageElement)) return;
      elements.detailImage.hidden = true;
      if (elements.detailFallback instanceof HTMLElement) elements.detailFallback.hidden = false;
      if (elements.detailCaption instanceof HTMLElement) {
        elements.detailCaption.textContent = "LATEST CAPTURE / NOT AVAILABLE";
      }
    });
    document.addEventListener("visibilitychange", () => {
      if (!document.hidden) scheduleShotRefresh();
    });
    window.addEventListener("beforeunload", () => {
      state.activeJobSource?.close();
      state.workspaceSource?.close();
    });
    installDragAndDrop();
  }

  async function initialize() {
    if (
      elements.shotCount instanceof HTMLElement &&
      elements.shotCount.textContent?.includes("{{")
    ) {
      elements.shotCount.textContent = "—";
    }
    installEventHandlers();
    try {
      await establishStudioSession();
    } catch (error) {
      const message = error instanceof Error
        ? error.message
        : "Studio could not establish its private local session.";
      setNotice(message, "error");
      setDetailStatus(message, "error");
      return;
    }
    connectWorkspaceEvents();
    const route = currentRoute();
    if (route.type === "detail") {
      if (elements.contactView instanceof HTMLElement) elements.contactView.hidden = true;
      if (elements.detailView instanceof HTMLElement) elements.detailView.hidden = false;
      await Promise.all([loadShots({ quiet: true }), loadDetail(route.slug)]);
      if (route.live) {
        showLivePreview(
          "Press OPEN LIVE PREVIEW to build, launch, and connect to Apple Simulator.",
        );
      }
      return;
    }
    if (route.type === "not-found") {
      if (elements.contactView instanceof HTMLElement) elements.contactView.hidden = true;
      if (elements.detailView instanceof HTMLElement) elements.detailView.hidden = false;
      setDetailStatus("UNKNOWN STUDIO ROUTE.", "error");
      return;
    }
    await loadShots();
  }

  void initialize();
})();
