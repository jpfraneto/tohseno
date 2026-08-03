export const INTENT_LIMITS = Object.freeze({
  promptBytes: 1024 * 1024,
  referenceBytes: 16 * 1024 * 1024,
  totalReferenceBytes: 48 * 1024 * 1024,
  packageBytes: 64 * 1024 * 1024,
  ciphertextBytes: 64 * 1024 * 1024 + 32,
  chunkBytes: 1024 * 1024,
  framingAllowance: 32 * 1024,
  references: 8,
  relayLifetimeMs: 7 * 24 * 60 * 60 * 1000,
  uploadLifetimeMs: 60 * 60 * 1000,
  leaseLifetimeMs: 15 * 60 * 1000,
  tombstoneLifetimeMs: 7 * 24 * 60 * 60 * 1000,
  maxChunks: 65,
});

export const ENVELOPE_ASSOCIATED_DATA = "tohseno.intent-envelope/1";
