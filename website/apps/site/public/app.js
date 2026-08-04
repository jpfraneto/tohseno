const WINDOW_DAYS = 30;
const RELEASE_HOUR = 8;

const arrows = document.querySelector("#arrows");
const quiverRange = document.querySelector("#quiver-range");
const releaseCountdown = document.querySelector("#release-countdown");
const copyInstaller = document.querySelector("#copy-installer");
const installerCommand = document.querySelector("#installer-command");
const dateFormat = new Intl.DateTimeFormat("en", {
  month: "short",
  day: "numeric",
});

function renderQuiver() {
  const now = new Date();
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const fragment = document.createDocumentFragment();

  for (let index = 0; index < WINDOW_DAYS; index += 1) {
    const arrowDate = new Date(start);
    arrowDate.setDate(start.getDate() + index);
    const arrow = document.createElement("span");
    arrow.className = `arrow${index === 0 ? " today" : ""}`;
    fragment.append(arrow);
  }

  arrows.replaceChildren(fragment);
  const end = new Date(start);
  end.setDate(start.getDate() + WINDOW_DAYS - 1);
  quiverRange.textContent = `${dateFormat.format(start)} — ${dateFormat.format(end)} · ONE ARROW PER DAY`.toUpperCase();
}

function renderCountdown() {
  const now = new Date();
  const next = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
    RELEASE_HOUR,
  );
  if (next <= now) next.setDate(next.getDate() + 1);
  const remaining = next.getTime() - now.getTime();
  const hours = Math.floor(remaining / 3_600_000);
  const minutes = Math.floor((remaining % 3_600_000) / 60_000);
  releaseCountdown.textContent = `next arrow in ${hours}h ${String(minutes).padStart(2, "0")}m`;
}

async function copyInstallCommand() {
  await navigator.clipboard.writeText(installerCommand.textContent.trim());
  copyInstaller.textContent = "COPIED";
  window.setTimeout(() => {
    copyInstaller.textContent = "COPY";
  }, 1600);
}

copyInstaller.addEventListener("click", () => {
  copyInstallCommand().catch(() => {
    copyInstaller.textContent = "SELECT + COPY";
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(installerCommand);
    selection.removeAllRanges();
    selection.addRange(range);
  });
});

renderQuiver();
renderCountdown();
window.setInterval(renderCountdown, 30_000);
window.setInterval(renderQuiver, 60_000);
