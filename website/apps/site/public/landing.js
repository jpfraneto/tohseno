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
  button.querySelector("[data-copy-label]").textContent = "COPY";
}

for (const button of copyButtons) {
  let resetTimer;
  button.addEventListener("click", async () => {
    const command = button.dataset.installCommand;
    try {
      await copyText(command);
      window.clearTimeout(resetTimer);
      button.querySelector("[data-install-text]").textContent =
        "COPIED. TAKE YOUR SHOT.";
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

const contractButton = document.querySelector("[data-copy-contract]");

if (contractButton) {
  const address = contractButton.dataset.contract;
  const message = `$TOHSENO IS LIVE ON ROBINHOOD CHAIN ~ ${address}`;
  const labels = [...contractButton.querySelectorAll("[data-contract-text]")];
  let resetTimer;

  contractButton.addEventListener("click", async () => {
    try {
      await copyText(address);
      window.clearTimeout(resetTimer);
      labels.forEach((label) => {
        label.textContent = "COPIED!";
      });
      contractButton.setAttribute("aria-label", "Contract address copied");
      contractButton.classList.remove("is-copied");
      void contractButton.offsetWidth;
      contractButton.classList.add("is-copied");
      status.textContent = "TOHSENO contract address copied.";
      resetTimer = window.setTimeout(() => {
        labels.forEach((label) => {
          label.textContent = message;
        });
        contractButton.setAttribute(
          "aria-label",
          "Copy the TOHSENO contract address",
        );
        contractButton.classList.remove("is-copied");
      }, 2000);
    } catch {
      status.textContent =
        "Could not copy automatically. Select the contract address and copy it.";
    }
  });
}
