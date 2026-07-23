(() => {
  "use strict";

  const sessionToken =
    document.querySelector("meta[name='tohseno-session']")?.getAttribute("content") ?? "";

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
    "agent-started": "Coding agent started",
    building: "Building the app",
    verifying: "Verifying the shot",
    "simulator-launching": "Launching Simulator",
    "screenshot-captured": "Simulator screenshot captured",
    completed: "Shot completed",
    failed: "Creation failed",
    interrupted: "Creation interrupted",
  };

  function isRecord(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  function asNonEmptyString(value) {
    return typeof value === "string" && value.trim() ? value.trim() : null;
  }

  function readErrorMessage(value, fallback) {
    if (!isRecord(value)) return fallback;
    return asNonEmptyString(value.message) ?? asNonEmptyString(value.error) ?? fallback;
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
      throw new Error(readErrorMessage(payload, `Local request failed (${response.status}).`));
    }
    return payload;
  }

  function mutationHeaders() {
    return {
      "X-Tohseno-Session": sessionToken,
    };
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
    if (typeof value === "number" && Number.isFinite(value)) {
      return String(value).padStart(3, "0");
    }
    const raw = asNonEmptyString(value);
    if (!raw) return "—";
    return /^\d+$/.test(raw) ? raw.padStart(3, "0") : raw;
  }

  function safePathSegment(value) {
    const segment = asNonEmptyString(value);
    if (!segment || !/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(segment)) return null;
    return segment;
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

  function normalizedShot(value) {
    if (!isRecord(value)) return null;
    const slug = safePathSegment(value.slug);
    if (!slug) return null;
    return {
      ...value,
      slug,
      name: asNonEmptyString(value.name) ?? slug,
      createdAt: asNonEmptyString(value.createdAt),
      screenshotUrl: sameOriginUrl(value.screenshotUrl),
      status: asNonEmptyString(value.status),
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

  function creationActivityLabel(value) {
    const status = asNonEmptyString(value) ?? "READY";
    return `CREATION / ${status}`;
  }

  function createShotFrame(shot) {
    const item = makeElement("li", "shot-frame");
    const link = makeElement("a", "shot-link");
    link.href = shotHref(shot.slug);

    const top = makeElement("div", "frame-topline");
    top.append(makeElement("span", "frame-number", `EXP ${formatSequence(shot.sequence)}`));
    top.append(makeElement("span", "frame-status", creationActivityLabel(shot.status)));

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
    const parsedLeftTime = left.createdAt ? new Date(left.createdAt).getTime() : 0;
    const parsedRightTime = right.createdAt ? new Date(right.createdAt).getTime() : 0;
    const leftTime = Number.isFinite(parsedLeftTime) ? parsedLeftTime : 0;
    const rightTime = Number.isFinite(parsedRightTime) ? parsedRightTime : 0;
    if (leftTime !== rightTime) return rightTime - leftTime;
    const leftSequence = Number(left.sequence) || 0;
    const rightSequence = Number(right.sequence) || 0;
    return rightSequence - leftSequence;
  }

  function renderShots(payload) {
    if (!(elements.shotsGrid instanceof HTMLOListElement)) return;
    const rawShots = isRecord(payload) && Array.isArray(payload.shots) ? payload.shots : [];
    const shots = rawShots.map(normalizedShot).filter(Boolean).sort(compareNewestFirst);
    const count =
      isRecord(payload) && typeof payload.count === "number" && Number.isFinite(payload.count)
        ? Math.max(0, payload.count)
        : shots.length;

    const fragment = document.createDocumentFragment();
    if (state.jobFrame) fragment.append(state.jobFrame);
    for (const shot of shots) fragment.append(createShotFrame(shot));
    elements.shotsGrid.replaceChildren(fragment);

    if (elements.shotCount instanceof HTMLElement) {
      elements.shotCount.textContent = String(count);
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
      const payload = await requestJson("/api/shots");
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
    const source = new EventSource("/api/events");
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

  function referenceData(value) {
    if (typeof value === "string") {
      const url = sameOriginUrl(value);
      return url ? { name: "REFERENCE", url, imageUrl: url } : null;
    }
    if (!isRecord(value)) return null;
    const url = sameOriginUrl(value.url ?? value.href ?? value.downloadUrl);
    const imageUrl = sameOriginUrl(value.imageUrl ?? value.thumbnailUrl ?? value.url);
    if (!url && !imageUrl) return null;
    return {
      name:
        asNonEmptyString(value.originalFilename) ??
        asNonEmptyString(value.filename) ??
        asNonEmptyString(value.name) ??
        "REFERENCE",
      url: url ?? imageUrl,
      imageUrl,
    };
  }

  function renderReferences(rawReferences) {
    if (!(elements.detailReferences instanceof HTMLUListElement)) return;
    const references = Array.isArray(rawReferences)
      ? rawReferences.map(referenceData).filter(Boolean)
      : [];
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
    const shot = normalizedShot(value);
    if (!shot) throw new Error("The local API returned an invalid shot.");
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
    if (elements.detailCreationActivity instanceof HTMLElement) {
      elements.detailCreationActivity.textContent = shot.status ?? "READY";
    }
    if (elements.detailIntention instanceof HTMLElement) {
      elements.detailIntention.textContent =
        asNonEmptyString(value.intention) ?? "No intention was found in this shot's provenance.";
    }
    renderReferences(value.references);

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
      const payload = await requestJson(`/api/shots/${encodeURIComponent(slug)}`);
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
        await requestJson(
          `/api/shots/${encodeURIComponent(slug)}/stop-preview`,
          {
            method: "POST",
            headers: mutationHeaders(),
          },
        );
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
        `/api/shots/${encodeURIComponent(slug)}/${encodeURIComponent(action)}`,
        {
          method: "POST",
          headers: mutationHeaders(),
        },
      );
      if (action === "preview") {
        const previewUrl = isRecord(payload) ? localPreviewUrl(payload.url) : null;
        if (!previewUrl) throw new Error("The preview helper returned an unsafe or invalid URL.");
        if (elements.simulatorFrame instanceof HTMLIFrameElement) {
          elements.simulatorFrame.src = previewUrl;
          elements.simulatorFrame.hidden = false;
        }
        if (elements.previewStatus instanceof HTMLElement) {
          elements.previewStatus.textContent = "LIVE — INTERACTING WITH APPLE SIMULATOR ON THIS MAC.";
        }
        window.history.replaceState(null, "", shotHref(slug, "/live"));
        setDetailStatus("LIVE PREVIEW READY.");
      } else {
        setDetailStatus(
          isRecord(payload) && asNonEmptyString(payload.message)
            ? payload.message
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
    const type = asNonEmptyString(event.type) ?? "activity";
    if (status) status.textContent = type.toUpperCase();
    if (developing) developing.textContent = progressLabels[type] ?? type.replaceAll("-", " ");
    if (type === "allocated" && number) {
      number.textContent = `EXP ${formatSequence(event.shot ?? event.sequence)}`;
    }
  }

  function appendProgressEvent(event) {
    if (!(elements.progressEvents instanceof HTMLOListElement)) return;
    const type = asNonEmptyString(event.type) ?? "activity";
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
    elements.progressEvents.append(item);
    item.scrollIntoView({ block: "nearest" });
    updateJobFrame(event);
  }

  function parseEventPayload(event) {
    if (!(event instanceof MessageEvent)) return { type: event.type };
    try {
      const parsed = JSON.parse(event.data);
      if (isRecord(parsed)) {
        return {
          ...parsed,
          type: asNonEmptyString(parsed.type) ?? event.type,
        };
      }
    } catch {
      // A plain-text event is still useful as a content-safe status message.
    }
    return {
      type: event.type === "message" ? "activity" : event.type,
      message: asNonEmptyString(event.data) ?? "Factory activity",
    };
  }

  function finishCreation(event, succeeded) {
    state.activeJobSource?.close();
    state.activeJobSource = null;
    const slug = safePathSegment(event.slug ?? (isRecord(event.shot) ? event.shot.slug : null));
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
    const source = new EventSource(`/api/jobs/${encodeURIComponent(jobId)}/events`);
    state.activeJobSource = source;
    const receive = (rawEvent) => {
      const event = parseEventPayload(rawEvent);
      appendProgressEvent(event);
      if (event.type === "completed") finishCreation(event, true);
      if (event.type === "failed" || event.type === "interrupted") finishCreation(event, false);
    };
    source.addEventListener("message", receive);
    for (const eventName of Object.keys(progressLabels)) {
      source.addEventListener(eventName, receive);
    }
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
      const response = await fetch("/api/shots", {
        method: "POST",
        headers: mutationHeaders(),
        body: new FormData(elements.createForm),
      });
      const payload = await readJson(response);
      if (!response.ok) {
        throw new Error(readErrorMessage(payload, `The factory rejected this shot (${response.status}).`));
      }
      const jobId = isRecord(payload) ? asNonEmptyString(payload.jobId) : null;
      if (!jobId) throw new Error("The factory did not return a creation job.");

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
