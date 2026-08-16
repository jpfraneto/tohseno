const $ = (selector) => document.querySelector(selector);
let apps = [];
let selected = null;

function setStatus(text, attention = false) {
  const element = $("#status");
  element.textContent = text;
  element.dataset.attention = String(attention);
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      "x-tohseno-studio": "1",
      ...(options.headers || {}),
    },
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.error || `Studio request failed (${response.status})`);
  }
  return response.json();
}

async function loadApps() {
  apps = (await api("/api/apps")).apps;
  const list = $("#apps");
  list.replaceChildren();
  for (const app of apps) {
    const button = document.createElement("button");
    button.className = `app${selected?.name === app.name ? " active" : ""}`;
    button.dataset.appName = app.name;
    const name = document.createElement("strong");
    name.textContent = app.name;
    const detail = document.createElement("span");
    if (app.read_only) {
      detail.textContent = `Read-only history${app.latest_version ? ` · Version ${app.latest_version}` : ""}`;
    } else if (app.needs_attention) {
      detail.textContent = "Needs attention";
    } else if (app.latest_version) {
      detail.textContent = `Version ${app.latest_version}${app.has_unrecorded_changes ? " · Changes to record" : ""}`;
    } else {
      detail.textContent = "No Versions yet";
    }
    button.append(name, detail);
    button.addEventListener("click", () => selectApp(app));
    list.append(button);
  }
  if (!apps.length) list.textContent = "No tracked apps.";
}

function selectApp(app) {
  if (!app) return;
  selected = app;
  $("#new-app").hidden = true;
  $("#selected").hidden = false;
  $("#selected-name").textContent = app.name;
  $("#folder").textContent = app.folder;
  if (app.read_only) {
    $("#version-title").textContent = "Read-only history";
  } else if (app.needs_attention) {
    $("#version-title").textContent = "History needs attention";
  } else {
    $("#version-title").textContent = app.latest_version
      ? `Current Version ${app.latest_version}`
      : "No Versions recorded";
  }
  $("#record-version button").disabled = app.read_only || app.needs_attention;
  const versions = $("#versions");
  versions.replaceChildren();
  for (const number of [...app.versions].reverse()) {
    const item = document.createElement("li");
    item.textContent = `Version ${number}`;
    versions.append(item);
  }
  for (const button of document.querySelectorAll(".app")) {
    button.classList.toggle("active", button.dataset.appName === app.name);
  }
}

$("#new-app-button").addEventListener("click", () => {
  $("#selected").hidden = true;
  $("#new-app").hidden = false;
});

$("#new-app").addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    setStatus("Initializing");
    await api("/api/apps", {
      method: "POST",
      body: JSON.stringify({ name: $("#app-name").value.trim().toLowerCase() }),
    });
    $("#app-name").value = "";
    await loadApps();
    setStatus("Ready");
  } catch (error) {
    setStatus(error.message, true);
  }
});

$("#record-version").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!selected) return;
  try {
    setStatus("Recording");
    const result = await api("/api/versions", {
      method: "POST",
      body: JSON.stringify({ app_name: selected.name, note: $("#version-note").value }),
    });
    $("#version-note").value = "";
    await loadApps();
    selectApp(apps.find((app) => app.name === selected.name));
    setStatus(`Recorded Version ${result.version}`);
  } catch (error) {
    setStatus(error.message, true);
  }
});

$("#open-folder").addEventListener("click", async () => {
  if (!selected) return;
  try {
    await api("/api/open", { method: "POST", body: JSON.stringify({ app_name: selected.name }) });
  } catch (error) {
    setStatus(error.message, true);
  }
});

loadApps().catch((error) => setStatus(error.message, true));
