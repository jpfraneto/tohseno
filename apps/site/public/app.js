(() => {
  "use strict";

  const bootstrapCapability = async () => {
    const privateDocument = document.body;
    const submissionId = privateDocument.dataset.privateSubmission;
    if (typeof submissionId !== "string" || !/^sub_[A-Za-z0-9_-]{24}$/.test(submissionId)) return;
    const parameters = new URLSearchParams(window.location.hash.slice(1));
    const token = parameters.get("capability");
    if (token === null) return;

    const content = document.querySelector("[data-private-content]");
    const progress = document.querySelector("[data-private-progress]");
    const error = document.querySelector("[data-private-error]");
    if (!(content instanceof HTMLElement) || !(progress instanceof HTMLElement) || !(error instanceof HTMLElement)) return;
    const onBootstrap = privateDocument.hasAttribute("data-capability-bootstrap");
    content.hidden = true;
    progress.hidden = false;
    error.hidden = true;

    const showError = () => {
      content.hidden = true;
      progress.hidden = true;
      error.hidden = false;
    };

    if (!/^[A-Za-z0-9_-]{43}$/.test(token)) {
      showError();
      return;
    }

    try {
      const response = await fetch("/api/capability/session", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json"
        },
        body: JSON.stringify({ submissionId, token }),
        credentials: "same-origin",
        redirect: "error"
      });
      if (!response.ok) {
        showError();
        return;
      }
      const result = await response.json();
      if (
        typeof result !== "object" || result === null ||
        result.authenticated !== true || typeof result.changed !== "boolean"
      ) {
        showError();
        return;
      }
      // Preserve the fragment as the owner's private coding-agent handoff.
      // URL fragments are never included in the reload's HTTP request.
      if (result.changed || onBootstrap) {
        window.location.reload();
        return;
      }
      progress.hidden = true;
      content.hidden = false;
    } catch {
      showError();
    }
  };

  void bootstrapCapability();

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

  const form = document.querySelector("[data-enhanced-form]");
  if (!(form instanceof HTMLFormElement)) return;

  const textarea = form.querySelector("#markdown");
  const fileInput = form.querySelector("#markdown-file");
  const fileNote = form.querySelector("[data-file-note]");
  const count = form.querySelector("[data-byte-count]");
  const errorBox = form.querySelector("[data-form-error]");
  const submitButton = form.querySelector("[data-submit-button]");
  const submitButtonLabel = form.querySelector("[data-button-label]");

  if (
    !(textarea instanceof HTMLTextAreaElement) ||
    !(fileInput instanceof HTMLInputElement) ||
    !(fileNote instanceof HTMLElement) ||
    !(count instanceof HTMLElement) ||
    !(errorBox instanceof HTMLElement) ||
    !(submitButton instanceof HTMLButtonElement) ||
    !(submitButtonLabel instanceof HTMLElement)
  ) {
    return;
  }

  const configuredMaximum = Number.parseInt(count.dataset.maxBytes || "", 10);
  const maximumBytes = Number.isSafeInteger(configuredMaximum) && configuredMaximum > 0
    ? configuredMaximum
    : null;
  const encoder = new TextEncoder();
  const originalButtonText = submitButtonLabel.textContent;

  const formatBytes = (bytes) => {
    if (bytes < 1024) return `${bytes.toLocaleString()} B`;
    return `${(bytes / 1024).toLocaleString(undefined, { maximumFractionDigits: 1 })} KiB`;
  };

  const markdownBytes = () => encoder.encode(textarea.value).byteLength;

  const updateCount = () => {
    const bytes = markdownBytes();
    count.textContent = maximumBytes
      ? `${formatBytes(bytes)} / ${formatBytes(maximumBytes)}`
      : formatBytes(bytes);
    count.dataset.overLimit = String(maximumBytes !== null && bytes > maximumBytes);
  };

  const showError = (message) => {
    const safeMessage = typeof message === "string" && message.trim().length > 0
      ? message.trim().slice(0, 240)
      : "The request could not be completed. Please try again.";
    errorBox.textContent = safeMessage;
    errorBox.hidden = false;
    errorBox.focus({ preventScroll: true });
    errorBox.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "center"
    });
  };

  const clearError = () => {
    errorBox.textContent = "";
    errorBox.hidden = true;
  };

  const setBusy = (busy) => {
    submitButton.disabled = busy;
    submitButton.setAttribute("aria-busy", String(busy));
    if (busy) {
      submitButtonLabel.textContent = submitButton.dataset.busyLabel || "";
    } else {
      submitButtonLabel.textContent = originalButtonText;
    }
  };

  const readJsonResponse = async (response) => {
    const type = response.headers.get("content-type") || "";
    if (!type.toLowerCase().includes("application/json")) return {};
    try {
      return await response.json();
    } catch {
      return {};
    }
  };

  const publicErrorFor = (response, body) => {
    const serverMessage = typeof body.error === "string"
      ? body.error
      : typeof body.message === "string"
        ? body.message
        : "";
    if (serverMessage) return serverMessage;
    if (response.status === 413) return "That Markdown document is larger than the accepted limit.";
    if (response.status === 415) return "The server could not accept that request format.";
    if (response.status === 429) return "Too many attempts were received. Wait a moment and try again.";
    if (response.status >= 500) return "TOHSENO is temporarily unavailable. Your document was not accepted; please try again.";
    return "Check the document, email, and ownership mode, then try again.";
  };

  const redirectFrom = (body) => {
    const candidate = body.checkoutUrl || body.checkout_url || body.statusUrl ||
      body.status_url || body.redirectUrl || body.redirect_url || body.url;
    if (typeof candidate !== "string" || candidate.length === 0) return false;

    try {
      const destination = new URL(candidate, window.location.origin);
      if (destination.protocol !== "https:" && destination.protocol !== "http:") return false;
      window.location.assign(destination.href);
      return true;
    } catch {
      return false;
    }
  };

  textarea.addEventListener("input", () => {
    clearError();
    updateCount();
  });

  fileInput.addEventListener("change", async () => {
    clearError();
    const file = fileInput.files?.[0];
    if (!file) return;

    if (!file.name.toLowerCase().endsWith(".md")) {
      fileInput.value = "";
      showError("Choose a Markdown file whose name ends in .md.");
      return;
    }
    if (maximumBytes !== null && file.size > maximumBytes) {
      fileInput.value = "";
      showError(`That file is ${formatBytes(file.size)}. The maximum is ${formatBytes(maximumBytes)}.`);
      return;
    }

    try {
      const bytes = await file.arrayBuffer();
      const markdown = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
      const decodedSize = encoder.encode(markdown).byteLength;
      if (maximumBytes !== null && decodedSize > maximumBytes) {
        throw new RangeError("Markdown exceeds the configured byte limit.");
      }
      textarea.value = markdown;
      fileNote.textContent = `${file.name} loaded locally (${formatBytes(decodedSize)}).`;
      updateCount();
      textarea.focus();
    } catch (error) {
      fileInput.value = "";
      showError(error instanceof RangeError
        ? "That Markdown file is larger than the accepted limit."
        : "That file is not valid UTF-8 Markdown.");
    }
  });

  form.addEventListener("change", (event) => {
    if (event.target instanceof HTMLInputElement && event.target.name === "operatingMode") {
      clearError();
    }
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    clearError();

    if (!form.reportValidity()) return;
    const markdown = textarea.value;
    if (markdown.trim().length === 0) {
      showError("Describe the continuity app before submitting.");
      textarea.focus();
      return;
    }
    if (maximumBytes !== null && markdownBytes() > maximumBytes) {
      showError(`The Markdown is over the ${formatBytes(maximumBytes)} limit.`);
      textarea.focus();
      return;
    }

    const data = new FormData(form);
    const operatingMode = data.get("operatingMode");
    if (typeof operatingMode !== "string" || operatingMode.length === 0) {
      showError("Choose an ownership and operating mode.");
      return;
    }

    setBusy(true);
    try {
      const response = await fetch(form.action, {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          markdown,
          email: String(data.get("email") || ""),
          operatingMode
        }),
        credentials: "same-origin",
        redirect: "follow"
      });

      const body = await readJsonResponse(response);
      if (!response.ok) {
        showError(publicErrorFor(response, body));
        return;
      }
      if (response.redirected && response.url) {
        window.location.assign(response.url);
        return;
      }
      if (!redirectFrom(body)) {
        showError("The submission was accepted, but no private next step was returned. Contact support@anky.app.");
      }
    } catch {
      showError("The request could not reach TOHSENO. Check your connection and try again.");
    } finally {
      setBusy(false);
    }
  });

  updateCount();
})();
