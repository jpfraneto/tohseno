const copyButton = document.getElementById("copy-command");
const copyStatus = document.getElementById("copy-status");
let resetCopy;
copyButton?.addEventListener("click", async () => {
  clearTimeout(resetCopy);
  try {
    await navigator.clipboard.writeText(document.getElementById("install-command").textContent);
    copyButton.textContent = "Copied";
    copyStatus.textContent = "Command copied.";
    resetCopy = setTimeout(() => { copyButton.textContent = "Copy command"; copyStatus.textContent = ""; }, 2000);
  } catch {
    copyButton.textContent = "Copy command";
    copyStatus.textContent = "Select and copy the command above.";
  }
});
for (const icon of document.querySelectorAll("[data-app-icon]")) {
  icon.addEventListener("error", () => { icon.src = "/menlo/app.svg"; }, { once: true });
}
