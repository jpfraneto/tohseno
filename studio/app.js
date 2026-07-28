const ui = {
  studio: document.querySelector(".studio"),
  connection: document.querySelector("#connection"),
  newShot: document.querySelector("#new-shot"),
  appCount: document.querySelector("#app-count"),
  appGrid: document.querySelector("#app-grid"),
  noApps: document.querySelector("#no-apps"),
  selection: document.querySelector("#selection"),
  selectedIcon: document.querySelector("#selected-icon"),
  selectedName: document.querySelector("#selected-name"),
  selectedLocation: document.querySelector("#selected-location"),
  previousShot: document.querySelector("#previous-shot"),
  nextShot: document.querySelector("#next-shot"),
  shotPosition: document.querySelector("#shot-position"),
  evolve: document.querySelector("#evolve"),
  openSimulator: document.querySelector("#open-simulator"),
  slotLabel: document.querySelector("#slot-label"),
  slotDots: document.querySelector("#slot-dots"),
  latestEvent: document.querySelector("#latest-event"),
  simulatorTitle: document.querySelector("#simulator-title"),
  simulatorEmpty: document.querySelector("#simulator-empty"),
  runningApp: document.querySelector("#running-app"),
  simulatorLoading: document.querySelector("#simulator-loading"),
  simulatorScreen: document.querySelector("#simulator-screen"),
  showLibrary: document.querySelector("#show-library"),
  eventLog: document.querySelector("#events"),
  composer: document.querySelector("#composer"),
  composerSplitter: document.querySelector("#composer-splitter"),
  composerKicker: document.querySelector("#composer-kicker"),
  composerTitle: document.querySelector("#composer-title"),
  composerSupport: document.querySelector("#composer-support"),
  closeComposer: document.querySelector("#close-composer"),
  form: document.querySelector("#shot-form"),
  harness: document.querySelector("#harness"),
  harnessStatus: document.querySelector("#harness-status"),
  appNameLabel: document.querySelector("#app-name-label"),
  appName: document.querySelector("#app-name"),
  promptLabel: document.querySelector("#prompt-label"),
  prompt: document.querySelector("#prompt"),
  imageInput: document.querySelector("#images"),
  dropZone: document.querySelector("#drop-zone"),
  attachments: document.querySelector("#attachments"),
  submit: document.querySelector("#submit"),
  librarySplitter: document.querySelector("#library-splitter"),
};

let library = { apps: [], iphone_slots_used: 0, iphone_slot_limit: 3 };
let harnesses = [];
let selectedApp = null;
let selectedShot = null;
let composerMode = "create";
let composerAppName = null;
let files = [];
let screenshotTimer = null;
let pendingShot = null;
let pressActive = false;
let shotCompleted = false;
let launchSequence = 0;

const escapeInitial = (name) => (name.trim()[0] || "T").toUpperCase();

const icon = (app, shot, className = "app-icon") => {
  const frame = document.createElement("span");
  frame.className = className;

  const fallback = document.createElement("span");
  fallback.className = "icon-fallback";
  fallback.textContent = escapeInitial(app.name);

  const image = document.createElement("img");
  image.alt = "";
  image.src = `/api/icon/${app.name}/${shot}`;
  image.addEventListener("error", () => image.remove());
  frame.append(fallback, image);

  if (app.retired && className === "app-icon") {
    const badge = document.createElement("span");
    badge.className = "local-badge";
    badge.title = "Saved locally, not installed on iPhone";
    frame.append(badge);
  }
  return frame;
};

const loadLibrary = async () => {
  const response = await fetch("/api/apps", { cache: "no-store" });
  if (!response.ok) throw new Error(await response.text());
  library = await response.json();

  if (selectedApp) {
    selectedApp = library.apps.find((app) => app.name === selectedApp.name) || null;
    if (selectedApp && !selectedApp.shots.includes(selectedShot)) {
      selectedShot = selectedApp.latest_shot;
    }
  }
  renderLibrary();
  renderSelection();
  renderSlots();
};

const loadHarnesses = async () => {
  const response = await fetch("/api/harnesses", { cache: "no-store" });
  if (!response.ok) throw new Error(await response.text());
  const payload = await response.json();
  harnesses = payload.harnesses.filter((harness) => harness.installed);
  renderHarnesses();
};

const renderHarnesses = () => {
  const selected = harnesses.find((harness) => harness.selected) || harnesses[0];
  if (!selected) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "No coding agents found";
    ui.harness.replaceChildren(option);
    ui.harness.disabled = true;
    ui.harnessStatus.textContent = "Install a supported coding agent to take a Shot.";
    updateSubmitState();
    return;
  }

  const options = harnesses.map((harness) => {
    const option = document.createElement("option");
    option.value = harness.id;
    option.textContent = harness.label;
    option.selected = harness.id === selected.id;
    return option;
  });
  ui.harness.replaceChildren(...options);
  ui.harness.disabled = pressActive;
  ui.harnessStatus.textContent = harnesses.length === 1
    ? "1 agent detected on this Mac."
    : `${harnesses.length} agents detected on this Mac.`;
  updateSubmitState();
};

const renderLibrary = () => {
  const tiles = library.apps.map((app) => {
    const tile = document.createElement("button");
    tile.type = "button";
    tile.className = "app-tile";
    if (selectedApp?.name === app.name) tile.classList.add("selected");
    tile.setAttribute("aria-label", `Run ${app.name}, Shot ${app.latest_shot}`);
    tile.setAttribute("aria-pressed", selectedApp?.name === app.name ? "true" : "false");

    const name = document.createElement("strong");
    name.className = "app-name";
    name.textContent = app.name;

    const meta = document.createElement("span");
    meta.className = "app-meta";
    meta.textContent = app.shots.length === 1 ? "1 Shot" : `${app.shots.length} Shots`;

    tile.append(icon(app, app.latest_shot), name, meta);
    tile.addEventListener("click", () => selectApp(app, app.latest_shot));
    return tile;
  });

  ui.appGrid.replaceChildren(...tiles);
  ui.noApps.hidden = library.apps.length > 0;
  ui.appCount.textContent = library.apps.length === 1 ? "1 app" : `${library.apps.length} apps`;
};

const renderSlots = () => {
  ui.slotLabel.textContent = `${library.iphone_slots_used} of ${library.iphone_slot_limit} installed`;
  ui.slotDots.replaceChildren(...Array.from({ length: library.iphone_slot_limit }, (_, index) => {
    const dot = document.createElement("span");
    dot.className = `slot-dot${index < library.iphone_slots_used ? " used" : ""}`;
    return dot;
  }));
};

const renderSelection = () => {
  if (!selectedApp) {
    ui.selection.hidden = true;
    return;
  }

  const index = selectedApp.shots.indexOf(selectedShot);
  ui.selection.hidden = false;
  ui.selectedIcon.replaceChildren(...icon(selectedApp, selectedShot, "selected-icon").childNodes);
  ui.selectedName.textContent = selectedApp.name;
  ui.selectedLocation.textContent = selectedApp.retired ? "Local library" : "Installed on iPhone";
  ui.shotPosition.textContent = `Shot ${selectedShot} of ${selectedApp.latest_shot}`;
  ui.previousShot.disabled = index <= 0;
  ui.nextShot.disabled = index < 0 || index >= selectedApp.shots.length - 1;
};

const selectApp = async (app, shot) => {
  selectedApp = app;
  selectedShot = shot;
  renderLibrary();
  renderSelection();

  ui.simulatorEmpty.hidden = true;
  ui.runningApp.hidden = false;
  ui.showLibrary.hidden = false;
  ui.simulatorTitle.textContent = `${app.name} · Shot ${shot}`;
  ui.simulatorLoading.hidden = false;
  ui.simulatorLoading.querySelector("strong").textContent = "Opening Shot…";
  ui.simulatorLoading.querySelector("span").textContent = "Building for Simulator";
  ui.simulatorScreen.removeAttribute("src");

  stopScreenshots();
  const sequence = ++launchSequence;
  try {
    const response = await fetch("/api/simulator/launch", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ app_name: app.name, shot }),
    });
    if (!response.ok) throw new Error(await response.text());
    if (sequence !== launchSequence) return;
    refreshScreenshot(sequence);
    screenshotTimer = setInterval(() => refreshScreenshot(sequence), 850);
  } catch (error) {
    if (sequence !== launchSequence) return;
    appendEvent("status", `Simulator stopped: ${error.message}`);
    ui.simulatorLoading.querySelector("strong").textContent = "Could not open Shot";
    ui.simulatorLoading.querySelector("span").textContent = "Read the live press for details";
  }
};

const refreshScreenshot = (sequence) => {
  const image = new Image();
  image.onload = () => {
    if (sequence !== launchSequence) return;
    ui.simulatorScreen.src = image.src;
    ui.simulatorLoading.hidden = true;
  };
  image.src = `/api/simulator/screen?t=${Date.now()}`;
};

const stopScreenshots = () => {
  if (screenshotTimer) clearInterval(screenshotTimer);
  screenshotTimer = null;
};

const showSimulatorEmpty = () => {
  stopScreenshots();
  launchSequence += 1;
  selectedApp = null;
  selectedShot = null;
  renderLibrary();
  renderSelection();
  ui.runningApp.hidden = true;
  ui.simulatorEmpty.hidden = false;
  ui.showLibrary.hidden = true;
  ui.simulatorTitle.textContent = "No Shot selected";
};

ui.previousShot.addEventListener("click", () => {
  const index = selectedApp.shots.indexOf(selectedShot);
  if (index > 0) selectApp(selectedApp, selectedApp.shots[index - 1]);
});

ui.nextShot.addEventListener("click", () => {
  const index = selectedApp.shots.indexOf(selectedShot);
  if (index < selectedApp.shots.length - 1) selectApp(selectedApp, selectedApp.shots[index + 1]);
});

ui.showLibrary.addEventListener("click", showSimulatorEmpty);

ui.openSimulator.addEventListener("click", async () => {
  await fetch("/api/simulator/focus", { method: "POST" });
});

const updateSubmitState = () => {
  const validName = composerMode === "evolve" || (ui.appName.validity.valid && ui.appName.value.length > 0);
  const ready = validName
    && ui.harness.value.length > 0
    && ui.prompt.value.trim().length > 0
    && !pressActive
    && !shotCompleted;
  ui.submit.disabled = !ready;
};

const setComposerBusy = (busy) => {
  pressActive = busy;
  ui.form.setAttribute("aria-busy", String(busy));
  ui.harness.disabled = busy || harnesses.length === 0;
  ui.appName.disabled = busy;
  ui.prompt.disabled = busy;
  ui.imageInput.disabled = busy;
  ui.dropZone.setAttribute("aria-disabled", String(busy));
  ui.submit.textContent = busy ? "Taking Shot…" : "Take Shot";
  updateSubmitState();
};

const openComposer = (mode) => {
  composerMode = mode;
  composerAppName = mode === "evolve" ? selectedApp.name : null;
  files = [];
  shotCompleted = false;
  renderFiles();
  ui.form.reset();
  const selectedHarness = harnesses.find((harness) => harness.selected) || harnesses[0];
  ui.harness.value = selectedHarness?.id || "";

  if (mode === "create") {
    ui.composerKicker.textContent = "CREATE";
    ui.composerTitle.textContent = "New Shot";
    ui.composerSupport.textContent = "Make the app no company would.";
    ui.appNameLabel.hidden = false;
    ui.appName.required = true;
    ui.promptLabel.textContent = "Make the intention exact";
  } else {
    ui.composerKicker.textContent = `SHOT ${selectedApp.latest_shot + 1}`;
    ui.composerTitle.textContent = `Evolve ${composerAppName}`;
    ui.composerSupport.textContent = "Use teaches the next evolution.";
    ui.appNameLabel.hidden = true;
    ui.appName.required = false;
    ui.appName.value = composerAppName;
    ui.promptLabel.textContent = "What should change?";
  }

  ui.composer.hidden = false;
  ui.composerSplitter.hidden = false;
  ui.studio.classList.add("composer-open");
  setComposerBusy(false);
  setTimeout(() => (mode === "create" ? ui.appName : ui.prompt).focus(), 0);
};

const closeComposer = () => {
  ui.composer.hidden = true;
  ui.composerSplitter.hidden = true;
  ui.studio.classList.remove("composer-open");
};

ui.newShot.addEventListener("click", () => openComposer("create"));
ui.evolve.addEventListener("click", () => openComposer("evolve"));
ui.closeComposer.addEventListener("click", closeComposer);

const renderFiles = () => {
  ui.attachments.replaceChildren(...files.map((file) => {
    const chip = document.createElement("span");
    chip.className = "attachment";
    chip.textContent = file.name;
    return chip;
  }));
};

const acceptFiles = (incoming) => {
  if (pressActive) return;
  const supported = /\.(png|jpe?g|heic|webp)$/i;
  const accepted = Array.from(incoming).filter((file) => supported.test(file.name));
  const available = 8 - files.length;
  files = [...files, ...accepted.slice(0, available)];
  if (accepted.length > available) {
    appendEvent("status", "Eight reference images are attached; the rest were ignored.");
  }
  renderFiles();
};

ui.dropZone.addEventListener("click", () => {
  if (!pressActive) ui.imageInput.click();
});
ui.dropZone.addEventListener("keydown", (event) => {
  if (!pressActive && (event.key === "Enter" || event.key === " ")) ui.imageInput.click();
});
ui.imageInput.addEventListener("change", () => acceptFiles(ui.imageInput.files));
for (const name of ["dragenter", "dragover"]) {
  ui.dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    if (!pressActive) ui.dropZone.classList.add("dragging");
  });
}
for (const name of ["dragleave", "drop"]) {
  ui.dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    ui.dropZone.classList.remove("dragging");
  });
}
ui.dropZone.addEventListener("drop", (event) => acceptFiles(event.dataTransfer.files));

for (const field of [ui.harness, ui.appName, ui.prompt]) {
  field.addEventListener("input", () => {
    shotCompleted = false;
    updateSubmitState();
  });
}

const filePayload = (file) => new Promise((resolve, reject) => {
  const reader = new FileReader();
  reader.onerror = reject;
  reader.onload = () => resolve({ name: file.name, data: reader.result.split(",", 2)[1] });
  reader.readAsDataURL(file);
});

ui.form.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (ui.submit.disabled) return;

  const appName = composerMode === "create" ? ui.appName.value : composerAppName;
  pendingShot = { mode: composerMode, appName };
  setComposerBusy(true);
  try {
    const response = await fetch("/shots", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        mode: composerMode,
        app_name: appName,
        harness: ui.harness.value,
        prompt: ui.prompt.value,
        images: await Promise.all(files.map(filePayload)),
      }),
    });
    if (!response.ok) throw new Error(await response.text());
  } catch (error) {
    appendEvent("status", `intake rejected: ${error.message}`);
    pendingShot = null;
    setComposerBusy(false);
  }
});

const appendEvent = (kind, message) => {
  ui.latestEvent.textContent = message;
  ui.eventLog.querySelector(".empty")?.remove();

  const line = document.createElement("p");
  line.className = kind;
  line.textContent = kind === "harness_line" ? `  ${message}` : message;
  ui.eventLog.append(line);

  if (kind === "result" && pendingShot) {
    const completed = pendingShot;
    pendingShot = null;
    pressActive = false;
    shotCompleted = true;
    ui.form.setAttribute("aria-busy", "false");
    ui.harness.disabled = harnesses.length === 0;
    ui.appName.disabled = false;
    ui.prompt.disabled = false;
    ui.imageInput.disabled = false;
    ui.dropZone.setAttribute("aria-disabled", "false");
    ui.submit.textContent = "Shot complete";
    ui.submit.disabled = true;
    for (const harness of harnesses) {
      harness.selected = harness.id === ui.harness.value;
    }

    loadLibrary().then(() => {
      const app = library.apps.find((candidate) => candidate.name === completed.appName);
      if (app) selectApp(app, app.latest_shot);
    });
  }

  if (kind === "status" && message.startsWith("engine stopped:")) {
    pendingShot = null;
    setComposerBusy(false);
  }
};

document.querySelector("#clear").addEventListener("click", () => {
  ui.eventLog.replaceChildren();
  appendEvent("status", "The display is clear.");
});

const clamp = (value, minimum, maximum) => Math.min(Math.max(value, minimum), maximum);

const configureSplitter = (splitter, property, minimum, maximum, storageKey) => {
  const stored = Number.parseInt(localStorage.getItem(storageKey), 10);
  if (Number.isFinite(stored)) {
    document.documentElement.style.setProperty(property, `${clamp(stored, minimum, maximum)}px`);
  }

  const currentWidth = () => Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(property));
  const setWidth = (width) => {
    const next = clamp(width, minimum, maximum);
    document.documentElement.style.setProperty(property, `${next}px`);
    splitter.setAttribute("aria-valuenow", String(Math.round(next)));
    localStorage.setItem(storageKey, String(Math.round(next)));
  };
  setWidth(currentWidth());

  splitter.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = currentWidth();
    splitter.setPointerCapture(event.pointerId);
    document.body.classList.add("resizing");

    const move = (moveEvent) => setWidth(startWidth + moveEvent.clientX - startX);
    const finish = () => {
      document.body.classList.remove("resizing");
      splitter.removeEventListener("pointermove", move);
      splitter.removeEventListener("pointerup", finish);
      splitter.removeEventListener("pointercancel", finish);
    };
    splitter.addEventListener("pointermove", move);
    splitter.addEventListener("pointerup", finish);
    splitter.addEventListener("pointercancel", finish);
  });

  splitter.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    event.preventDefault();
    setWidth(currentWidth() + (event.key === "ArrowRight" ? 16 : -16));
  });
};

configureSplitter(ui.librarySplitter, "--library-width", 240, 520, "tohseno-library-width");
configureSplitter(ui.composerSplitter, "--composer-width", 320, 640, "tohseno-composer-width");

const stream = new EventSource("/events");
stream.onopen = () => {
  ui.connection.textContent = "press connected";
  ui.connection.classList.add("online");
};
stream.onerror = () => {
  ui.connection.textContent = "reconnecting";
  ui.connection.classList.remove("online");
};
stream.onmessage = (event) => {
  const item = JSON.parse(event.data);
  appendEvent(item.kind, item.message);
};

Promise.all([loadLibrary(), loadHarnesses()]).catch((error) => {
  appendEvent("status", `studio data unavailable: ${error.message}`);
});
