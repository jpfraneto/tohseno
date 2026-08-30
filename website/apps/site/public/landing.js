const downloadLinks = [...document.querySelectorAll("[data-installer-download]")];

function detectSystem() {
  const clientPlatform = navigator.userAgentData?.platform ?? "";
  const legacyPlatform = navigator.platform ?? "";
  const platform = `${clientPlatform} ${legacyPlatform}`;
  const userAgent = navigator.userAgent ?? "";

  if (
    /iPhone|iPad|iPod/i.test(platform) ||
    /iPhone|iPad|iPod/i.test(userAgent) ||
    (/Mac/i.test(platform) && navigator.maxTouchPoints > 1)
  ) return "ios";
  if (/Mac/i.test(platform)) return "macos";
  if (/Windows|Win32|Win64/i.test(platform) || /Windows/i.test(userAgent)) return "windows";
  if (/Android/i.test(platform) || /Android/i.test(userAgent)) return "android";
  if (/CrOS/i.test(userAgent)) return "chromeos";
  if (/Linux/i.test(platform) || /Linux/i.test(userAgent)) return "linux";
  return "unknown";
}

const system = detectSystem();
const systemCopy = {
  macos: {
    title: "Download for this Mac",
    detail: "macOS 14+ · Apple silicon and Intel",
  },
  ios: {
    title: "Download for Mac",
    detail: "Open this page on a Mac running macOS 14+",
  },
  windows: {
    title: "Download for Mac",
    detail: "You’re on Windows · TOHSENO requires macOS 14+",
  },
  android: {
    title: "Download for Mac",
    detail: "You’re on Android · TOHSENO requires macOS 14+",
  },
  chromeos: {
    title: "Download for Mac",
    detail: "You’re on ChromeOS · TOHSENO requires macOS 14+",
  },
  linux: {
    title: "Download for Mac",
    detail: "You’re on Linux · TOHSENO requires macOS 14+",
  },
  unknown: {
    title: "Download for Mac",
    detail: "TOHSENO requires macOS 14 or later",
  },
}[system];

for (const link of downloadLinks) {
  link.dataset.detectedSystem = system;
  link.querySelector("[data-download-title]").textContent = systemCopy.title;
  link.querySelector("[data-download-detail]").textContent = systemCopy.detail;
  link.setAttribute("aria-label", `${systemCopy.title}. ${systemCopy.detail}`);
}
