const copyButtons = [...document.querySelectorAll("[data-copy-install]")];
const status = document.querySelector(".copy-status");

async function copyText(text) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const input = document.createElement("textarea");
  input.value = text;
  input.setAttribute("readonly", "");
  input.className = "copy-fallback";
  document.body.append(input);
  input.select();
  const copied = document.execCommand("copy");
  input.remove();
  if (!copied) throw new Error("Copy failed");
}

function resetButton(button) {
  const command = button.dataset.installCommand;
  button.querySelector("[data-install-text]").textContent = command;
  button.querySelector("[data-copy-label]").textContent = "Copy";
}

for (const button of copyButtons) {
  let resetTimer;
  button.addEventListener("click", async () => {
    const command = button.dataset.installCommand;
    try {
      await copyText(command);
      window.clearTimeout(resetTimer);
      button.querySelector("[data-install-text]").textContent =
        "Copied — paste into Terminal";
      button.querySelector("[data-copy-label]").textContent = "✓";
      status.textContent = "Install command copied.";
      resetTimer = window.setTimeout(() => resetButton(button), 1800);
    } catch {
      status.textContent = "Could not copy automatically. Select the command and copy it.";
      const commandNode = button.querySelector("[data-install-text]");
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(commandNode);
      selection.removeAllRanges();
      selection.addRange(range);
    }
  });
}
