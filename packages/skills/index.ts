import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import type {
  AcceptanceFileCheck,
  AppCatalog,
  AppSkillDescriptor,
  AppSkillsLock,
  AppTemplateDescriptor,
  AppliedComposition,
  AppliedFile,
  CatalogKernel,
  CatalogSkill,
  CatalogTemplate,
  DeclaredComposition,
  KernelDescriptor,
  ResolvedComposition,
} from "./types.ts";
export * from "./types.ts";

const ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/u;
const VERSION = /^[0-9]+\.[0-9]+\.[0-9]+$/u;
const DIGEST = /^[a-f0-9]{64}$/u;
const MAX_DESCRIPTOR_BYTES = 256 * 1024;
const MAX_CATALOG_FILES = 10_000;
const MAX_FILE_BYTES = 32 * 1024 * 1024;

export class AppCatalogError extends Error {
  readonly code:
    | "INVALID_DESCRIPTOR"
    | "UNKNOWN_KERNEL"
    | "UNKNOWN_TEMPLATE"
    | "UNKNOWN_SKILL"
    | "DEPENDENCY_CYCLE"
    | "SKILL_CONFLICT"
    | "UNSAFE_PATH"
    | "FILE_COLLISION"
    | "DIGEST_MISMATCH"
    | "ACCEPTANCE_FAILED";

  constructor(code: AppCatalogError["code"], message: string) {
    super(message);
    this.name = "AppCatalogError";
    this.code = code;
  }
}

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function exactKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
  label: string,
): void {
  const allowedSet = new Set(allowed);
  const unknown = Object.keys(value).filter((key) => !allowedSet.has(key));
  if (unknown.length > 0) {
    throw new AppCatalogError(
      "INVALID_DESCRIPTOR",
      `${label} contains unsupported field ${JSON.stringify(unknown[0])}`,
    );
  }
}

function text(value: unknown, label: string): string {
  if (typeof value !== "string" || value.trim() === "") {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `${label} must be a non-empty string`);
  }
  return value;
}

function identifier(value: unknown, label: string): string {
  const result = text(value, label);
  if (!ID.test(result)) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `${label} is not a valid identifier`);
  }
  return result;
}

function version(value: unknown, label: string): string {
  const result = text(value, label);
  if (!VERSION.test(result)) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `${label} must be an exact semantic version`);
  }
  return result;
}

function stringArray(value: unknown, label: string): string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `${label} must be an array of strings`);
  }
  const result = value as string[];
  if (new Set(result).size !== result.length) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `${label} must not contain duplicates`);
  }
  return result;
}

function idArray(value: unknown, label: string): string[] {
  const result = stringArray(value, label);
  if (result.some((item) => !ID.test(item))) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `${label} contains an invalid identifier`);
  }
  return result;
}

function safeRelativePath(value: unknown, label: string): string {
  const path = text(value, label).replaceAll("\\", "/");
  if (
    path.startsWith("/") ||
    path === "." ||
    path === ".." ||
    path.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new AppCatalogError("UNSAFE_PATH", `${label} must stay inside its declared root`);
  }
  return path;
}

function pathArray(value: unknown, label: string): string[] {
  const paths = stringArray(value, label).map((item, index) =>
    safeRelativePath(item, `${label}[${index}]`));
  if (new Set(paths).size !== paths.length) {
    throw new AppCatalogError(
      "INVALID_DESCRIPTOR",
      `${label} must not contain paths that normalize to the same value`,
    );
  }
  return paths;
}

function parseAcceptance(value: unknown): AcceptanceFileCheck[] {
  if (!Array.isArray(value)) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "acceptanceChecks must be an array");
  }
  const checks: AcceptanceFileCheck[] = value.map((item, index) => {
    const check = record(item);
    if (check === null) {
      throw new AppCatalogError(
        "INVALID_DESCRIPTOR",
        `acceptanceChecks[${index}] must be an object`,
      );
    }
    exactKeys(check, ["id", "type", "path"], `acceptanceChecks[${index}]`);
    if (check.type !== "file") {
      throw new AppCatalogError(
        "INVALID_DESCRIPTOR",
        `acceptanceChecks[${index}].type must be file`,
      );
    }
    return {
      id: identifier(check.id, `acceptanceChecks[${index}].id`),
      type: "file",
      path: safeRelativePath(check.path, `acceptanceChecks[${index}].path`),
    };
  });
  if (new Set(checks.map((check) => check.id)).size !== checks.length) {
    throw new AppCatalogError(
      "INVALID_DESCRIPTOR",
      "acceptanceChecks must not contain duplicate ids",
    );
  }
  if (new Set(checks.map((check) => check.path)).size !== checks.length) {
    throw new AppCatalogError(
      "INVALID_DESCRIPTOR",
      "acceptanceChecks must not contain duplicate paths",
    );
  }
  return checks;
}

export function validateSkillDescriptor(value: unknown): AppSkillDescriptor {
  const skill = record(value);
  if (skill === null) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "skill descriptor must be an object");
  }
  exactKeys(skill, [
    "schemaVersion",
    "id",
    "version",
    "title",
    "summary",
    "category",
    "maturity",
    "platforms",
    "requires",
    "conflicts",
    "contributions",
    "acceptanceChecks",
    "agentInstructions",
  ], "skill descriptor");
  if (skill.schemaVersion !== 1) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "skill schemaVersion must be 1");
  }
  const platforms = stringArray(skill.platforms, "platforms");
  if (platforms.length !== 1 || platforms[0] !== "ios") {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "skill platforms must be exactly [\"ios\"]");
  }
  const category = text(skill.category, "category");
  if (!["experience", "data", "progression", "sharing"].includes(category)) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "skill category is unsupported");
  }
  const maturity = text(skill.maturity, "maturity");
  if (maturity !== "experimental" && maturity !== "stable") {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "skill maturity is unsupported");
  }
  const contributions = record(skill.contributions);
  if (contributions === null) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "contributions must be an object");
  }
  exactKeys(contributions, [
    "overlay",
    "manifestFragments",
    "entitlements",
    "privacy",
    "xcodeSourcePaths",
    "replaces",
  ], "contributions");
  return {
    schemaVersion: 1,
    id: identifier(skill.id, "id"),
    version: version(skill.version, "version"),
    title: text(skill.title, "title"),
    summary: text(skill.summary, "summary"),
    category: category as AppSkillDescriptor["category"],
    maturity: maturity as AppSkillDescriptor["maturity"],
    platforms: ["ios"],
    requires: idArray(skill.requires, "requires"),
    conflicts: idArray(skill.conflicts, "conflicts"),
    contributions: {
      overlay: safeRelativePath(contributions.overlay, "contributions.overlay"),
      manifestFragments: pathArray(contributions.manifestFragments, "contributions.manifestFragments"),
      entitlements: pathArray(contributions.entitlements, "contributions.entitlements"),
      privacy: stringArray(contributions.privacy, "contributions.privacy"),
      xcodeSourcePaths: pathArray(contributions.xcodeSourcePaths, "contributions.xcodeSourcePaths"),
      ...(contributions.replaces === undefined
        ? {}
        : { replaces: pathArray(contributions.replaces, "contributions.replaces") }),
    },
    acceptanceChecks: parseAcceptance(skill.acceptanceChecks),
    agentInstructions: stringArray(skill.agentInstructions, "agentInstructions"),
  };
}

export function validateTemplateDescriptor(value: unknown): AppTemplateDescriptor {
  const template = record(value);
  if (template === null) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "template descriptor must be an object");
  }
  exactKeys(template, [
    "schemaVersion",
    "id",
    "version",
    "title",
    "summary",
    "platforms",
    "kernel",
    "skills",
    "overlay",
    "replaces",
    "agentInstructions",
    "definitionOfDone",
  ], "template descriptor");
  if (template.schemaVersion !== 1) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "template schemaVersion must be 1");
  }
  const platforms = stringArray(template.platforms, "platforms");
  if (platforms.length !== 1 || platforms[0] !== "ios") {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "template platforms must be exactly [\"ios\"]");
  }
  return {
    schemaVersion: 1,
    id: identifier(template.id, "id"),
    version: version(template.version, "version"),
    title: text(template.title, "title"),
    summary: text(template.summary, "summary"),
    platforms: ["ios"],
    kernel: identifier(template.kernel, "kernel"),
    skills: idArray(template.skills, "skills"),
    ...(template.overlay === undefined
      ? {}
      : { overlay: safeRelativePath(template.overlay, "overlay") }),
    ...(template.replaces === undefined
      ? {}
      : { replaces: pathArray(template.replaces, "replaces") }),
    agentInstructions: stringArray(template.agentInstructions, "agentInstructions"),
    definitionOfDone: stringArray(template.definitionOfDone, "definitionOfDone"),
  };
}

function validateKernelDescriptor(value: unknown): KernelDescriptor {
  const kernel = record(value);
  if (kernel === null) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "kernel descriptor must be an object");
  }
  exactKeys(kernel, ["schemaVersion", "id", "version", "title", "platforms"], "kernel descriptor");
  const platforms = stringArray(kernel.platforms, "platforms");
  if (
    kernel.schemaVersion !== 1 ||
    platforms.length !== 1 ||
    platforms[0] !== "ios"
  ) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "kernel descriptor is unsupported");
  }
  return {
    schemaVersion: 1,
    id: identifier(kernel.id, "id"),
    version: version(kernel.version, "version"),
    title: text(kernel.title, "title"),
    platforms: ["ios"],
  };
}

function inside(root: string, candidate: string): boolean {
  const path = relative(root, candidate);
  return path === "" || (path !== ".." && !path.startsWith(`..${sep}`));
}

interface CatalogFile {
  absolute: string;
  relative: string;
  bytes: Buffer;
}

function safeTree(rootValue: string): CatalogFile[] {
  const root = realpathSync(rootValue);
  const files: CatalogFile[] = [];
  const visit = (directory: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name))) {
      const path = join(directory, entry.name);
      const details = lstatSync(path);
      if (details.isSymbolicLink()) {
        throw new AppCatalogError("UNSAFE_PATH", `catalog contains a symbolic link: ${relative(root, path)}`);
      }
      if (entry.isDirectory()) {
        visit(path);
        continue;
      }
      if (!entry.isFile() || details.nlink !== 1 || details.size > MAX_FILE_BYTES) {
        throw new AppCatalogError("UNSAFE_PATH", `catalog contains an unsafe file: ${relative(root, path)}`);
      }
      if (files.length >= MAX_CATALOG_FILES) {
        throw new AppCatalogError("UNSAFE_PATH", "catalog contains too many files");
      }
      const canonical = realpathSync(path);
      if (!inside(root, canonical)) {
        throw new AppCatalogError("UNSAFE_PATH", "catalog file escapes its root");
      }
      files.push({
        absolute: canonical,
        relative: relative(root, canonical).split(sep).join("/"),
        bytes: readFileSync(canonical),
      });
    }
  };
  visit(root);
  return files;
}

export function contentDigest(root: string): string {
  const hash = createHash("sha256");
  for (const file of safeTree(root).sort((left, right) =>
    left.relative.localeCompare(right.relative))) {
    hash.update(file.relative);
    hash.update("\0");
    hash.update(createHash("sha256").update(file.bytes).digest("hex"));
    hash.update("\0");
  }
  return hash.digest("hex");
}

function readDescriptor(path: string): unknown {
  const details = lstatSync(path);
  if (details.isSymbolicLink() || !details.isFile() || details.size > MAX_DESCRIPTOR_BYTES) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `descriptor is not a safe regular file: ${path}`);
  }
  try {
    return JSON.parse(readFileSync(path, "utf8")) as unknown;
  } catch {
    throw new AppCatalogError("INVALID_DESCRIPTOR", `descriptor is not valid JSON: ${path}`);
  }
}

function loadDirectories<T>(
  parent: string,
  descriptorName: string,
  parse: (value: unknown) => T,
  idOf: (value: T) => string,
): Map<string, { directory: string; descriptorPath: string; descriptor: T; digest: string }> {
  const result = new Map<string, {
    directory: string;
    descriptorPath: string;
    descriptor: T;
    digest: string;
  }>();
  for (const entry of readdirSync(parent, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    if (!entry.isDirectory()) continue;
    const directory = join(parent, entry.name);
    const descriptorPath = join(directory, descriptorName);
    if (!existsSync(descriptorPath)) continue;
    const descriptor = parse(readDescriptor(descriptorPath));
    const id = idOf(descriptor);
    if (entry.name !== id) {
      throw new AppCatalogError(
        "INVALID_DESCRIPTOR",
        `${descriptorName} id ${id} does not match directory ${entry.name}`,
      );
    }
    if (result.has(id)) {
      throw new AppCatalogError("INVALID_DESCRIPTOR", `duplicate catalog id ${id}`);
    }
    result.set(id, {
      directory: realpathSync(directory),
      descriptorPath: realpathSync(descriptorPath),
      descriptor,
      digest: contentDigest(directory),
    });
  }
  return result;
}

export function loadCatalog(rootValue: string): AppCatalog {
  const root = realpathSync(rootValue);
  const skillsRoot = join(root, "skills");
  const templatesRoot = join(root, "templates");
  const kernelsRoot = join(root, "kernels");
  for (const path of [skillsRoot, templatesRoot, kernelsRoot]) {
    const details = lstatSync(path);
    if (details.isSymbolicLink() || !details.isDirectory()) {
      throw new AppCatalogError("UNSAFE_PATH", `catalog directory is unsafe: ${path}`);
    }
  }
  return {
    root,
    skills: loadDirectories<AppSkillDescriptor>(
      skillsRoot,
      "skill.json",
      validateSkillDescriptor,
      (value) => value.id,
    ) as Map<string, CatalogSkill>,
    templates: loadDirectories<AppTemplateDescriptor>(
      templatesRoot,
      "template.json",
      validateTemplateDescriptor,
      (value) => value.id,
    ) as Map<string, CatalogTemplate>,
    kernels: loadDirectories<KernelDescriptor>(
      kernelsRoot,
      "kernel.json",
      validateKernelDescriptor,
      (value) => value.id,
    ) as Map<string, CatalogKernel>,
  };
}

export function resolveComposition(
  catalog: AppCatalog,
  declaration: DeclaredComposition,
): ResolvedComposition {
  const parsed = validateCompositionDeclaration(declaration);
  const { template, kernel } = compositionBase(catalog, parsed);
  const templateComposition = resolveSkillSeeds(
    catalog,
    template,
    kernel,
    template.descriptor.skills,
  );
  const suppliedByTemplate = new Set(
    templateComposition.skills.map((skill) => skill.descriptor.id),
  );
  const overlap = parsed.skills.find((id) => suppliedByTemplate.has(id));
  if (overlap !== undefined) {
    throw new AppCatalogError(
      "INVALID_DESCRIPTOR",
      `app skill ${overlap} is already supplied by template ${template.descriptor.id}`,
    );
  }
  return resolveSkillSeeds(
    catalog,
    template,
    kernel,
    [...template.descriptor.skills, ...parsed.skills],
  );
}

function validateCompositionDeclaration(value: unknown): DeclaredComposition {
  const declaration = record(value);
  if (declaration === null) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "composition must be an object");
  }
  exactKeys(declaration, ["schemaVersion", "template", "skills"], "composition");
  if (declaration.schemaVersion !== 1) {
    throw new AppCatalogError("INVALID_DESCRIPTOR", "composition schemaVersion must be 1");
  }
  return {
    schemaVersion: 1,
    template: identifier(declaration.template, "composition.template"),
    skills: idArray(declaration.skills, "composition.skills"),
  };
}

function compositionBase(
  catalog: AppCatalog,
  declaration: DeclaredComposition,
): { template: CatalogTemplate; kernel: CatalogKernel } {
  const template = catalog.templates.get(declaration.template);
  if (template === undefined) {
    throw new AppCatalogError("UNKNOWN_TEMPLATE", `unknown template ${declaration.template}`);
  }
  const kernel = catalog.kernels.get(template.descriptor.kernel);
  if (kernel === undefined) {
    throw new AppCatalogError("UNKNOWN_KERNEL", `unknown kernel ${template.descriptor.kernel}`);
  }
  return { template, kernel };
}

function resolveSkillSeeds(
  catalog: AppCatalog,
  template: CatalogTemplate,
  kernel: CatalogKernel,
  seeds: readonly string[],
): ResolvedComposition {
  const resolved: CatalogSkill[] = [];
  const visiting = new Set<string>();
  const visited = new Set<string>();
  const visit = (id: string, trail: readonly string[]): void => {
    if (visited.has(id)) return;
    if (visiting.has(id)) {
      throw new AppCatalogError(
        "DEPENDENCY_CYCLE",
        `app skill dependency cycle: ${[...trail, id].join(" -> ")}`,
      );
    }
    const skill = catalog.skills.get(id);
    if (skill === undefined) {
      throw new AppCatalogError("UNKNOWN_SKILL", `unknown app skill ${id}`);
    }
    visiting.add(id);
    for (const dependency of [...skill.descriptor.requires].sort()) {
      visit(dependency, [...trail, id]);
    }
    visiting.delete(id);
    visited.add(id);
    resolved.push(skill);
  };
  for (const id of [...seeds].sort()) visit(id, []);

  const installed = new Set(resolved.map((skill) => skill.descriptor.id));
  for (const skill of resolved) {
    for (const conflict of skill.descriptor.conflicts) {
      if (installed.has(conflict)) {
        throw new AppCatalogError(
          "SKILL_CONFLICT",
          `app skill ${skill.descriptor.id} conflicts with ${conflict}`,
        );
      }
    }
  }
  return { template, kernel, skills: resolved };
}

/**
 * Validates the complete, persisted skill list used by Shot plans and locks.
 * Unlike resolveComposition(), this accepts no shorthand: template defaults,
 * transitive dependencies, and canonical dependency order must all be present.
 */
export function resolveInstalledComposition(
  catalog: AppCatalog,
  declaration: DeclaredComposition,
): ResolvedComposition {
  const parsed = validateCompositionDeclaration(declaration);
  const { template, kernel } = compositionBase(catalog, parsed);
  const missingDefault = template.descriptor.skills.find((id) =>
    !parsed.skills.includes(id)
  );
  if (missingDefault !== undefined) {
    throw new AppCatalogError(
      "INVALID_DESCRIPTOR",
      `installed composition is missing template skill ${missingDefault}`,
    );
  }
  const composition = resolveSkillSeeds(
    catalog,
    template,
    kernel,
    parsed.skills,
  );
  const canonical = composition.skills.map((skill) => skill.descriptor.id);
  if (JSON.stringify(parsed.skills) !== JSON.stringify(canonical)) {
    throw new AppCatalogError(
      "INVALID_DESCRIPTOR",
      "installed composition skill list is not complete and canonical",
    );
  }
  return composition;
}

function copyContribution(options: {
  source: string;
  target: string;
  owner: string;
  replaces: ReadonlySet<string>;
  owners: Map<string, string>;
  files: AppliedFile[];
}): void {
  for (const file of safeTree(options.source)) {
    const relativePath = file.relative;
    const existingOwner = options.owners.get(relativePath);
    if (existingOwner !== undefined && !options.replaces.has(relativePath)) {
      throw new AppCatalogError(
        "FILE_COLLISION",
        `${options.owner} may not overwrite ${relativePath} owned by ${existingOwner}`,
      );
    }
    const destination = resolve(options.target, relativePath);
    if (!inside(options.target, destination)) {
      throw new AppCatalogError("UNSAFE_PATH", `${options.owner} overlay escapes the shot root`);
    }
    mkdirSync(dirname(destination), { recursive: true });
    writeFileSync(destination, file.bytes, { mode: lstatSync(file.absolute).mode & 0o111 ? 0o755 : 0o644 });
    const sha256 = createHash("sha256").update(file.bytes).digest("hex");
    options.owners.set(relativePath, options.owner);
    const previous = options.files.findIndex((candidate) => candidate.path === relativePath);
    if (previous >= 0) options.files.splice(previous, 1);
    options.files.push({ path: relativePath, owner: options.owner, sha256 });
  }
}

export function lockForComposition(
  composition: ResolvedComposition,
  factoryReleaseId: string,
  files: AppliedFile[] = [],
): AppSkillsLock {
  return {
    schemaVersion: 1,
    factoryReleaseId,
    kernel: {
      id: composition.kernel.descriptor.id,
      version: composition.kernel.descriptor.version,
      digest: composition.kernel.digest,
    },
    template: {
      id: composition.template.descriptor.id,
      version: composition.template.descriptor.version,
      digest: composition.template.digest,
    },
    skills: composition.skills.map((skill) => ({
      id: skill.descriptor.id,
      version: skill.descriptor.version,
      digest: skill.digest,
    })),
    resolvedOrder: composition.skills.map((skill) => skill.descriptor.id),
    files,
  };
}

export function applyComposition(options: {
  composition: ResolvedComposition;
  target: string;
  factoryReleaseId: string;
}): AppliedComposition {
  const target = resolve(options.target);
  mkdirSync(target, { recursive: true });
  if (readdirSync(target).length > 0) {
    throw new AppCatalogError("FILE_COLLISION", "composition target must be empty");
  }
  const owners = new Map<string, string>();
  const files: AppliedFile[] = [];
  copyContribution({
    source: join(options.composition.kernel.directory, "overlay"),
    target,
    owner: `kernel:${options.composition.kernel.descriptor.id}`,
    replaces: new Set(),
    owners,
    files,
  });
  const templateOverlay = options.composition.template.descriptor.overlay;
  if (templateOverlay !== undefined) {
    copyContribution({
      source: join(options.composition.template.directory, templateOverlay),
      target,
      owner: `template:${options.composition.template.descriptor.id}`,
      replaces: new Set(options.composition.template.descriptor.replaces ?? []),
      owners,
      files,
    });
  }
  for (const skill of options.composition.skills) {
    copyContribution({
      source: join(skill.directory, skill.descriptor.contributions.overlay),
      target,
      owner: `skill:${skill.descriptor.id}`,
      replaces: new Set(skill.descriptor.contributions.replaces ?? []),
      owners,
      files,
    });
  }
  return {
    lock: lockForComposition(
      options.composition,
      options.factoryReleaseId,
      files
        .filter((file) =>
          file.path !== "Config/App.xcconfig" &&
          file.path !== "app.manifest.json"
        )
        .sort((left, right) => left.path.localeCompare(right.path)),
    ),
    files: files.sort((left, right) => left.path.localeCompare(right.path)),
  };
}

export function verifyLock(
  catalog: AppCatalog,
  lock: AppSkillsLock,
): ResolvedComposition {
  if (
    lock.schemaVersion !== 1 ||
    !DIGEST.test(lock.kernel.digest) ||
    !DIGEST.test(lock.template.digest) ||
    lock.skills.some((skill) => !DIGEST.test(skill.digest))
  ) {
    throw new AppCatalogError("DIGEST_MISMATCH", "app skill lock has an invalid shape");
  }
  const composition = resolveInstalledComposition(catalog, {
    schemaVersion: 1,
    template: lock.template.id,
    skills: lock.skills.map((skill) => skill.id),
  });
  const expected = lockForComposition(composition, lock.factoryReleaseId);
  const header = { ...lock, files: [] };
  if (JSON.stringify(expected) !== JSON.stringify(header)) {
    throw new AppCatalogError("DIGEST_MISMATCH", "app skill lock does not match authenticated catalog content");
  }
  return composition;
}

export function runAcceptanceChecks(
  rootValue: string,
  composition: ResolvedComposition,
): Array<{ skill: string; check: string; path: string }> {
  const root = realpathSync(rootValue);
  const passed: Array<{ skill: string; check: string; path: string }> = [];
  for (const skill of composition.skills) {
    for (const check of skill.descriptor.acceptanceChecks) {
      const path = resolve(root, check.path);
      if (!inside(root, path) || !existsSync(path)) {
        throw new AppCatalogError(
          "ACCEPTANCE_FAILED",
          `${skill.descriptor.id} acceptance check ${check.id} failed: ${check.path} is missing`,
        );
      }
      const details = lstatSync(path);
      if (details.isSymbolicLink() || !details.isFile()) {
        throw new AppCatalogError(
          "ACCEPTANCE_FAILED",
          `${skill.descriptor.id} acceptance check ${check.id} failed: ${check.path} is unsafe`,
        );
      }
      passed.push({ skill: skill.descriptor.id, check: check.id, path: check.path });
    }
  }
  return passed;
}
