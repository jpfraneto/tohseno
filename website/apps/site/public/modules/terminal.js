// The website terminal's pure logic. Nothing here touches the DOM or the
// network, so the checked-in Bun suite exercises the whole command surface,
// the optional app-name rule, and the demo table directly.

// Mirrors `validate_app_name` and `sanitize_component` in engine/src/ledger.rs
// so a name this browser accepts is the same name `tohseno create` accepts on
// the Mac. A divergence here would only be discovered after the handoff.
const RESERVED_APP_NAMES = Object.freeze([
  "identity",
  "locks",
  "walls",
  "apps",
  "config",
  "incomplete",
]);
const MAX_APP_NAME_LENGTH = 63;

const IMAGE_EXTENSION_BY_MEDIA_TYPE = Object.freeze({
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/heic": "heic",
  "image/webp": "webp",
});
const KNOWN_IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "heic", "webp"]);

export function sanitizeAppName(value) {
  let sanitized = "";
  let lastWasDash = false;
  for (const character of value.toLowerCase()) {
    if (/[a-z0-9]/.test(character)) {
      sanitized += character;
      lastWasDash = false;
    } else if (!lastWasDash && sanitized) {
      sanitized += "-";
      lastWasDash = true;
    }
  }
  return sanitized.replace(/^-+|-+$/g, "");
}

/// Returns human copy explaining why a name is unusable, or null when it is.
export function appNameProblem(name) {
  if (!name) return "name it: tohseno create my-app-name";
  if (name.length > MAX_APP_NAME_LENGTH) {
    return `app names stop at ${MAX_APP_NAME_LENGTH} characters`;
  }
  if (RESERVED_APP_NAMES.includes(name)) {
    return `"${name}" is reserved. pick another name.`;
  }
  const sanitized = sanitizeAppName(name);
  if (!sanitized) return "app names are lowercase letters, numbers, and dashes";
  if (sanitized !== name) {
    return `app names are lowercase letters, numbers, and dashes — try "${sanitized}"`;
  }
  return null;
}

/// The listed commands, and only those. `docs` and `privacy` left the status
/// bar and leave this list with it: the help is the short menu a person can
/// hold in their head, not an index of everything that resolves.
export const COMMANDS = Object.freeze([
  {
    name: "create",
    usage: "tohseno create [name]",
    summary: "describe an app; naming it is optional",
  },
  { name: "demo", usage: "tohseno demo", summary: "watch one get built" },
  { name: "install", usage: "tohseno install", summary: "the one-line installer" },
  { name: "token", usage: "tohseno token", summary: "$TOHSENO on dexscreener" },
  { name: "source", usage: "tohseno source", summary: "every line of it" },
  { name: "community", usage: "tohseno community", summary: "where the builders are" },
  { name: "whitepaper", usage: "tohseno whitepaper", summary: "the long form" },
  { name: "help", usage: "help", summary: "this list" },
  { name: "clear", usage: "clear", summary: "clear the screen" },
]);

/// The local label used when an intention or image arrives without a filename.
export const DEFAULT_APP_NAME = "my-app";

/// Every command that only opens something. The href is read from the page
/// chrome where the chrome carries it, so `config.ts` stays the one authority.
export const LINK_COMMANDS = Object.freeze([
  "token",
  "docs",
  "source",
  "privacy",
  "community",
  "whitepaper",
]);

/// Pages that are deliberately in neither the status bar nor the help list,
/// and must still be reachable by name.
export const FALLBACK_LINKS = Object.freeze({
  docs: "/docs",
  privacy: "/privacy",
});

const ALIASES = Object.freeze({
  new: "create",
  "?": "help",
  "--help": "help",
  "-h": "help",
  man: "help",
  cls: "clear",
  repo: "source",
  github: "source",
  paper: "whitepaper",
  $tohseno: "token",
  tohseno$: "token",
  ca: "token",
  chart: "token",
});

/// Resolves one typed line into an intent. `tohseno` is optional, so both
/// `tohseno create`, `tohseno create my-app`, and `create my-app` work.
///
/// The page asks a person to describe an app, so a sentence typed at the
/// prompt is an intention, not a mistake. Anything unrecognized that reads
/// like prose becomes one. A lone unrecognized word is still treated as a
/// mistyped command, because that is what a lone word usually is.
export function resolveCommand(input) {
  const raw = input.trim().replace(/\s+/g, " ");
  if (!raw) return { kind: "empty" };
  const words = raw.split(" ");
  const start = words[0].toLowerCase() === "tohseno" ? 1 : 0;
  const typed = (words[start] ?? "").toLowerCase();
  const argument = words.slice(start + 1).join(" ");
  if (!typed) return { kind: "bare" };
  const name = ALIASES[typed] ?? typed;
  if (name === "create") return { kind: "create", argument };
  if (name === "evolve") return { kind: "evolve", argument };
  if (name === "demo") return { kind: "demo" };
  if (name === "install") return { kind: "install" };
  if (name === "help") return { kind: "help" };
  if (name === "clear") return { kind: "clear" };
  if (LINK_COMMANDS.includes(name)) return { kind: "link", link: name };
  // Typing the `tohseno` prefix is a clear attempt at a command, so it is
  // never reinterpreted as prose.
  if (start === 0 && words.length > 1) return { kind: "intention", text: input.trim() };
  return { kind: "unknown", typed };
}

/// The three doors offered once an intent is written. Only `mac` can complete
/// today: the iPhone Companion is not published, so `app` explains itself and
/// hands back rather than pretending to be available.
export const DOORS = Object.freeze([
  { id: "mac", label: "send it to my mac", note: "ready" },
  { id: "app", label: "link the tohseno app", note: "soon" },
  { id: "demo", label: "see a demo", note: "no install" },
]);

export function cycleIndex(index, delta, length) {
  if (length < 1) return 0;
  return ((index + delta) % length + length) % length;
}

/// The demo replays the six human states from application/src/presentation.rs
/// with that file's exact headlines, so the website cannot describe a build
/// differently from the Mac and the phone.
export const DEMO_STEPS = Object.freeze([
  { state: "waiting", headline: "Waiting to build…", holdMs: 900 },
  {
    state: "building",
    headline: "Building your app…",
    detail: "source · build · tests · experience verification",
    holdMs: 2600,
  },
  {
    state: "ready_for_phone",
    headline: "Your app is ready.",
    detail: "Plug your iPhone into this Mac and I'll install it automatically.",
    holdMs: 1600,
  },
  { state: "installing", headline: "Installing on your iPhone…", holdMs: 1400 },
  { state: "installed", headline: "{app} is on your iPhone ✓", holdMs: 0 },
]);

export function demoHeadline(step, appName) {
  return step.headline.replace("{app}", appName);
}

export function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function countWords(prompt) {
  const trimmed = prompt.trim();
  return trimmed ? trimmed.split(/\s+/).length : 0;
}

export function intentSummary(prompt, referenceCount) {
  const words = countWords(prompt);
  const wordPart = `${words} ${words === 1 ? "word" : "words"}`;
  if (referenceCount === 0) return wordPart;
  return `${wordPart} · ${referenceCount} ${referenceCount === 1 ? "image" : "images"}`;
}

/// A blank last line is the send gesture, so a person can write paragraphs
/// without a mouse and still finish with the keyboard.
export function endsWithBlankLine(value) {
  return /\n[ \t]*$/.test(value) && value.trim().length > 0;
}

/// The exact command a person pastes on their Mac. The installer parses
/// `--claim` from its own argv, so the token follows `bash -s --`.
export function claimCommand(installCommand, token) {
  return `${installCommand} -s -- --claim ${token}`;
}

const TEXT_EXTENSIONS = new Set(["md", "markdown", "mdown", "txt", "text"]);
const TEXT_MEDIA_TYPES = new Set([
  "text/markdown",
  "text/x-markdown",
  "text/plain",
]);

function fileExtension(name) {
  const lastDot = name.lastIndexOf(".");
  return lastDot === -1 ? "" : name.slice(lastDot + 1).toLowerCase();
}

/// A dropped file is either reference material — an image — or the intention
/// itself, written down. Several platforms hand `.md` files to the browser
/// with an empty media type, so the extension is the authority and the media
/// type only decides the extensionless case.
export function classifyDroppedFile(name = "", mediaType = "") {
  const extension = fileExtension(name);
  if (mediaType.startsWith("image/")) return "image";
  if (KNOWN_IMAGE_EXTENSIONS.has(extension)) return "image";
  if (TEXT_EXTENSIONS.has(extension)) return "text";
  if (!extension && TEXT_MEDIA_TYPES.has(mediaType)) return "text";
  return null;
}

/// A dropped document names the app it describes, the way the file on disk
/// already does. An unusable name is not an error here — the default stands.
export function appNameFromFilename(filename = "") {
  const lastDot = filename.lastIndexOf(".");
  const base = lastDot > 0 ? filename.slice(0, lastDot) : filename;
  const name = sanitizeAppName(base).slice(0, MAX_APP_NAME_LENGTH).replace(/-+$/, "");
  return appNameProblem(name) ? null : name;
}

/// Joins written text onto whatever is already in the prompt, with one blank
/// line between them. Dropping a second file appends; it never overwrites the
/// sentences someone already typed.
export function joinPromptText(existing, added) {
  const base = existing.replace(/\s+$/, "");
  const incoming = added
    .replace(/^\uFEFF/, "")
    .replace(/\r\n?/g, "\n")
    .trim();
  if (!base) return incoming;
  if (!incoming) return base;
  return `${base}\n\n${incoming}`;
}

/// Produces a portable filename `intent-package.js` will accept, keeping the
/// original name legible where it already is. Dropped screenshots and pasted
/// clipboard images routinely carry names the package format rejects.
export function toSafeReferenceName(originalName, mediaType, ordinal, taken = []) {
  const extension = IMAGE_EXTENSION_BY_MEDIA_TYPE[mediaType] ?? "png";
  // Only a recognized image suffix is treated as an extension. Otherwise the
  // whole name is the base, so a dot early in the name cannot swallow it.
  const lastDot = originalName.lastIndexOf(".");
  const suffix = lastDot === -1 ? "" : originalName.slice(lastDot + 1).toLowerCase();
  const withoutExtension = KNOWN_IMAGE_EXTENSIONS.has(suffix)
    ? originalName.slice(0, lastDot)
    : originalName;
  const base = sanitizeAppName(withoutExtension).slice(0, 48) || `image-${ordinal + 1}`;
  let candidate = `${base}.${extension}`;
  let attempt = 2;
  const used = new Set(taken.map((name) => name.toLowerCase()));
  while (used.has(candidate.toLowerCase())) {
    candidate = `${base}-${attempt}.${extension}`;
    attempt += 1;
  }
  return candidate;
}
