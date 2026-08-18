#!/usr/bin/env bun

// Optional local evidence run. It drives a real Chrome against a running
// Local Workspace Service and captures the collapsed Studio surface:
// the creation composer, then the Settings pairing code.

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
await waitFor(
  "document.readyState === 'complete' && document.querySelector('#shell')?.dataset.view === 'compose'",
  "Studio did not open the creation composer",
);
const composer = await evaluate(`(() => ({
  origin: location.origin,
  name: document.querySelector('#compose-name')?.value,
  question: document.querySelector('#compose-question')?.textContent.trim(),
  submit: document.querySelector('#compose-submit')?.textContent.trim(),
  addImages: document.querySelector('#reference-drop')?.textContent.trim(),
  visibleControls: document.querySelectorAll('#compose-view button, #compose-view input, #compose-view textarea').length,
  connectIphoneOnComposer: document.querySelector('#compose-view #add-iphone') !== null
}))()`);
const composerShot = await command("Page.captureScreenshot", { format: "png", fromSurface: true });
await Bun.write(`${evidenceDirectory}/studio-create-route.png`, Buffer.from(composerShot.data, "base64"));

await evaluate("document.querySelector('#open-settings').click(); true");
await waitFor("document.querySelector('#shell')?.dataset.view === 'settings'", "Settings did not open");
await evaluate("document.querySelector('#add-iphone').click(); true");
await waitFor(
  "document.querySelector('#pairing-dialog')?.open && document.querySelector('#pairing-qr')?.complete && document.querySelector('#pairing-qr')?.naturalWidth > 0",
  "Studio pairing QR did not render",
);
await Bun.sleep(500);
const pairing = await evaluate(`(() => {
  const image = document.querySelector('#pairing-qr');
  const rectangle = image.getBoundingClientRect();
  return {
    dialogOpen: document.querySelector('#pairing-dialog').open,
    state: document.querySelector('#pairing-state').textContent.trim(),
    countdown: document.querySelector('#pairing-countdown').textContent.trim(),
    imageWidth: rectangle.width,
    naturalWidth: image.naturalWidth,
    naturalHeight: image.naturalHeight,
    clip: { x: rectangle.x, y: rectangle.y, width: rectangle.width, height: rectangle.height, scale: 1 }
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
  schema: "tohseno.studio-browser-verification/2",
  originMatches: composer.origin === origin,
  createRoutePrepopulated: composer.name === "tohseno",
  composerAsksOneQuestion: composer.question === "What do you want this app to be?",
  primaryActionIsCreateApp: composer.submit === "Create App",
  imagesAreOptionalAndSecondary: composer.addImages === "+ Add images",
  // name, intent, image input, image label, and one primary action.
  composerControlCount: composer.visibleControls,
  deviceAdministrationOffTheComposer: composer.connectIphoneOnComposer === false,
  pairingReachedFromSettings: pairing.dialogOpen,
  pairingStateWaiting: pairing.state === "Waiting for your iPhone…",
  pairingCountdownVisible: /^\d+:\d{2}$/.test(pairing.countdown),
  qrSquareAndRendered: pairing.imageWidth >= 190 && pairing.naturalWidth > 0 && pairing.naturalWidth === pairing.naturalHeight,
  screenshots: ["studio-create-route.png", "studio-pairing-modal.png", "studio-rendered-qr.png"],
  chromeProcessRunning: child.pid > 0,
};
await Bun.write(`${evidenceDirectory}/studio-browser-redacted.json`, `${JSON.stringify(result, null, 2)}\n`);
console.log(JSON.stringify(result));
if (process.env.TOHSENO_BROWSER_WAIT_FOR_PAIRING === "1") {
  await waitFor(
    "document.querySelector('#pairing-state')?.textContent.trim() === 'iPhone connected'",
    "Studio did not receive the successful pairing update",
  );
  const pairedScreenshot = await command("Page.captureScreenshot", { format: "png", fromSurface: true });
  await Bun.write(`${evidenceDirectory}/studio-pairing-success.png`, Buffer.from(pairedScreenshot.data, "base64"));
  await Bun.write(
    `${evidenceDirectory}/studio-pairing-live-update.json`,
    `${JSON.stringify({ schema: "tohseno.studio-live-pairing-update/2", livePairingUpdate: true }, null, 2)}\n`,
  );
}
if (process.env.TOHSENO_BROWSER_HOLD === "1") {
  await new Promise(() => {});
}
socket.close();
stopBrowser();
await Promise.race([child.exited, Bun.sleep(2_000)]);
