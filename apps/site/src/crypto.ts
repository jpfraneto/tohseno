const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export interface EncryptedEnvelopeV1 {
  version: 1;
  algorithm: "AES-256-GCM";
  key: "primary";
  context: string;
  nonce: string;
  ciphertext: string;
}

function bytesToBase64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("base64");
}

function base64ToBytes(value: string): Uint8Array {
  return Uint8Array.from(Buffer.from(value, "base64"));
}

function base64Url(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("base64url");
}

function toHex(bytes: ArrayBuffer): string {
  return Buffer.from(bytes).toString("hex");
}

function ownedBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

async function importDataKey(base64Key: string): Promise<CryptoKey> {
  const raw = base64ToBytes(base64Key);
  if (raw.byteLength !== 32) throw new Error("Data encryption key must be exactly 32 bytes");
  return crypto.subtle.importKey("raw", ownedBuffer(raw), { name: "AES-GCM" }, false, ["encrypt", "decrypt"]);
}

export async function encryptString(
  plaintext: string,
  base64Key: string,
  context = "tohseno:generic:v1",
): Promise<string> {
  if (!context || context.length > 512) throw new Error("Encryption context must contain 1 to 512 characters");
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const key = await importDataKey(base64Key);
  const ciphertext = await crypto.subtle.encrypt({
    name: "AES-GCM",
    iv: ownedBuffer(nonce),
    additionalData: encoder.encode(context),
  }, key, encoder.encode(plaintext));
  const envelope: EncryptedEnvelopeV1 = {
    version: 1,
    algorithm: "AES-256-GCM",
    key: "primary",
    context,
    nonce: bytesToBase64(nonce),
    ciphertext: bytesToBase64(new Uint8Array(ciphertext)),
  };
  return JSON.stringify(envelope);
}

function parseEnvelope(serialized: string): EncryptedEnvelopeV1 {
  const value: unknown = JSON.parse(serialized);
  if (
    typeof value !== "object" || value === null ||
    !("version" in value) || value.version !== 1 ||
    !("algorithm" in value) || value.algorithm !== "AES-256-GCM" ||
    !("key" in value) || value.key !== "primary" ||
    !("context" in value) || typeof value.context !== "string" || value.context.length < 1 || value.context.length > 512 ||
    !("nonce" in value) || typeof value.nonce !== "string" ||
    !("ciphertext" in value) || typeof value.ciphertext !== "string"
  ) {
    throw new Error("Unsupported encrypted envelope");
  }
  return value as EncryptedEnvelopeV1;
}

export async function decryptString(
  serialized: string,
  base64Key: string,
  expectedContext?: string,
): Promise<string> {
  const envelope = parseEnvelope(serialized);
  if (expectedContext !== undefined && envelope.context !== expectedContext) {
    throw new Error("Encrypted envelope context mismatch");
  }
  const nonce = base64ToBytes(envelope.nonce);
  if (nonce.byteLength !== 12) throw new Error("Invalid encrypted envelope nonce");
  const key = await importDataKey(base64Key);
  const plaintext = await crypto.subtle.decrypt(
    {
      name: "AES-GCM",
      iv: ownedBuffer(nonce),
      additionalData: encoder.encode(envelope.context),
    },
    key,
    ownedBuffer(base64ToBytes(envelope.ciphertext)),
  );
  return decoder.decode(plaintext);
}

export async function sha256Hex(value: string | Uint8Array): Promise<string> {
  const bytes = typeof value === "string" ? encoder.encode(value) : value;
  return toHex(await crypto.subtle.digest("SHA-256", ownedBuffer(bytes)));
}

export function generateCapabilityToken(): string {
  return base64Url(crypto.getRandomValues(new Uint8Array(32)));
}

export function generateOpaqueId(prefix: string): string {
  return `${prefix}_${base64Url(crypto.getRandomValues(new Uint8Array(18)))}`;
}

export function isCapabilityTokenShape(token: string): boolean {
  return /^[A-Za-z0-9_-]{43}$/.test(token);
}

export async function hashCapabilityToken(token: string): Promise<string> {
  return sha256Hex(`tohseno-capability-v1\0${token}`);
}
