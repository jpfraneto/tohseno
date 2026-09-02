import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";

const projectRoot = join(import.meta.dirname, "..");
const distRoot = join(projectRoot, "dist");

async function exists(path: string): Promise<boolean> {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

async function htmlFiles(root: string): Promise<string[]> {
  const entries = await readdir(root, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const path = join(root, entry.name);
      if (entry.isDirectory()) return htmlFiles(path);
      return entry.isFile() && entry.name.endsWith(".html") ? [path] : [];
    }),
  );
  return nested.flat();
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

const home = await readFile(join(distRoot, "index.html"), "utf8");
assert(home.includes("Start where you are"), "docs home must lead with the three useful paths");
assert(home.includes("data-page-ai"), "docs home must include the page-level AI handoff");
assert(!home.includes("data-minute-player"), "the retired minute-player tutorial must not ship");
assert(!home.includes('href="/docs.css"'), "the retired tutorial stylesheet must not ship");
assert(!home.includes('src="/docs.js"'), "the retired tutorial script must not ship");

const guideFiles = (await htmlFiles(join(distRoot, "guide"))).filter((path) => path.endsWith("index.html"));
assert(guideFiles.length === 40, `expected 40 documentation pages, found ${guideFiles.length}`);
assert(await exists(join(distRoot, "pagefind", "pagefind.js")), "Pagefind search index is missing");
assert(await exists(join(distRoot, "sitemap-index.xml")), "sitemap is missing");
assert(await exists(join(distRoot, "llms.txt")), "AI-readable documentation index is missing");
assert(await exists(join(distRoot, "llms-full.txt")), "AI-readable documentation corpus is missing");

for (const file of guideFiles) {
  const content = await readFile(file, "utf8");
  assert(content.includes("data-page-ai"), `page-level AI handoff is missing in ${file}`);
  assert(content.includes("guide-note"), `character guide is missing in ${file}`);
}

for (const member of ["mac", "ledger", "echo-dot", "orbit", "tink", "ione", "companion", "tick", "hearth"]) {
  assert(await exists(join(distRoot, "crew", `${member}.webp`)), `crew portrait ${member} is missing`);
}

const allHtml = await htmlFiles(distRoot);
for (const file of allHtml) {
  const content = await readFile(file, "utf8");
  for (const match of content.matchAll(/href="(\/[^"]*)"/g)) {
    const href = match[1].split("#", 1)[0].split("?", 1)[0];
    if (!href || href.startsWith("/_astro/") || href.startsWith("/pagefind/")) continue;
    if (/\.[a-z0-9]+$/i.test(href)) {
      assert(await exists(join(distRoot, href)), `broken asset link ${href} in ${file}`);
      continue;
    }
    const target = href === "/" ? join(distRoot, "index.html") : join(distRoot, href, "index.html");
    assert(await exists(target), `broken internal link ${href} in ${file}`);
  }
}

console.log(`Verified lightweight home, ${guideFiles.length} guided docs pages, AI feeds, search, sitemap, and internal links.`);
