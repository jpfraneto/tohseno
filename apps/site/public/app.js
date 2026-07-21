(() => {
  "use strict";

  // Buttons are server-rendered hidden because copying requires script.
  // Without script the command is still selectable text.
  if (!navigator.clipboard?.writeText) return;
  for (const copyButton of document.querySelectorAll("[data-copy-command]")) {
    if (!(copyButton instanceof HTMLButtonElement)) continue;
    const commandLine = copyButton.closest("[role='group'], .command-block")?.querySelector("[data-oneshot-command]");
    if (!(commandLine instanceof HTMLElement)) continue;
    copyButton.hidden = false;
    const idleLabel = copyButton.textContent;
    let resetTimer = 0;
    copyButton.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(commandLine.textContent?.trim() ?? "");
        copyButton.textContent = copyButton.dataset.copiedLabel || idleLabel;
        copyButton.dataset.copied = "true";
        window.clearTimeout(resetTimer);
        resetTimer = window.setTimeout(() => {
          copyButton.textContent = idleLabel;
          delete copyButton.dataset.copied;
        }, 2000);
      } catch {
        // Clipboard permission was refused; the command remains selectable.
      }
    });
  }
})();
