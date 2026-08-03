export const LIMITS = Object.freeze({
  promptBytes: 1024 * 1024,
  referenceBytes: 16 * 1024 * 1024,
  totalReferenceBytes: 48 * 1024 * 1024,
  packageBytes: 64 * 1024 * 1024,
  chunkBytes: 1024 * 1024,
  references: 8,
});

export const PACKAGE_SCHEMA = "tohseno.intent-package/1";
export const ENVELOPE_ASSOCIATED_DATA = "tohseno.intent-envelope/1";
