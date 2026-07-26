(() => {
  "use strict";

  // Buttons are server-rendered hidden because copying requires script.
  // Without script the command is still selectable text.
  if (navigator.clipboard?.writeText) {
    for (const copyButton of document.querySelectorAll("[data-copy-command]")) {
      if (!(copyButton instanceof HTMLButtonElement)) continue;
      const commandLine = copyButton
        .closest("[role='group'], .command-block")
        ?.querySelector("[data-install-command]");
      if (!(commandLine instanceof HTMLElement)) continue;
      copyButton.hidden = false;
      copyButton.setAttribute("aria-live", "polite");
      const idleLabel = copyButton.textContent?.trim() || "Copy";
      let resetTimer = 0;
      copyButton.addEventListener("click", async () => {
        try {
          const copyValue =
            commandLine.dataset.copyValue ||
            commandLine.textContent?.trim() ||
            "";
          await navigator.clipboard.writeText(copyValue);
          copyButton.textContent = copyButton.dataset.copiedLabel || "Copied";
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

  const shotField = document.querySelector("[data-shot-field]");
  const shotToggle = document.querySelector("[data-shot-toggle]");
  if (
    shotField instanceof HTMLOListElement &&
    shotToggle instanceof HTMLButtonElement
  ) {
    const collapsedLabel = shotToggle.querySelector("span");
    const direction = shotToggle.querySelector("span:last-child");
    const idleLabel =
      collapsedLabel?.textContent || "Show the full contact sheet";
    shotField.dataset.ready = "true";
    shotToggle.hidden = false;
    shotToggle.addEventListener("click", () => {
      const expanded = shotToggle.getAttribute("aria-expanded") !== "true";
      shotToggle.setAttribute("aria-expanded", String(expanded));
      shotField.dataset.expanded = String(expanded);
      if (collapsedLabel) {
        collapsedLabel.textContent = expanded
          ? shotToggle.dataset.expandedLabel || "Fold the contact sheet"
          : idleLabel;
      }
      if (direction) direction.textContent = expanded ? "↑" : "↓";
    });
  }
})();
