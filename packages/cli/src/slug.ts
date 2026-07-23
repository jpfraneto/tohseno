import { CliError } from "./errors.ts";

const SHOT_SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const MAX_SLUG_LENGTH = 63;

export function validateShotSlug(value: string): string {
  if (value.length === 0) {
    throw new CliError("a shot slug is required (example: tohseno create the-trenches)", 2);
  }
  if (value.length > MAX_SLUG_LENGTH || !SHOT_SLUG.test(value)) {
    throw new CliError(
      `invalid shot slug ${JSON.stringify(value)}; use 1-${MAX_SLUG_LENGTH} lowercase letters, numbers, and single hyphens`,
      2,
    );
  }
  return value;
}

export function displayNameForSlug(slug: string): string {
  return slug
    .split("-")
    .map((part) => `${part[0]!.toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

export function bundleIdForSlug(slug: string): string {
  return `com.tohseno.${slug}`;
}

export function slugForShotName(value: string): string {
  const slug = value
    .trim()
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/gu, "")
    .replace(/[^a-z0-9]+/gu, "-")
    .replace(/^-+|-+$/gu, "")
    .replace(/-+/gu, "-");
  if (slug.length > 63) {
    throw new CliError("shot name is too long after converting it to a filesystem name", 2);
  }
  return validateShotSlug(slug);
}
