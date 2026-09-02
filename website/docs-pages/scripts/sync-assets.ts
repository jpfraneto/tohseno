import { cp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, relative, sep } from "node:path";

const projectRoot = join(import.meta.dirname, "..");
const sourceRoot = join(projectRoot, "..", "apps", "site", "public");
const publicRoot = join(projectRoot, "public");
const docsRoot = join(projectRoot, "src", "content", "docs");

async function markdownFiles(root: string): Promise<string[]> {
  const entries = await readdir(root, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const path = join(root, entry.name);
      if (entry.isDirectory()) return markdownFiles(path);
      return entry.isFile() && /\.mdx?$/.test(entry.name) ? [path] : [];
    }),
  );
  return nested.flat().sort();
}

function frontmatterValue(source: string, key: string): string {
  const frontmatter = source.match(/^---\n([\s\S]*?)\n---/)?.[1] ?? "";
  return frontmatter.match(new RegExp(`^${key}:\\s*(.+)$`, "m"))?.[1]?.trim() ?? "";
}

function routeForFile(path: string): string {
  const route = relative(docsRoot, path)
    .split(sep)
    .join("/")
    .replace(/\.mdx?$/, "")
    .replace(/(^|\/)index$/, "$1");
  return `/${route}`.replace(/\/+$/, "/");
}

const docs = await Promise.all(
  (await markdownFiles(docsRoot)).map(async (path) => {
    const source = await readFile(path, "utf8");
    return {
      title: frontmatterValue(source, "title") || routeForFile(path),
      description: frontmatterValue(source, "description"),
      route: routeForFile(path),
      body: source.replace(/^---\n[\s\S]*?\n---\n?/, "").trim(),
    };
  }),
);

const llmsIndex = [
  "# Tohseno documentation",
  "",
  "> Tohseno connects a native iPhone app to its source, accepted history, and next change.",
  "",
  "This index is generated from the same public Markdown used by the human-readable site.",
  "For a single model-ready corpus, use https://docs.tohseno.com/llms-full.txt.",
  "",
  "## Pages",
  "",
  ...docs.map(
    ({ title, description, route }) =>
      `- [${title}](https://docs.tohseno.com${route})${description ? `: ${description}` : ""}`,
  ),
  "",
].join("\n");

const llmsFull = docs
  .map(
    ({ title, route, body }) =>
      `# ${title}\n\nSource: https://docs.tohseno.com${route}\n\n${body}\n`,
  )
  .join("\n---\n\n");

await mkdir(publicRoot, { recursive: true });
await rm(join(publicRoot, "fonts"), { recursive: true, force: true });
await rm(join(publicRoot, "modules"), { recursive: true, force: true });
await rm(join(publicRoot, "docs.css"), { force: true });
await rm(join(publicRoot, "docs.js"), { force: true });

await Promise.all([
  cp(join(sourceRoot, "favicon.png"), join(publicRoot, "favicon.png")),
  cp(join(sourceRoot, "fonts"), join(publicRoot, "fonts"), { recursive: true }),
  mkdir(join(publicRoot, "modules"), { recursive: true }).then(() =>
    cp(
      join(sourceRoot, "modules", "obsolete-worker-cleanup.js"),
      join(publicRoot, "modules", "obsolete-worker-cleanup.js"),
    ),
  ),
  writeFile(
    join(publicRoot, "_headers"),
    [
      "/*",
      "  X-Content-Type-Options: nosniff",
      "  Referrer-Policy: no-referrer",
      "  Cross-Origin-Opener-Policy: same-origin",
      "  Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=()",
      "  Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' https://tohseno.com data:; connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
      "",
      "/_astro/*",
      "  Cache-Control: public, max-age=31536000, immutable",
      "",
      "/pagefind/*",
      "  Cache-Control: public, max-age=31536000, immutable",
      "",
      "/llms*.txt",
      "  Content-Type: text/plain; charset=utf-8",
      "  Cache-Control: public, max-age=300",
      "",
    ].join("\n"),
  ),
  writeFile(join(publicRoot, "llms.txt"), llmsIndex),
  writeFile(join(publicRoot, "llms-full.txt"), llmsFull),
  writeFile(
    join(publicRoot, "_redirects"),
    ["/docs / 308", "/docs/* /guide/:splat 308", ""].join("\n"),
  ),
]);
