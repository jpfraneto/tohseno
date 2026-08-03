import { ENVELOPE_ASSOCIATED_DATA } from "./intent-limits.js";
import { sha256Hex } from "./intent-package.js";

const encoder = new TextEncoder();

export function base64url(bytes) {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

export function fromBase64url(value) {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) throw new Error("Malformed base64url value.");
  const padded = value.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((value.length + 3) % 4);
  return Uint8Array.from(atob(padded), (character) => character.charCodeAt(0));
}

export function parseClaimToken(token) {
  const parts = token.split(".");
  if (parts.length !== 4 || parts[0] !== "ti1" || !/^[A-Za-z0-9_-]{32}$/.test(parts[1]) || !/^[A-Za-z0-9_-]{43}$/.test(parts[2]) || !/^[A-Za-z0-9_-]{43}$/.test(parts[3])) throw new Error("Malformed TOHSENO claim token.");
  return { relayId: parts[1], claimCapability: parts[2], key: fromBase64url(parts[3]) };
}

export async function createEncryptedEnvelope(packageBytes) {
  if (!globalThis.crypto?.subtle || !crypto.getRandomValues) throw new Error("Web Crypto is unavailable; no plaintext was uploaded.");
  const keyBytes = crypto.getRandomValues(new Uint8Array(32));
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const capabilities = {
    upload: base64url(crypto.getRandomValues(new Uint8Array(32))),
    status: base64url(crypto.getRandomValues(new Uint8Array(32))),
    claim: base64url(crypto.getRandomValues(new Uint8Array(32))),
  };
  const ciphertext = await encryptIntentPackage(packageBytes, keyBytes, nonce);
  return {
    ciphertext,
    ciphertextSha256: await sha256Hex(ciphertext),
    nonce: base64url(nonce),
    key: base64url(keyBytes),
    capabilities,
    verifiers: {
      upload: await sha256Hex(encoder.encode(capabilities.upload)),
      status: await sha256Hex(encoder.encode(capabilities.status)),
      claim: await sha256Hex(encoder.encode(capabilities.claim)),
    },
    associatedData: ENVELOPE_ASSOCIATED_DATA,
  };
}

export async function encryptIntentPackage(packageBytes, keyBytes, nonce) {
  if (keyBytes.byteLength !== 32 || nonce.byteLength !== 12) throw new Error("AES-GCM key or nonce length is invalid.");
  const key = await crypto.subtle.importKey("raw", keyBytes, "AES-GCM", false, ["encrypt"]);
  return new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce, additionalData: encoder.encode(ENVELOPE_ASSOCIATED_DATA), tagLength: 128 }, key, packageBytes));
}

export function claimToken(relayId, claimCapability, key) {
  const token = `ti1.${relayId}.${claimCapability}.${key}`;
  parseClaimToken(token);
  return token;
}
