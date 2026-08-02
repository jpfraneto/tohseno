(() => {
  "use strict";

  // Earlier releases registered a service worker for the mobile Shot
  // composer; unregister it so returning visitors load the plain page.
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.getRegistrations().then(
      (registrations) => {
        for (const registration of registrations) registration.unregister();
      },
      () => {},
    );
  }

  // Buttons are server-rendered hidden because copying requires script.
  // Without script the commands are still selectable text.
  if (navigator.clipboard?.writeText) {
    for (const copyButton of document.querySelectorAll("[data-copy-command]")) {
      if (!(copyButton instanceof HTMLButtonElement)) continue;
      const commandLine = copyButton
        .closest(".term-row, .command-block, [role='group']")
        ?.querySelector("[data-copy-value]");
      if (!(commandLine instanceof HTMLElement)) continue;
      copyButton.hidden = false;
      copyButton.setAttribute("aria-live", "polite");
      const idleLabel = copyButton.textContent?.trim() || "copy";
      let resetTimer = 0;
      const copyCommand = async () => {
        try {
          await navigator.clipboard.writeText(
            commandLine.dataset.copyValue ||
              commandLine.textContent?.trim() ||
              "",
          );
          copyButton.textContent = copyButton.dataset.copiedLabel || "copied";
          copyButton.dataset.copied = "true";
          window.clearTimeout(resetTimer);
          resetTimer = window.setTimeout(() => {
            copyButton.textContent = idleLabel;
            delete copyButton.dataset.copied;
          }, 2000);
        } catch {
          // Clipboard permission was refused; the command remains selectable.
        }
      };
      copyButton.addEventListener("click", copyCommand);
      commandLine.classList.add("is-copyable");
      commandLine.addEventListener("click", () => {
        // A click that ends a text selection is not a copy request.
        if (window.getSelection()?.toString()) return;
        copyCommand();
      });
    }
  }

  // Type the three commands one after another. Every line is server-rendered
  // visible, so without script (or with reduced motion) the page is static.
  const terminal = document.querySelector("[data-terminal]");
  if (
    terminal instanceof HTMLElement &&
    !window.matchMedia("(prefers-reduced-motion: reduce)").matches
  ) {
    const steps = [...terminal.querySelectorAll("[data-term-step]")];
    if (steps.length > 0) {
      terminal.dataset.typing = "true";
      const wait = (milliseconds) =>
        new Promise((resolve) => window.setTimeout(resolve, milliseconds));
      (async () => {
        for (const step of steps) {
          step.dataset.visible = "true";
          const commandText = step.querySelector("[data-command-text]");
          if (commandText instanceof HTMLElement) {
            const command = commandText.textContent ?? "";
            commandText.textContent = "";
            step.dataset.active = "true";
            for (const character of command) {
              commandText.textContent += character;
              await wait(18);
            }
            delete step.dataset.active;
            await wait(500);
          } else {
            await wait(350);
          }
        }
        delete terminal.dataset.typing;
      })();
    }
  }
})();
