#!/usr/bin/env bun

const origin = process.env.STUDIO_ORIGIN;
const evidenceDirectory = process.env.TOHSENO_BROWSER_EVIDENCE_DIR;
const debuggingPort = Number(process.env.TOHSENO_BROWSER_DEBUG_PORT || "49221");
if (!origin?.startsWith("http://127.0.0.1:") || !evidenceDirectory || !Number.isInteger(debuggingPort)) {
  throw new Error("STUDIO_ORIGIN, TOHSENO_BROWSER_EVIDENCE_DIR, and a valid debug port are required");
}

const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const profile = `/tmp/tohseno-studio-browser-${debuggingPort}`;
const child = Bun.spawn([
  chrome,
  `--remote-debugging-port=${debuggingPort}`,
  `--user-data-dir=${profile}`,
  "--no-first-run",
  "--disable-default-apps",
  "--disable-background-networking",
  "--window-size=1180,900",
  `${origin}/create?name=tohseno`,
], { stdout: "ignore", stderr: "ignore" });
child.unref();
let browserStopped = false;
function stopBrowser() {
  if (browserStopped) return;
  browserStopped = true;
  try {
    child.kill("SIGTERM");
  } catch {}
}
process.on("exit", stopBrowser);
process.on("SIGINT", () => {
  stopBrowser();
  process.exit(130);
});
process.on("SIGTERM", () => {
  stopBrowser();
  process.exit(143);
});

let page;
for (let attempt = 0; attempt < 100; attempt += 1) {
  try {
    const pages = await fetch(`http://127.0.0.1:${debuggingPort}/json/list`).then((response) => response.json());
    page = pages.find((candidate) => candidate.type === "page" && candidate.url.startsWith(origin));
    if (page) break;
  } catch {}
  await Bun.sleep(100);
}
if (!page) throw new Error("Chrome did not expose the Studio page");

const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.addEventListener("open", resolve, { once: true });
  socket.addEventListener("error", reject, { once: true });
});
let sequence = 1;
const pending = new Map();
socket.addEventListener("message", (event) => {
  const message = JSON.parse(event.data);
  if (!message.id) return;
  const waiter = pending.get(message.id);
  if (!waiter) return;
  pending.delete(message.id);
  if (message.error) waiter.reject(new Error(message.error.message));
  else waiter.resolve(message.result);
});
function command(method, params = {}) {
  const id = sequence++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const response = await command("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) throw new Error("Studio JavaScript evaluation failed");
  return response.result.value;
}
async function waitFor(expression, message) {
  for (let attempt = 0; attempt < 1_800; attempt += 1) {
    if (await evaluate(expression)) return;
    await Bun.sleep(100);
  }
  throw new Error(message);
}

await command("Page.enable");
await command("Runtime.enable");
await waitFor("document.readyState === 'complete' && document.querySelector('#app-shell')?.dataset.connection === 'connected' && document.querySelector('#service-status')?.textContent.includes('running')", "Studio did not become ready");
const initial = await evaluate(`(() => ({
  origin: location.origin,
  connectPresent: document.querySelector('#connect-iphone') !== null,
  connectWidth: document.querySelector('#connect-iphone')?.getBoundingClientRect().width ?? 0,
  connectHeight: document.querySelector('#connect-iphone')?.getBoundingClientRect().height ?? 0,
  connectText: document.querySelector('#connect-iphone')?.textContent.trim(),
  createName: document.querySelector('#create-name')?.value,
  referenceLimit: document.querySelector('#create-reference-count')?.textContent,
  corsHeaderVisibleToPage: false
}))()`);
const initialScreenshot = await command("Page.captureScreenshot", { format: "png", fromSurface: true });
await Bun.write(`${evidenceDirectory}/studio-create-route.png`, Buffer.from(initialScreenshot.data, "base64"));
await evaluate("document.querySelector('#connect-iphone').click(); true");
await waitFor("document.querySelector('#pairing-dialog')?.open && document.querySelector('#pairing-qr')?.complete && document.querySelector('#pairing-qr')?.naturalWidth > 0", "Studio pairing QR did not render");
await Bun.sleep(500);
const pairing = await evaluate(`(() => {
  const image = document.querySelector('#pairing-qr');
  const frame = document.querySelector('.qr-frame');
  const imageRect = image.getBoundingClientRect();
  const frameRect = frame.getBoundingClientRect();
  return {
    dialogOpen: document.querySelector('#pairing-dialog').open,
    state: document.querySelector('#pairing-state').textContent.trim(),
    countdown: document.querySelector('#pairing-countdown').textContent.trim(),
    imageWidth: imageRect.width,
    imageHeight: imageRect.height,
    naturalWidth: image.naturalWidth,
    naturalHeight: image.naturalHeight,
    framePadding: Number.parseFloat(getComputedStyle(frame).padding),
    sealInsideQrFrame: frame.contains(document.querySelector('.pairing-seal::before')),
    clip: { x: imageRect.x, y: imageRect.y, width: imageRect.width, height: imageRect.height, scale: 1 }
  };
})()`);
const screenshot = await command("Page.captureScreenshot", { format: "png", fromSurface: true });
await Bun.write(`${evidenceDirectory}/studio-pairing-modal.png`, Buffer.from(screenshot.data, "base64"));
const qrScreenshot = await command("Page.captureScreenshot", {
  format: "png",
  fromSurface: true,
  clip: pairing.clip,
});
await Bun.write(`${evidenceDirectory}/studio-rendered-qr.png`, Buffer.from(qrScreenshot.data, "base64"));
delete pairing.clip;

const result = {
  schema: "tohseno.studio-browser-verification/1",
  originMatches: initial.origin === origin,
  connectIphoneVisible: initial.connectPresent && initial.connectWidth > 0 && initial.connectHeight > 0 && initial.connectText.includes("CONNECT IPHONE"),
  createRoutePrepopulated: initial.createName === "tohseno",
  documentedReferenceLimitVisible: initial.referenceLimit === "0 / 8",
  pairingDialogOpen: pairing.dialogOpen,
  pairingStateWaiting: pairing.state === "Waiting for iPhone…",
  pairingCountdownVisible: /^\d+:\d{2}$/.test(pairing.countdown),
  qrSquareAndRendered: pairing.imageWidth >= 190 && pairing.naturalWidth > 0 && pairing.naturalWidth === pairing.naturalHeight,
  qrFramePadding: pairing.framePadding,
  qrRenderedWidth: pairing.imageWidth,
  qrRenderedHeight: pairing.imageHeight,
  qrNaturalWidth: pairing.naturalWidth,
  qrNaturalHeight: pairing.naturalHeight,
  screenshots: ["studio-create-route.png", "studio-pairing-modal.png", "studio-rendered-qr.png"],
  chromeProcessRunning: child.pid > 0,
};
await Bun.write(`${evidenceDirectory}/studio-browser-redacted.json`, `${JSON.stringify(result, null, 2)}\n`);
console.log(JSON.stringify(result));
if (process.env.TOHSENO_BROWSER_WAIT_FOR_PAIRING === "1") {
  await waitFor("document.querySelector('#pairing-state')?.textContent.trim() === 'iPhone connected'", "Studio did not receive the successful pairing update");
  await Bun.sleep(500);
  const paired = await evaluate(`(() => {
    const text = document.querySelector('#device-list')?.textContent || '';
    return {
      pairingState: document.querySelector('#pairing-state')?.textContent.trim(),
      pairingCountdown: document.querySelector('#pairing-countdown')?.textContent.trim(),
      activeDeviceCount: Number(document.querySelector('#device-count')?.textContent),
      capabilitiesVisible: ['view', 'create', 'evolve', 'send feedback', 'send marketing notes'].every(value => text.includes(value)),
      datesVisible: text.includes('Connected') && text.includes('Last synchronized'),
      syncStateVisible: text.includes('SYNC'),
      revokeVisible: text.includes('REVOKE')
    };
  })()`);
  const pairedScreenshot = await command("Page.captureScreenshot", { format: "png", fromSurface: true });
  await Bun.write(`${evidenceDirectory}/studio-pairing-success.png`, Buffer.from(pairedScreenshot.data, "base64"));
  const pairedResult = {
    schema: "tohseno.studio-live-pairing-update/1",
    livePairingUpdate: paired.pairingState === "iPhone connected" && paired.pairingCountdown === "PAIRED",
    activeDeviceCount: paired.activeDeviceCount,
    capabilitiesVisible: paired.capabilitiesVisible,
    datesVisible: paired.datesVisible,
    syncStateVisible: paired.syncStateVisible,
    revokeVisible: paired.revokeVisible,
    screenshot: "studio-pairing-success.png"
  };
  await Bun.write(`${evidenceDirectory}/studio-pairing-live-update.json`, `${JSON.stringify(pairedResult, null, 2)}\n`);
  console.log(JSON.stringify(pairedResult));
}
if (process.env.TOHSENO_BROWSER_HOLD === "1") {
  await new Promise(() => {});
}
socket.close();
stopBrowser();
await Promise.race([child.exited, Bun.sleep(2_000)]);
