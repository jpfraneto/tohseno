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

  const mobileTime = document.querySelector("[data-mobile-time]");
  if (mobileTime instanceof HTMLTimeElement) {
    const now = new Date();
    mobileTime.dateTime = now.toISOString();
    mobileTime.textContent = new Intl.DateTimeFormat(undefined, {
      hour: "numeric",
      minute: "2-digit",
    }).format(now);
  }

  let mobileInstallPrompt = null;
  window.addEventListener("beforeinstallprompt", (event) => {
    event.preventDefault();
    mobileInstallPrompt = event;
  });
  if ("serviceWorker" in navigator) {
    window.addEventListener("load", () => {
      navigator.serviceWorker.register("/sw.js").catch(() => {});
    });
  }

  const mobileLauncher = document.querySelector("[data-mobile-shot-launcher]");
  const mobileApp = document.querySelector("[data-mobile-app]");
  const mobileAppClose = document.querySelector("[data-mobile-app-close]");
  const mobileIntention = document.querySelector("[data-mobile-intention]");
  const mobileBriefFile = document.querySelector("[data-mobile-brief-file]");
  const mobileImages = document.querySelector("[data-mobile-images]");
  const mobileAttachments = document.querySelector("[data-mobile-attachments]");
  const mobileImageCount = document.querySelector("[data-mobile-image-count]");
  const mobileTakeShot = document.querySelector("[data-mobile-take-shot]");
  const mobileStatus = document.querySelector("[data-mobile-intention-status]");
  const mobileAppNext = document.querySelector("[data-mobile-app-next]");
  const mobileInstallApp = document.querySelector("[data-mobile-install-app]");
  const mobileInstallSteps = document.querySelector("[data-mobile-install-steps]");

  if (
    mobileLauncher instanceof HTMLButtonElement &&
    mobileApp instanceof HTMLElement &&
    mobileAppClose instanceof HTMLButtonElement &&
    mobileIntention instanceof HTMLTextAreaElement &&
    mobileBriefFile instanceof HTMLInputElement &&
    mobileImages instanceof HTMLInputElement &&
    mobileAttachments instanceof HTMLElement &&
    mobileImageCount instanceof HTMLElement &&
    mobileTakeShot instanceof HTMLButtonElement &&
    mobileStatus instanceof HTMLElement &&
    mobileAppNext instanceof HTMLElement &&
    mobileInstallApp instanceof HTMLButtonElement &&
    mobileInstallSteps instanceof HTMLOListElement
  ) {
    let referenceImages = [];
    const textEncoder = new TextEncoder();
    const maxReferenceBytes = 20 * 1024 * 1024;

    const littleEndian16 = (value) =>
      new Uint8Array([value & 255, (value >>> 8) & 255]);
    const littleEndian32 = (value) => {
      const unsigned = value >>> 0;
      return new Uint8Array([
        unsigned & 255,
        (unsigned >>> 8) & 255,
        (unsigned >>> 16) & 255,
        (unsigned >>> 24) & 255,
      ]);
    };
    const joinBytes = (parts) => {
      const output = new Uint8Array(
        parts.reduce((length, part) => length + part.byteLength, 0),
      );
      let offset = 0;
      for (const part of parts) {
        output.set(part, offset);
        offset += part.byteLength;
      }
      return output;
    };
    const crcTable = Array.from({ length: 256 }, (_, index) => {
      let value = index;
      for (let bit = 0; bit < 8; bit += 1) {
        value = (value & 1) ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
      }
      return value >>> 0;
    });
    const crc32 = (bytes) => {
      let checksum = 0xffffffff;
      for (const byte of bytes) {
        checksum = crcTable[(checksum ^ byte) & 255] ^ (checksum >>> 8);
      }
      return (checksum ^ 0xffffffff) >>> 0;
    };
    const sha256 = async (bytes) => {
      const digest = await crypto.subtle.digest("SHA-256", bytes);
      return [...new Uint8Array(digest)]
        .map((byte) => byte.toString(16).padStart(2, "0"))
        .join("");
    };
    const safeReferenceName = (name, index) => {
      const cleaned = name
        .normalize("NFKD")
        .replace(/[^a-zA-Z0-9._-]+/g, "-")
        .replace(/^[.-]+|[.-]+$/g, "")
        .slice(0, 96) || `reference-${index + 1}`;
      return `references/${String(index + 1).padStart(2, "0")}-${cleaned}`;
    };
    const createZip = (entries) => {
      const localRecords = [];
      const centralRecords = [];
      let localOffset = 0;
      const now = new Date();
      const dosTime =
        (now.getHours() << 11) |
        (now.getMinutes() << 5) |
        Math.floor(now.getSeconds() / 2);
      const dosDate =
        ((Math.max(1980, now.getFullYear()) - 1980) << 9) |
        ((now.getMonth() + 1) << 5) |
        now.getDate();

      for (const entry of entries) {
        const name = textEncoder.encode(entry.path);
        const checksum = crc32(entry.bytes);
        const local = joinBytes([
          littleEndian32(0x04034b50),
          littleEndian16(20),
          littleEndian16(0x0800),
          littleEndian16(0),
          littleEndian16(dosTime),
          littleEndian16(dosDate),
          littleEndian32(checksum),
          littleEndian32(entry.bytes.byteLength),
          littleEndian32(entry.bytes.byteLength),
          littleEndian16(name.byteLength),
          littleEndian16(0),
          name,
          entry.bytes,
        ]);
        const central = joinBytes([
          littleEndian32(0x02014b50),
          littleEndian16(20),
          littleEndian16(20),
          littleEndian16(0x0800),
          littleEndian16(0),
          littleEndian16(dosTime),
          littleEndian16(dosDate),
          littleEndian32(checksum),
          littleEndian32(entry.bytes.byteLength),
          littleEndian32(entry.bytes.byteLength),
          littleEndian16(name.byteLength),
          littleEndian16(0),
          littleEndian16(0),
          littleEndian16(0),
          littleEndian16(0),
          littleEndian32(0),
          littleEndian32(localOffset),
          name,
        ]);
        localRecords.push(local);
        centralRecords.push(central);
        localOffset += local.byteLength;
      }

      const localDirectory = joinBytes(localRecords);
      const centralDirectory = joinBytes(centralRecords);
      const end = joinBytes([
        littleEndian32(0x06054b50),
        littleEndian16(0),
        littleEndian16(0),
        littleEndian16(entries.length),
        littleEndian16(entries.length),
        littleEndian32(centralDirectory.byteLength),
        littleEndian32(localDirectory.byteLength),
        littleEndian16(0),
      ]);
      return new Blob([localDirectory, centralDirectory, end], {
        type: "application/zip",
      });
    };

    const buildShotSeed = async (markdown) => {
      const intentionBytes = textEncoder.encode(`${markdown.trim()}\n`);
      const references = await Promise.all(
        referenceImages.map(async (file, index) => ({
          path: safeReferenceName(file.name, index),
          bytes: new Uint8Array(await file.arrayBuffer()),
          mediaType: file.type || "application/octet-stream",
        })),
      );
      const payloads = [
        {
          path: "intention.md",
          bytes: intentionBytes,
          mediaType: "text/markdown",
        },
        ...references,
      ];
      const files = await Promise.all(
        payloads.map(async (file) => ({
          path: file.path,
          media_type: file.mediaType,
          byte_length: file.bytes.byteLength,
          sha256: await sha256(file.bytes),
        })),
      );
      const manifest = {
        schema: "tohseno.shot-seed/1",
        status: "pre_protocol_private_input",
        created_at: new Date().toISOString(),
        generator: "https://tohseno.com/",
        intention: "intention.md",
        references: references.map((file) => file.path),
        files,
        omitted: [
          "shot_id",
          "builder_id",
          "genome",
          "expression_source",
          "signatures",
          "lineage",
        ],
      };
      const readme = `# TOHSENO Shot Seed

This archive is private pre-protocol input. It is not yet a Shot, a Genome,
signed lineage, or a portable Shot bundle.

## Continue on your Mac

1. Install and open TOHSENO Studio:
   curl -fsSL https://tohseno.com/oneshot.sh | bash
2. Unzip this archive.
3. Start a new Shot and drop intention.md plus the files in references/ into
   the Studio composer.
4. Studio uses the installed, verified TOHSENO rails to propose the Genome.

MASTER_PROMPT.md is intentionally absent: it is historical implementation
input, not current protocol or deployment authority.
`;
      return createZip([
        { path: "README.md", bytes: textEncoder.encode(readme) },
        {
          path: "shot-seed.json",
          bytes: textEncoder.encode(`${JSON.stringify(manifest, null, 2)}\n`),
        },
        ...payloads,
      ]);
    };

    const updateMobileComposer = () => {
      mobileTakeShot.disabled = mobileIntention.value.trim().length === 0;
      mobileImageCount.textContent = `${referenceImages.length} / 8`;
    };

    const renderMobileImages = () => {
      mobileAttachments.replaceChildren(
        ...referenceImages.map((file, index) => {
          const figure = document.createElement("figure");
          const image = document.createElement("img");
          const remove = document.createElement("button");
          const objectUrl = URL.createObjectURL(file);
          image.src = objectUrl;
          image.alt = "";
          image.addEventListener("load", () => URL.revokeObjectURL(objectUrl), {
            once: true,
          });
          remove.type = "button";
          remove.textContent = "×";
          remove.setAttribute("aria-label", `Remove ${file.name}`);
          remove.addEventListener("click", () => {
            referenceImages.splice(index, 1);
            renderMobileImages();
            updateMobileComposer();
          });
          figure.append(image, remove);
          return figure;
        }),
      );
    };

    const addMobileImages = (incomingFiles) => {
      const accepted = [...incomingFiles].filter((file) =>
        (file.size <= maxReferenceBytes) &&
        (/^(image\/(png|jpeg|heic|webp))$/i.test(file.type) ||
          /\.(png|jpe?g|heic|webp)$/i.test(file.name))
      );
      const available = 8 - referenceImages.length;
      referenceImages = [...referenceImages, ...accepted.slice(0, available)];
      renderMobileImages();
      updateMobileComposer();
      if (accepted.length > available) {
        mobileStatus.textContent =
          "Eight reference images are attached; extras were skipped.";
      } else if (accepted.length < incomingFiles.length) {
        mobileStatus.textContent =
          "Some files were skipped. Use PNG, JPEG, HEIC or WebP under 20 MB each.";
      } else {
        mobileStatus.textContent =
          "The ZIP is built on this phone. Nothing is uploaded to tohseno.com.";
      }
    };

    const readMobileBrief = async (file) => {
      if (!file || !/\.(md|markdown|txt)$/i.test(file.name)) return;
      mobileIntention.value = await file.text();
      updateMobileComposer();
      mobileStatus.textContent = `${file.name} is ready.`;
    };

    const openMobileApp = () => {
      mobileApp.hidden = false;
      mobileApp.setAttribute("aria-hidden", "false");
      mobileLauncher.setAttribute("aria-expanded", "true");
      document.body.classList.add("mobile-app-open");
      window.requestAnimationFrame(() => mobileIntention.focus());
    };

    const closeMobileApp = () => {
      mobileApp.hidden = true;
      mobileApp.setAttribute("aria-hidden", "true");
      mobileLauncher.setAttribute("aria-expanded", "false");
      document.body.classList.remove("mobile-app-open");
      mobileLauncher.focus();
    };

    const takeMobileShot = async () => {
      const markdown = mobileIntention.value.trim();
      if (!markdown) return;
      mobileTakeShot.disabled = true;
      mobileTakeShot.textContent = "Building Shot Seed…";
      mobileStatus.textContent = "Sealing the brief and references locally…";
      try {
        const seed = await buildShotSeed(markdown);
        const url = URL.createObjectURL(seed);
        const download = document.createElement("a");
        const stamp = new Date().toISOString().replace(/[-:]/g, "").slice(0, 13);
        download.href = url;
        download.download = `tohseno-shot-seed-${stamp}.zip`;
        document.body.append(download);
        download.click();
        download.remove();
        window.setTimeout(() => URL.revokeObjectURL(url), 30_000);
        mobileStatus.textContent =
          "Shot Seed downloaded. It contains your Markdown, references and integrity manifest.";
        mobileAppNext.hidden = false;
        window.setTimeout(() =>
          mobileAppNext.scrollIntoView({ behavior: "smooth", block: "nearest" }),
        50);
      } catch {
        mobileStatus.textContent =
          "The ZIP could not be created. Your brief and images are still here.";
      } finally {
        mobileTakeShot.textContent = "Download Shot Seed";
        updateMobileComposer();
      }
    };

    mobileLauncher.addEventListener("click", openMobileApp);
    mobileAppClose.addEventListener("click", closeMobileApp);
    mobileIntention.addEventListener("input", () => {
      updateMobileComposer();
      mobileStatus.textContent =
        "The ZIP is built on this phone. Nothing is uploaded to tohseno.com.";
    });
    mobileBriefFile.addEventListener("change", () =>
      readMobileBrief(mobileBriefFile.files?.[0]),
    );
    mobileImages.addEventListener("change", () => {
      if (mobileImages.files) addMobileImages(mobileImages.files);
      mobileImages.value = "";
    });
    mobileIntention.addEventListener("paste", (event) => {
      const pastedFiles = [...(event.clipboardData?.files || [])];
      const brief = pastedFiles.find((file) => /\.(md|markdown|txt)$/i.test(file.name));
      if (brief) {
        event.preventDefault();
        readMobileBrief(brief);
      }
      addMobileImages(pastedFiles);
    });
    mobileIntention.addEventListener("drop", (event) => {
      if (!event.dataTransfer?.files.length) return;
      event.preventDefault();
      const droppedFiles = [...event.dataTransfer.files];
      readMobileBrief(
        droppedFiles.find((file) => /\.(md|markdown|txt)$/i.test(file.name)),
      );
      addMobileImages(droppedFiles);
    });
    mobileTakeShot.addEventListener("click", takeMobileShot);
    mobileInstallApp.addEventListener("click", async () => {
      if (mobileInstallPrompt) {
        await mobileInstallPrompt.prompt();
        const choice = await mobileInstallPrompt.userChoice;
        if (choice.outcome === "accepted") {
          mobileInstallApp.textContent = "TOHSENO installed";
          mobileInstallApp.disabled = true;
        }
        mobileInstallPrompt = null;
        return;
      }
      mobileInstallSteps.hidden = false;
      mobileInstallSteps.focus();
    });
    if (window.matchMedia("(display-mode: standalone)").matches) {
      mobileInstallApp.textContent = "TOHSENO is installed";
      mobileInstallApp.disabled = true;
    }
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && !mobileApp.hidden) closeMobileApp();
    });
    updateMobileComposer();
  }
})();
