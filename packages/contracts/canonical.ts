import type { Sha256Digest, SignedRequestEnvelopeV1 } from "./types";

const encoder = new TextEncoder();
const REQUEST_DOMAIN = "TOHSENO-SIGNED-REQUEST-V1";

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}

export function decodeBase64(value: string): Uint8Array {
  let decoded: string;
  try {
    decoded = atob(value);
  } catch {
    throw new TypeError("Invalid base64 bytes");
  }
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0));
}

export function encodeBase64(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const source = new Uint8Array(bytes);
  const digest = await crypto.subtle.digest("SHA-256", source);
  return bytesToHex(new Uint8Array(digest));
}

export async function sha256Digest(bytes: Uint8Array): Promise<Sha256Digest> {
  return {
    algorithm: "sha-256",
    value: await sha256Hex(bytes),
  };
}

export async function verifySha256Digest(
  bytes: Uint8Array,
  digest: Sha256Digest,
): Promise<boolean> {
  return digest.algorithm === "sha-256" &&
    (await sha256Hex(bytes)) === digest.value;
}

/**
 * Canonical bytes signed by SignedRequestEnvelopeV1.
 *
 * Signature bytes and public-key encoding remain suite-specific. The envelope
 * signs its actual uppercase method and origin-form path; callers must compare
 * those fields with the request being authorized before suite verification.
 */
export function canonicalSignedRequestText(
  envelope: Omit<SignedRequestEnvelopeV1, "signature">,
): string {
  return [
    REQUEST_DOMAIN,
    envelope.method,
    envelope.path,
    `${envelope.bodyHash.algorithm}:${envelope.bodyHash.value}`,
    envelope.timestamp,
    envelope.nonce,
    envelope.signer.suite,
    envelope.signer.keyId,
    envelope.signer.publicKey,
  ].join("\n");
}

export function canonicalSignedRequestBytes(
  envelope: Omit<SignedRequestEnvelopeV1, "signature">,
): Uint8Array {
  return encoder.encode(canonicalSignedRequestText(envelope));
}

export interface RequestBindingResult {
  valid: boolean;
  mismatches: Array<"method" | "path" | "bodyHash">;
}

/**
 * Bind an envelope to the HTTP request before invoking a versioned signature
 * suite. A success here is not signature verification or proof of human action.
 */
export async function verifyRequestBinding(
  envelope: SignedRequestEnvelopeV1,
  actualMethod: string,
  actualPath: string,
  actualBody: Uint8Array,
): Promise<RequestBindingResult> {
  const mismatches: RequestBindingResult["mismatches"] = [];
  if (envelope.method !== actualMethod) mismatches.push("method");
  if (envelope.path !== actualPath) mismatches.push("path");
  if (!(await verifySha256Digest(actualBody, envelope.bodyHash))) {
    mismatches.push("bodyHash");
  }
  return { valid: mismatches.length === 0, mismatches };
}
