import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign as nodeSign,
  verify as nodeVerify,
  type KeyObject,
} from "node:crypto";
import {
  type IdentityReference,
  type VerificationMethod,
  validateIdentityReference,
  validateVerificationMethod,
} from "../../identity/src/index.ts";

export const ED25519_SUITE = "ed25519-raw-v1" as const;
export const SIGNATURE_ENCODING = "base64url" as const;

export interface SignatureEnvelope {
  identity: IdentityReference;
  suite: string;
  keyId: string;
  publicKey: string;
  encoding: typeof SIGNATURE_ENCODING;
  value: string;
}

export interface Signer {
  readonly verificationMethod: VerificationMethod;
  sign(message: Uint8Array): Promise<SignatureEnvelope>;
}

export interface SignatureVerifier {
  verify(
    message: Uint8Array,
    envelope: SignatureEnvelope,
  ): Promise<boolean>;
}

export interface SignatureSuiteVerifier extends SignatureVerifier {
  readonly suite: string;
}

export class SignatureValidationError extends Error {
  override readonly name = "SignatureValidationError";

  constructor(
    readonly path: string,
    message: string,
  ) {
    super(`${path}: ${message}`);
  }
}

const BASE64URL_PATTERN = /^[A-Za-z0-9_-]+$/u;
const SPKI_ED25519_PREFIX = Buffer.from(
  "302a300506032b6570032100",
  "hex",
);
const PKCS8_ED25519_SEED_PREFIX = Buffer.from(
  "302e020100300506032b657004220420",
  "hex",
);
const SIGNER_DOMAIN = new TextEncoder().encode(
  "TOHSENO-OPAQUE-SIGNATURE-BINDING-V1\0",
);
const TEST_SIGNER_DOMAIN = new TextEncoder().encode(
  "TOHSENO-DETERMINISTIC-TEST-SIGNER-V1\0",
);

function plainObject(value: unknown, path: string): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new SignatureValidationError(path, "must be an object");
  }
  return value as Record<string, unknown>;
}

function assertClosed(
  value: Record<string, unknown>,
  allowed: readonly string[],
  path: string,
): void {
  const allowedSet = new Set(allowed);
  for (const key of Object.keys(value)) {
    if (!allowedSet.has(key)) {
      throw new SignatureValidationError(`${path}.${key}`, "is not allowed");
    }
  }
  for (const key of allowed) {
    if (!Object.hasOwn(value, key)) {
      throw new SignatureValidationError(`${path}.${key}`, "is required");
    }
  }
}

function base64url(value: unknown, path: string): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    !BASE64URL_PATTERN.test(value) ||
    value.includes("=")
  ) {
    throw new SignatureValidationError(path, "must be unpadded base64url");
  }
  const decoded = Buffer.from(value, "base64url");
  if (decoded.toString("base64url") !== value) {
    throw new SignatureValidationError(path, "must be canonical base64url");
  }
  return value;
}

function strictDecoded(
  value: string,
  expectedBytes: number,
  path: string,
): Buffer {
  base64url(value, path);
  const decoded = Buffer.from(value, "base64url");
  if (decoded.byteLength !== expectedBytes) {
    throw new SignatureValidationError(
      path,
      `must encode exactly ${expectedBytes} bytes`,
    );
  }
  return decoded;
}

function methodFromEnvelope(envelope: SignatureEnvelope): VerificationMethod {
  return {
    identity: envelope.identity,
    suite: envelope.suite,
    keyId: envelope.keyId,
    publicKey: envelope.publicKey,
  };
}

function prefixed(bytes: Uint8Array): Uint8Array {
  const prefix = Buffer.allocUnsafe(4);
  prefix.writeUInt32BE(bytes.byteLength);
  return Buffer.concat([prefix, bytes]);
}

function boundSignatureBytes(
  message: Uint8Array,
  method: VerificationMethod,
): Uint8Array {
  const encoder = new TextEncoder();
  return Buffer.concat([
    SIGNER_DOMAIN,
    prefixed(encoder.encode(method.identity.role)),
    prefixed(encoder.encode(method.identity.method)),
    prefixed(encoder.encode(method.identity.id)),
    prefixed(encoder.encode(method.suite)),
    prefixed(encoder.encode(method.keyId)),
    prefixed(encoder.encode(method.publicKey)),
    prefixed(message),
  ]);
}

export function validateSignatureEnvelope(
  value: unknown,
  path = "signature",
): SignatureEnvelope {
  const candidate = plainObject(value, path);
  assertClosed(
    candidate,
    ["identity", "suite", "keyId", "publicKey", "encoding", "value"],
    path,
  );
  if (candidate.encoding !== SIGNATURE_ENCODING) {
    throw new SignatureValidationError(
      `${path}.encoding`,
      `must be ${SIGNATURE_ENCODING}`,
    );
  }
  const method = validateVerificationMethod(
    {
      identity: candidate.identity,
      suite: candidate.suite,
      keyId: candidate.keyId,
      publicKey: candidate.publicKey,
    },
    path,
  );
  return {
    ...method,
    encoding: SIGNATURE_ENCODING,
    value: base64url(candidate.value, `${path}.value`),
  };
}

export function parseSignatureEnvelope(value: unknown): SignatureEnvelope {
  return validateSignatureEnvelope(value);
}

export function ed25519KeyId(publicKey: string | Uint8Array): string {
  const raw = typeof publicKey === "string"
    ? strictDecoded(publicKey, 32, "publicKey")
    : Buffer.from(publicKey);
  if (raw.byteLength !== 32) {
    throw new SignatureValidationError(
      "publicKey",
      "must be exactly 32 bytes",
    );
  }
  const digest = createHash("sha256")
    .update("TOHSENO-ED25519-KEY-ID-V1\0", "utf8")
    .update(raw)
    .digest("base64url");
  return `ed25519_${digest}`;
}

function rawEd25519PublicKey(publicKey: KeyObject): Buffer {
  const encoded = publicKey.export({ type: "spki", format: "der" });
  const bytes = Buffer.from(encoded);
  if (
    bytes.byteLength !== SPKI_ED25519_PREFIX.byteLength + 32 ||
    !bytes.subarray(0, SPKI_ED25519_PREFIX.byteLength).equals(
      SPKI_ED25519_PREFIX,
    )
  ) {
    throw new Error("the generated public key is not canonical Ed25519");
  }
  return bytes.subarray(SPKI_ED25519_PREFIX.byteLength);
}

export class LocalEd25519Signer implements Signer {
  readonly verificationMethod: VerificationMethod;
  readonly #privateKey: KeyObject;

  private constructor(privateKey: KeyObject, method: VerificationMethod) {
    this.#privateKey = privateKey;
    this.verificationMethod = Object.freeze({
      ...method,
      identity: Object.freeze({ ...method.identity }),
    });
  }

  static generate(identityValue: IdentityReference): LocalEd25519Signer {
    const identity = validateIdentityReference(identityValue);
    const pair = generateKeyPairSync("ed25519");
    const rawPublicKey = rawEd25519PublicKey(pair.publicKey);
    const publicKey = rawPublicKey.toString("base64url");
    return new LocalEd25519Signer(pair.privateKey, {
      identity,
      suite: ED25519_SUITE,
      keyId: ed25519KeyId(rawPublicKey),
      publicKey,
    });
  }

  /**
   * Produces a stable test-only signer from explicitly public fixture input.
   *
   * This is not a custody, recovery, or production key derivation mechanism.
   */
  static deterministicForTests(
    identityValue: IdentityReference,
    seed: string | Uint8Array,
    namespace = "default",
  ): LocalEd25519Signer {
    const identity = validateIdentityReference(identityValue);
    if (
      namespace.length < 1 ||
      namespace.length > 100 ||
      /[\u0000-\u001f\u007f]/u.test(namespace)
    ) {
      throw new SignatureValidationError(
        "namespace",
        "has an invalid format",
      );
    }
    const seedBytes = typeof seed === "string"
      ? new TextEncoder().encode(seed)
      : seed;
    const privateSeed = createHash("sha256")
      .update(TEST_SIGNER_DOMAIN)
      .update(prefixed(new TextEncoder().encode(namespace)))
      .update(prefixed(seedBytes))
      .digest();
    const encodedPrivateKey = Buffer.concat([
      PKCS8_ED25519_SEED_PREFIX,
      privateSeed,
    ]);
    const privateKey = createPrivateKey({
      key: encodedPrivateKey,
      type: "pkcs8",
      format: "der",
    });
    const rawPublicKey = rawEd25519PublicKey(
      createPublicKey(
        privateKey.export({ type: "pkcs8", format: "pem" }),
      ),
    );
    const publicKey = rawPublicKey.toString("base64url");
    return new LocalEd25519Signer(privateKey, {
      identity,
      suite: ED25519_SUITE,
      keyId: ed25519KeyId(rawPublicKey),
      publicKey,
    });
  }

  async sign(message: Uint8Array): Promise<SignatureEnvelope> {
    const method = this.verificationMethod;
    const signature = nodeSign(
      null,
      boundSignatureBytes(message, method),
      this.#privateKey,
    );
    return {
      ...method,
      encoding: SIGNATURE_ENCODING,
      value: signature.toString("base64url"),
    };
  }
}

export class Ed25519Verifier implements SignatureSuiteVerifier {
  readonly suite = ED25519_SUITE;

  async verify(
    message: Uint8Array,
    envelopeValue: SignatureEnvelope,
  ): Promise<boolean> {
    try {
      const envelope = validateSignatureEnvelope(envelopeValue);
      if (envelope.suite !== ED25519_SUITE) return false;
      const rawPublicKey = strictDecoded(
        envelope.publicKey,
        32,
        "signature.publicKey",
      );
      const signature = strictDecoded(
        envelope.value,
        64,
        "signature.value",
      );
      if (envelope.keyId !== ed25519KeyId(rawPublicKey)) return false;
      const publicKey = createPublicKey({
        key: Buffer.concat([SPKI_ED25519_PREFIX, rawPublicKey]),
        type: "spki",
        format: "der",
      });
      return nodeVerify(
        null,
        boundSignatureBytes(message, methodFromEnvelope(envelope)),
        publicKey,
        signature,
      );
    } catch {
      return false;
    }
  }
}

export class VerifierSet implements SignatureVerifier {
  readonly #verifiers = new Map<string, SignatureSuiteVerifier>();

  constructor(verifiers: Iterable<SignatureSuiteVerifier> = []) {
    for (const verifier of verifiers) this.register(verifier);
  }

  register(verifier: SignatureSuiteVerifier): this {
    if (this.#verifiers.has(verifier.suite)) {
      throw new Error(`a verifier is already registered for ${verifier.suite}`);
    }
    this.#verifiers.set(verifier.suite, verifier);
    return this;
  }

  async verify(
    message: Uint8Array,
    envelope: SignatureEnvelope,
  ): Promise<boolean> {
    const verifier = this.#verifiers.get(envelope.suite);
    return verifier === undefined
      ? false
      : verifier.verify(message, envelope);
  }
}

export function localEd25519VerifierSet(): VerifierSet {
  return new VerifierSet([new Ed25519Verifier()]);
}
