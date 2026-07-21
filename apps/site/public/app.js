(() => {
  "use strict";

  const copyButton = document.querySelector("[data-copy-command]");
  const commandLine = document.querySelector("[data-oneshot-command]");
  if (copyButton instanceof HTMLButtonElement && commandLine instanceof HTMLElement) {
    // The button is server-rendered hidden because copying requires script.
    // Without script the command is still selectable text.
    if (navigator.clipboard?.writeText) {
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
  }
})();
