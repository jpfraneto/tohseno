const form = document.querySelector("#shot-form");
const imageInput = document.querySelector("#images");
const dropZone = document.querySelector("#drop-zone");
const attachments = document.querySelector("#attachments");
const eventLog = document.querySelector("#events");
const submit = document.querySelector("#submit");
const connection = document.querySelector("#connection");
let files = [];

const renderFiles = () => {
  attachments.replaceChildren(...files.map((file) => {
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

dropZone.addEventListener("click", () => imageInput.click());
dropZone.addEventListener("keydown", (event) => {
  if (event.key === "Enter" || event.key === " ") imageInput.click();
});
imageInput.addEventListener("change", () => acceptFiles(imageInput.files));
for (const name of ["dragenter", "dragover"]) {
  dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    dropZone.classList.add("dragging");
  });
}
for (const name of ["dragleave", "drop"]) {
  dropZone.addEventListener(name, (event) => {
    event.preventDefault();
    dropZone.classList.remove("dragging");
  });
}
dropZone.addEventListener("drop", (event) => acceptFiles(event.dataTransfer.files));

const filePayload = (file) => new Promise((resolve, reject) => {
  const reader = new FileReader();
  reader.onerror = reject;
  reader.onload = () => resolve({ name: file.name, data: reader.result.split(",", 2)[1] });
  reader.readAsDataURL(file);
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  submit.disabled = true;
  submit.textContent = "Sending to press…";
  try {
    const response = await fetch("/shots", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        mode: document.querySelector("#mode").value,
        app_name: document.querySelector("#app-name").value,
        prompt: document.querySelector("#prompt").value,
        images: await Promise.all(files.map(filePayload)),
      }),
    });
    if (!response.ok) throw new Error(await response.text());
    submit.textContent = "Shot in progress";
  } catch (error) {
    appendEvent("status", `intake rejected: ${error.message}`);
    submit.disabled = false;
    submit.textContent = "Print shot";
  }
});

const appendEvent = (kind, message) => {
  eventLog.querySelector(".empty")?.remove();
  const line = document.createElement("p");
  line.className = kind;
  line.textContent = kind === "harness_line" ? `  ${message}` : message;
  eventLog.append(line);
  eventLog.scrollTop = eventLog.scrollHeight;
  if (kind === "result") {
    submit.disabled = false;
    submit.textContent = "Print shot";
  }
  if (kind === "status" && message.startsWith("engine stopped:")) {
    submit.disabled = false;
    submit.textContent = "Print shot";
  }
};

document.querySelector("#clear").addEventListener("click", () => {
  eventLog.replaceChildren();
  appendEvent("status", "The display is clear.");
});

const stream = new EventSource("/events");
stream.onopen = () => {
  connection.textContent = "press connected";
  connection.classList.add("online");
};
stream.onerror = () => {
  connection.textContent = "reconnecting";
  connection.classList.remove("online");
};
stream.onmessage = (event) => {
  const item = JSON.parse(event.data);
  appendEvent(item.kind, item.message);
};
