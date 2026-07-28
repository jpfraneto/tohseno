const ui = {
  connection: document.querySelector("#connection"),
  newShot: document.querySelector("#new-shot"),
  emptySelection: document.querySelector("#empty-selection"),
  selection: document.querySelector("#selection"),
  selectedIcon: document.querySelector("#selected-icon"),
  selectedName: document.querySelector("#selected-name"),
  selectedLocation: document.querySelector("#selected-location"),
  previousShot: document.querySelector("#previous-shot"),
  nextShot: document.querySelector("#next-shot"),
  shotPosition: document.querySelector("#shot-position"),
  evolve: document.querySelector("#evolve"),
  openSimulator: document.querySelector("#open-simulator"),
  backHome: document.querySelector("#back-home"),
  slotLabel: document.querySelector("#slot-label"),
  slotDots: document.querySelector("#slot-dots"),
  appGrid: document.querySelector("#app-grid"),
  noApps: document.querySelector("#no-apps"),
  libraryHome: document.querySelector("#library-home"),
  runningApp: document.querySelector("#running-app"),
  statusBar: document.querySelector("#status-bar"),
  homeIndicator: document.querySelector("#home-indicator"),
  simulatorLoading: document.querySelector("#simulator-loading"),
  simulatorScreen: document.querySelector("#simulator-screen"),
  eventLog: document.querySelector("#events"),
  composer: document.querySelector("#composer"),
  composerKicker: document.querySelector("#composer-kicker"),
  composerTitle: document.querySelector("#composer-title"),
  closeComposer: document.querySelector("#close-composer"),
  form: document.querySelector("#shot-form"),
  appNameLabel: document.querySelector("#app-name-label"),
  appName: document.querySelector("#app-name"),
  promptLabel: document.querySelector("#prompt-label"),
  prompt: document.querySelector("#prompt"),
  imageInput: document.querySelector("#images"),
  dropZone: document.querySelector("#drop-zone"),
  attachments: document.querySelector("#attachments"),
  submit: document.querySelector("#submit"),
};

let library = { apps: [], iphone_slots_used: 0, iphone_slot_limit: 3 };
let selectedApp = null;
let selectedShot = null;
let composerMode = "create";
let files = [];
let screenshotTimer = null;
let pendingShot = null;

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
  renderLibrary();
  renderSlots();
};

const renderLibrary = () => {
  const tiles = library.apps.map((app) => {
    const tile = document.createElement("button");
    tile.type = "button";
    tile.className = "app-tile";
    tile.setAttribute("aria-label", `Open ${app.name}, shot ${app.latest_shot}`);
    const name = document.createElement("span");
    name.className = "app-name";
    name.textContent = app.name;
    tile.append(icon(app, app.latest_shot), name);
    tile.addEventListener("click", () => selectApp(app, app.latest_shot));
    return tile;
  });
  ui.appGrid.replaceChildren(...tiles);
  ui.noApps.hidden = library.apps.length > 0;
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
    ui.emptySelection.hidden = false;
    ui.selection.hidden = true;
    return;
  }
  const index = selectedApp.shots.indexOf(selectedShot);
  ui.emptySelection.hidden = true;
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
  renderSelection();
  ui.libraryHome.hidden = true;
  ui.runningApp.hidden = false;
  ui.statusBar.hidden = true;
  ui.homeIndicator.hidden = true;
  ui.simulatorLoading.hidden = false;
  ui.simulatorLoading.querySelector("strong").textContent = "Opening shot…";
  ui.simulatorLoading.querySelector("span").textContent = "Building for Simulator";
  ui.simulatorScreen.removeAttribute("src");
  stopScreenshots();
  try {
    const response = await fetch("/api/simulator/launch", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ app_name: app.name, shot }),
    });
    if (!response.ok) throw new Error(await response.text());
    refreshScreenshot();
    screenshotTimer = setInterval(refreshScreenshot, 850);
  } catch (error) {
    appendEvent("status", `Simulator stopped: ${error.message}`);
    ui.simulatorLoading.querySelector("strong").textContent = "Could not open shot";
    ui.simulatorLoading.querySelector("span").textContent = "Read the live press for details";
  }
};

const refreshScreenshot = () => {
  const image = new Image();
  image.onload = () => {
    ui.simulatorScreen.src = image.src;
    ui.simulatorLoading.hidden = true;
  };
  image.src = `/api/simulator/screen?t=${Date.now()}`;
};

const stopScreenshots = () => {
  if (screenshotTimer) clearInterval(screenshotTimer);
  screenshotTimer = null;
};

const showHome = () => {
  stopScreenshots();
  selectedApp = null;
  selectedShot = null;
  renderSelection();
  ui.runningApp.hidden = true;
  ui.libraryHome.hidden = false;
  ui.statusBar.hidden = false;
  ui.homeIndicator.hidden = false;
};

ui.previousShot.addEventListener("click", () => {
  const index = selectedApp.shots.indexOf(selectedShot);
  if (index > 0) selectApp(selectedApp, selectedApp.shots[index - 1]);
});

ui.nextShot.addEventListener("click", () => {
  const index = selectedApp.shots.indexOf(selectedShot);
  if (index < selectedApp.shots.length - 1) selectApp(selectedApp, selectedApp.shots[index + 1]);
});

ui.backHome.addEventListener("click", showHome);

ui.openSimulator.addEventListener("click", async () => {
  await fetch("/api/simulator/focus", { method: "POST" });
});

const openComposer = (mode) => {
  composerMode = mode;
  files = [];
  renderFiles();
  ui.form.reset();
  if (mode === "create") {
    ui.composerKicker.textContent = "CREATE";
    ui.composerTitle.textContent = "New shot";
    ui.appNameLabel.hidden = false;
    ui.appName.required = true;
    ui.promptLabel.textContent = "Describe the app";
    ui.submit.textContent = "Create app";
  } else {
    ui.composerKicker.textContent = `SHOT ${selectedApp.latest_shot + 1}`;
    ui.composerTitle.textContent = selectedApp.name;
    ui.appNameLabel.hidden = true;
    ui.appName.required = false;
    ui.appName.value = selectedApp.name;
    ui.promptLabel.textContent = "What should change?";
    ui.submit.textContent = "Evolve app";
  }
  ui.composer.showModal();
  setTimeout(() => (mode === "create" ? ui.appName : ui.prompt).focus(), 0);
};

ui.newShot.addEventListener("click", () => openComposer("create"));
ui.evolve.addEventListener("click", () => openComposer("evolve"));
ui.closeComposer.addEventListener("click", () => ui.composer.close());

const renderFiles = () => {
  ui.attachments.replaceChildren(...files.map((file) => {
    const chip = document.createElement("span");
    chip.className = "attachment";
    chip.textContent = file.name;
    return chip;
  }));
};

const acceptFiles = (incoming) => {
  const supported = /\.(png|jpe?g|heic|webp)$/i;
  files = [...files, ...Array.from(incoming).filter((file) => supported.test(file.name))].slice(0, 8);
  renderFiles();
};

ui.dropZone.addEventListener("click", () => ui.imageInput.click());
ui.dropZone.addEventListener("keydown", (event) => {
  if (event.key === "Enter" || event.key === " ") ui.imageInput.click();
});
ui.imageInput.addEventListener("change", () => acceptFiles(ui.imageInput.files));
for (const name of ["dragenter", "dragover"]) {
  ui.dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    ui.dropZone.classList.add("dragging");
  });
}
for (const name of ["dragleave", "drop"]) {
  ui.dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    ui.dropZone.classList.remove("dragging");
  });
}
ui.dropZone.addEventListener("drop", (event) => acceptFiles(event.dataTransfer.files));

const filePayload = (file) => new Promise((resolve, reject) => {
  const reader = new FileReader();
  reader.onerror = reject;
  reader.onload = () => resolve({ name: file.name, data: reader.result.split(",", 2)[1] });
  reader.readAsDataURL(file);
});

ui.form.addEventListener("submit", async (event) => {
  event.preventDefault();
  ui.submit.disabled = true;
  const appName = composerMode === "create" ? ui.appName.value : selectedApp.name;
  pendingShot = { mode: composerMode, appName };
  try {
    const response = await fetch("/shots", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        mode: composerMode,
        app_name: appName,
        prompt: ui.prompt.value,
        images: await Promise.all(files.map(filePayload)),
      }),
    });
    if (!response.ok) throw new Error(await response.text());
    ui.composer.close();
  } catch (error) {
    appendEvent("status", `intake rejected: ${error.message}`);
    pendingShot = null;
  } finally {
    ui.submit.disabled = false;
  }
});

const appendEvent = (kind, message) => {
  ui.eventLog.querySelector(".empty")?.remove();
  const line = document.createElement("p");
  line.className = kind;
  line.textContent = kind === "harness_line" ? `  ${message}` : message;
  ui.eventLog.append(line);
  ui.eventLog.scrollTop = ui.eventLog.scrollHeight;
  if (kind === "result" && pendingShot) {
    const completed = pendingShot;
    pendingShot = null;
    loadLibrary().then(() => {
      if (completed.mode === "evolve") {
        const app = library.apps.find((candidate) => candidate.name === completed.appName);
        if (app) selectApp(app, app.latest_shot);
      } else {
        showHome();
      }
    });
  }
  if (kind === "status" && message.startsWith("engine stopped:")) {
    pendingShot = null;
  }
};

document.querySelector("#clear").addEventListener("click", () => {
  ui.eventLog.replaceChildren();
  appendEvent("status", "The display is clear.");
});

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

loadLibrary().catch((error) => appendEvent("status", `library unavailable: ${error.message}`));
