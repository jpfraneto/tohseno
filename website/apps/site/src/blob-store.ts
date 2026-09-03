import {
  DeleteObjectCommand,
  GetObjectCommand,
  HeadObjectCommand,
  PutObjectCommand,
  S3Client,
  S3ServiceException,
} from "@aws-sdk/client-s3";
import { NodeHttpHandler } from "@smithy/node-http-handler";
import { createReadStream } from "node:fs";
import {
  constants,
  copyFile,
  lstat,
  mkdir,
  open,
  rm,
} from "node:fs/promises";
import { dirname, join } from "node:path";
import { Readable } from "node:stream";
import type { RegistryConfig } from "../config.ts";

export type RegistryBlobKind = "source" | "icon";
export type BlobStoreErrorKind = "not_found" | "transient" | "integrity";

export interface RegistryBlobDescriptor {
  digest: `0x${string}`;
  byteLength: number;
}

export interface RegistryBlobLocator {
  digest: `0x${string}`;
  byteLength?: number;
}

export interface RegistryBlobMetadata extends RegistryBlobDescriptor {
  contentType: "application/octet-stream";
}

export interface RegistryBlobRead extends RegistryBlobMetadata {
  stream: ReadableStream<Uint8Array>;
  range?: { start: number; end: number };
}

export class RegistryBlobStoreError extends Error {
  constructor(
    readonly kind: BlobStoreErrorKind,
    message: string,
    readonly cause?: unknown,
  ) {
    super(message);
    this.name = "RegistryBlobStoreError";
  }
}

/**
 * The only Registry persistence seam for immutable public source and icon
 * bytes. Catalogs, jobs, profiles, aliases, Claims state, and upload staging
 * remain in REGISTRY_ROOT.
 */
export interface RegistryBlobStore {
  readonly kind: "filesystem" | "r2";
  initialize(): Promise<void>;
  stagePending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    localPath: string,
    expected: RegistryBlobDescriptor,
  ): Promise<void>;
  verifyPending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    expected: RegistryBlobDescriptor,
  ): Promise<void>;
  promotePending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    expected: RegistryBlobDescriptor,
  ): Promise<"created" | "existing">;
  metadata(expected: RegistryBlobLocator): Promise<RegistryBlobMetadata>;
  read(
    expected: RegistryBlobLocator,
    range?: { start: number; end: number },
  ): Promise<RegistryBlobRead>;
  removePending(stagingID: string): Promise<void>;
}

const CONTENT_TYPE = "application/octet-stream" as const;
const DIGEST_METADATA = "tohseno-sha256";
const LENGTH_METADATA = "tohseno-byte-length";
const REMOTE_REQUEST_TIMEOUT_MS = 30_000;
const REMOTE_TRANSFER_MAXIMUM_MS = 10 * 60_000;
const REMOTE_MINIMUM_BYTES_PER_SECOND = 1024 * 1024;
const REMOTE_OPERATION_ATTEMPTS = 3;

export function createRegistryBlobStore(config: RegistryConfig): RegistryBlobStore {
  if (config.blobStore === "r2") {
    if (!config.r2) {
      throw new RegistryBlobStoreError("integrity", "R2 blob storage configuration is incomplete");
    }
    return new R2RegistryBlobStore(config.r2);
  }
  if (!config.root) {
    throw new RegistryBlobStoreError("integrity", "REGISTRY_ROOT is required for filesystem blob storage");
  }
  return new FilesystemRegistryBlobStore(join(config.root, "blobs"));
}

export class FilesystemRegistryBlobStore implements RegistryBlobStore {
  readonly kind = "filesystem" as const;

  constructor(private readonly root: string) {}

  async initialize(): Promise<void> {
    await mkdir(join(this.root, "sha256"), { recursive: true, mode: 0o700 });
    await mkdir(join(this.root, "pending"), { recursive: true, mode: 0o700 });
  }

  async stagePending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    localPath: string,
    expected: RegistryBlobDescriptor,
  ): Promise<void> {
    validateCoordinates(stagingID, blobKind, expected);
    await verifyLocalFile(localPath, expected);
    const destination = this.pendingPath(stagingID, blobKind);
    await mkdir(dirname(destination), { recursive: true, mode: 0o700 });
    try {
      await copyFile(localPath, destination, constants.COPYFILE_EXCL);
      await syncFileAndDirectory(destination);
    } catch (error) {
      if (!isNodeError(error, "EEXIST")) throw filesystemError(error, "stage pending Registry bytes");
    }
    await verifyLocalFile(destination, expected);
  }

  async verifyPending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    expected: RegistryBlobDescriptor,
  ): Promise<void> {
    validateCoordinates(stagingID, blobKind, expected);
    await verifyLocalFile(this.pendingPath(stagingID, blobKind), expected);
  }

  async promotePending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    expected: RegistryBlobDescriptor,
  ): Promise<"created" | "existing"> {
    await this.verifyPending(stagingID, blobKind, expected);
    const source = this.pendingPath(stagingID, blobKind);
    const destination = this.finalPath(expected.digest);
    await mkdir(dirname(destination), { recursive: true, mode: 0o700 });
    let result: "created" | "existing" = "created";
    try {
      await copyFile(source, destination, constants.COPYFILE_EXCL);
      await syncFileAndDirectory(destination);
    } catch (error) {
      if (!isNodeError(error, "EEXIST")) throw filesystemError(error, "promote Registry bytes");
      result = "existing";
    }
    await verifyLocalFile(destination, expected);
    return result;
  }

  async metadata(expected: RegistryBlobLocator): Promise<RegistryBlobMetadata> {
    validateLocator(expected);
    const path = this.finalPath(expected.digest);
    let details;
    try {
      details = await lstat(path);
    } catch (error) {
      throw filesystemError(error, "read Registry blob metadata");
    }
    if (!details.isFile() || details.isSymbolicLink()) {
      throw new RegistryBlobStoreError("integrity", "Registry blob is not a regular file");
    }
    if (expected.byteLength !== undefined && details.size !== expected.byteLength) {
      throw new RegistryBlobStoreError("integrity", "Registry blob length differs from its signed release");
    }
    return { digest: expected.digest, byteLength: details.size, contentType: CONTENT_TYPE };
  }

  async read(
    expected: RegistryBlobLocator,
    range?: { start: number; end: number },
  ): Promise<RegistryBlobRead> {
    const metadata = await this.metadata(expected);
    validateRange(range, metadata.byteLength);
    const stream = createReadStream(this.finalPath(expected.digest), range);
    return {
      ...metadata,
      stream: Readable.toWeb(stream) as unknown as ReadableStream<Uint8Array>,
      range,
    };
  }

  async removePending(stagingID: string): Promise<void> {
    validateStagingID(stagingID);
    try {
      await rm(join(this.root, "pending", stagingID), { recursive: true, force: true });
    } catch (error) {
      throw filesystemError(error, "remove pending Registry bytes");
    }
  }

  private pendingPath(stagingID: string, kind: RegistryBlobKind): string {
    return join(this.root, "pending", stagingID, kind);
  }

  private finalPath(digest: string): string {
    const hex = digest.slice(2);
    // Preserve the existing local development layout. The R2 implementation
    // deliberately uses the unsharded canonical object key.
    return join(this.root, "sha256", hex.slice(0, 2), hex.slice(2));
  }
}

export interface R2RegistryBlobStoreConfiguration {
  accountId: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;
  endpoint: string;
}

export interface R2Client {
  send(command: unknown, options?: { abortSignal?: AbortSignal }): Promise<any>;
}

export class R2RegistryBlobStore implements RegistryBlobStore {
  readonly kind = "r2" as const;
  private readonly client: R2Client;

  constructor(
    private readonly config: R2RegistryBlobStoreConfiguration,
    client?: R2Client,
  ) {
    this.client = client ?? new S3Client({
      region: "auto",
      endpoint: config.endpoint,
      credentials: {
        accessKeyId: config.accessKeyId,
        secretAccessKey: config.secretAccessKey,
      },
      // Stream retries are owned below so every attempt receives a fresh body.
      // A generic SDK retry cannot replay an already-consumed Node stream.
      maxAttempts: 1,
      requestChecksumCalculation: "WHEN_REQUIRED",
      responseChecksumValidation: "WHEN_REQUIRED",
      requestHandler: new NodeHttpHandler({
        connectionTimeout: 5_000,
        requestTimeout: REMOTE_TRANSFER_MAXIMUM_MS,
        throwOnRequestTimeout: true,
      }),
    }) as unknown as R2Client;
  }

  async initialize(): Promise<void> {
    // Bucket creation and mutation are owner-attended operations. Startup
    // validates configuration; the first object operation proves access.
  }

  async stagePending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    localPath: string,
    expected: RegistryBlobDescriptor,
  ): Promise<void> {
    validateCoordinates(stagingID, blobKind, expected);
    await verifyLocalFile(localPath, expected);
    const key = pendingKey(stagingID, blobKind);
    await boundedRetry(async () => {
      try {
        await this.send(new PutObjectCommand({
          Bucket: this.config.bucket,
          Key: key,
          Body: createReadStream(localPath),
          ContentLength: expected.byteLength,
          ContentType: CONTENT_TYPE,
          Metadata: objectMetadata(expected),
          IfNoneMatch: "*",
        }), transferTimeout(expected.byteLength));
      } catch (error) {
        if (!isPreconditionFailed(error)) throw r2Error(error, "stage pending Registry bytes");
      }
    });
    await this.verifyObject(key, expected);
  }

  async verifyPending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    expected: RegistryBlobDescriptor,
  ): Promise<void> {
    validateCoordinates(stagingID, blobKind, expected);
    await this.verifyObject(pendingKey(stagingID, blobKind), expected);
  }

  async promotePending(
    stagingID: string,
    blobKind: RegistryBlobKind,
    expected: RegistryBlobDescriptor,
  ): Promise<"created" | "existing"> {
    validateCoordinates(stagingID, blobKind, expected);
    const sourceKey = pendingKey(stagingID, blobKind);
    await this.verifyObject(sourceKey, expected);
    const destinationKey = finalKey(expected.digest);
    const result = await boundedRetry(async (): Promise<"created" | "existing"> => {
      let source;
      try {
        source = await this.send(new GetObjectCommand({
          Bucket: this.config.bucket,
          Key: sourceKey,
        }));
      } catch (error) {
        throw r2Error(error, "read pending Registry bytes");
      }
      if (!source.Body) {
        throw new RegistryBlobStoreError("integrity", "R2 returned pending metadata without an object body");
      }
      try {
        await this.send(new PutObjectCommand({
          Bucket: this.config.bucket,
          Key: destinationKey,
          Body: source.Body,
          ContentLength: expected.byteLength,
          ContentType: CONTENT_TYPE,
          Metadata: objectMetadata(expected),
          IfNoneMatch: "*",
        }), transferTimeout(expected.byteLength));
        return "created";
      } catch (error) {
        if (isPreconditionFailed(error)) return "existing";
        throw r2Error(error, "promote Registry bytes");
      }
    });
    // ETags are intentionally ignored. Both a newly written object and a
    // concurrent existing object must prove the signed length and SHA-256.
    await this.verifyObject(destinationKey, expected);
    return result;
  }

  async metadata(expected: RegistryBlobLocator): Promise<RegistryBlobMetadata> {
    validateLocator(expected);
    const output = await boundedRetry(async () => {
      try {
        return await this.send(new HeadObjectCommand({
          Bucket: this.config.bucket,
          Key: finalKey(expected.digest),
        }));
      } catch (error) {
        throw r2Error(error, "read Registry blob metadata");
      }
    });
    const byteLength = verifyRemoteMetadata(output, expected);
    return { digest: expected.digest, byteLength, contentType: CONTENT_TYPE };
  }

  async read(
    expected: RegistryBlobLocator,
    range?: { start: number; end: number },
  ): Promise<RegistryBlobRead> {
    validateLocator(expected);
    const metadata = await this.metadata(expected);
    validateRange(range, metadata.byteLength);
    const output = await boundedRetry(async () => {
      try {
        return await this.send(new GetObjectCommand({
          Bucket: this.config.bucket,
          Key: finalKey(expected.digest),
          Range: range ? `bytes=${range.start}-${range.end}` : undefined,
        }));
      } catch (error) {
        throw r2Error(error, "stream Registry blob");
      }
    });
    if (!output.Body) {
      throw new RegistryBlobStoreError("integrity", "R2 returned Registry metadata without an object body");
    }
    verifyRemoteMetadata(output, metadata, range);
    return {
      digest: metadata.digest,
      byteLength: metadata.byteLength,
      contentType: CONTENT_TYPE,
      stream: boundedRemoteStream(output.Body, transferTimeout(metadata.byteLength)),
      range,
    };
  }

  async removePending(stagingID: string): Promise<void> {
    validateStagingID(stagingID);
    for (const kind of ["source", "icon"] as const) {
      await boundedRetry(async () => {
        try {
          await this.send(new DeleteObjectCommand({
            Bucket: this.config.bucket,
            Key: pendingKey(stagingID, kind),
          }));
        } catch (error) {
          throw r2Error(error, "remove pending Registry bytes");
        }
      });
    }
  }

  private async verifyObject(key: string, expected: RegistryBlobDescriptor): Promise<void> {
    await boundedRetry(async () => {
      let output;
      try {
        output = await this.send(new GetObjectCommand({ Bucket: this.config.bucket, Key: key }));
      } catch (error) {
        throw r2Error(error, "verify Registry bytes");
      }
      if (!output.Body) {
        throw new RegistryBlobStoreError("integrity", "R2 returned Registry metadata without an object body");
      }
      verifyRemoteMetadata(output, expected);
      const observed = await hashBody(
        output.Body as AsyncIterable<Uint8Array>,
        transferTimeout(expected.byteLength),
      );
      if (observed.byteLength !== expected.byteLength || observed.digest !== expected.digest) {
        throw new RegistryBlobStoreError("integrity", "R2 Registry bytes differ from their signed digest or length");
      }
    });
  }

  private async send(command: unknown, timeoutMs = REMOTE_REQUEST_TIMEOUT_MS): Promise<any> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
      return await this.client.send(command, { abortSignal: controller.signal });
    } finally {
      clearTimeout(timeout);
    }
  }
}

export function finalKey(digest: string): string {
  validateDigest(digest);
  return `sha256/${digest.slice(2)}`;
}

export function pendingKey(stagingID: string, kind: RegistryBlobKind): string {
  validateStagingID(stagingID);
  if (kind !== "source" && kind !== "icon") {
    throw new RegistryBlobStoreError("integrity", "Registry blob kind is invalid");
  }
  return `pending/${stagingID}/${kind}`;
}

async function verifyLocalFile(path: string, expected: RegistryBlobDescriptor): Promise<void> {
  validateDescriptor(expected);
  let initial;
  try {
    initial = await lstat(path);
  } catch (error) {
    throw filesystemError(error, "verify local Registry bytes");
  }
  if (!initial.isFile() || initial.isSymbolicLink()) {
    throw new RegistryBlobStoreError("integrity", "Registry bytes are not a regular file");
  }
  const observed = await hashBody(createReadStream(path) as AsyncIterable<Uint8Array>);
  let final;
  try {
    final = await lstat(path);
  } catch (error) {
    throw filesystemError(error, "verify local Registry bytes");
  }
  if (initial.dev !== final.dev || initial.ino !== final.ino || initial.size !== final.size
      || initial.mtimeMs !== final.mtimeMs) {
    throw new RegistryBlobStoreError("integrity", "Registry bytes changed while they were being verified");
  }
  if (observed.byteLength !== expected.byteLength || observed.digest !== expected.digest) {
    throw new RegistryBlobStoreError("integrity", "Registry bytes differ from their signed digest or length");
  }
}

async function hashBody(
  body: AsyncIterable<Uint8Array>,
  timeoutMs?: number,
): Promise<RegistryBlobDescriptor> {
  const hasher = new Bun.CryptoHasher("sha256");
  let byteLength = 0;
  const startedAt = Date.now();
  const iterator = body[Symbol.asyncIterator]();
  let complete = false;
  try {
    while (true) {
      const remaining = timeoutMs === undefined ? undefined : timeoutMs - (Date.now() - startedAt);
      if (remaining !== undefined && remaining <= 0) throw new Error("Registry blob stream timed out");
      const next = remaining === undefined
        ? await iterator.next()
        : await within(iterator.next(), remaining);
      if (next.done) {
        complete = true;
        break;
      }
      const value = next.value;
      const bytes = value instanceof Uint8Array ? value : new Uint8Array(value as never);
      byteLength += bytes.byteLength;
      hasher.update(bytes);
    }
  } catch (error) {
    throw new RegistryBlobStoreError("transient", "Registry blob stream was interrupted", error);
  } finally {
    if (!complete) void iterator.return?.();
  }
  return { digest: `0x${hasher.digest("hex")}`, byteLength };
}

function boundedRemoteStream(body: any, timeoutMs: number): ReadableStream<Uint8Array> {
  const source = body.transformToWebStream() as ReadableStream<Uint8Array>;
  const reader = source.getReader();
  const startedAt = Date.now();
  return new ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        const remaining = timeoutMs - (Date.now() - startedAt);
        if (remaining <= 0) throw new Error("Registry blob stream timed out");
        const next = await within(reader.read(), remaining);
        if (next.done) {
          reader.releaseLock();
          controller.close();
        } else {
          controller.enqueue(next.value);
        }
      } catch (error) {
        void reader.cancel(error);
        controller.error(new RegistryBlobStoreError(
          "transient",
          "Registry blob stream was interrupted",
          error,
        ));
      }
    },
    cancel(reason) {
      return reader.cancel(reason);
    },
  });
}

function transferTimeout(byteLength: number): number {
  const transferMs = Math.ceil(byteLength / REMOTE_MINIMUM_BYTES_PER_SECOND) * 1000;
  return Math.min(REMOTE_TRANSFER_MAXIMUM_MS, REMOTE_REQUEST_TIMEOUT_MS + transferMs);
}

function within<T>(operation: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Registry blob operation timed out")), timeoutMs);
    operation.then(
      (value) => { clearTimeout(timeout); resolve(value); },
      (error) => { clearTimeout(timeout); reject(error); },
    );
  });
}

async function boundedRetry<T>(operation: () => Promise<T>): Promise<T> {
  for (let attempt = 1; attempt <= REMOTE_OPERATION_ATTEMPTS; attempt += 1) {
    try {
      return await operation();
    } catch (error) {
      if (attempt === REMOTE_OPERATION_ATTEMPTS
          || !(error instanceof RegistryBlobStoreError)
          || error.kind !== "transient") {
        throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, 50 * attempt));
    }
  }
  throw new RegistryBlobStoreError("transient", "R2 operation exhausted its retry bound");
}

function objectMetadata(expected: RegistryBlobDescriptor): Record<string, string> {
  return {
    [DIGEST_METADATA]: expected.digest.slice(2),
    [LENGTH_METADATA]: String(expected.byteLength),
  };
}

function verifyRemoteMetadata(
  output: { ContentLength?: number; ContentType?: string; Metadata?: Record<string, string> },
  expected: RegistryBlobLocator,
  range?: { start: number; end: number },
): number {
  const declaredLength = Number(output.Metadata?.[LENGTH_METADATA] ?? "NaN");
  if (!Number.isSafeInteger(declaredLength) || declaredLength < 0) {
    throw new RegistryBlobStoreError("integrity", "R2 Registry object length metadata is invalid");
  }
  if (expected.byteLength !== undefined && declaredLength !== expected.byteLength) {
    throw new RegistryBlobStoreError("integrity", "R2 Registry object length differs from its signed release");
  }
  const expectedContentLength = range
    ? range.end - range.start + 1
    : declaredLength;
  if (output.ContentLength !== expectedContentLength) {
    throw new RegistryBlobStoreError("integrity", "R2 Registry object length differs from its signed release");
  }
  if (output.ContentType !== CONTENT_TYPE) {
    throw new RegistryBlobStoreError("integrity", "R2 Registry object content type is not canonical");
  }
  if (output.Metadata?.[DIGEST_METADATA] !== expected.digest.slice(2)
      || output.Metadata?.[LENGTH_METADATA] !== String(declaredLength)) {
    throw new RegistryBlobStoreError("integrity", "R2 Registry object metadata differs from its signed release");
  }
  return declaredLength;
}

function validateCoordinates(
  stagingID: string,
  kind: RegistryBlobKind,
  expected: RegistryBlobDescriptor,
): void {
  validateStagingID(stagingID);
  if (kind !== "source" && kind !== "icon") {
    throw new RegistryBlobStoreError("integrity", "Registry blob kind is invalid");
  }
  validateDescriptor(expected);
}

function validateDescriptor(expected: RegistryBlobDescriptor): void {
  validateLocator(expected);
  if (expected.byteLength === undefined) {
    throw new RegistryBlobStoreError("integrity", "Registry blob length is invalid");
  }
}

function validateLocator(expected: RegistryBlobLocator): void {
  validateDigest(expected.digest);
  if (expected.byteLength !== undefined
      && (!Number.isSafeInteger(expected.byteLength) || expected.byteLength < 0)) {
    throw new RegistryBlobStoreError("integrity", "Registry blob length is invalid");
  }
}

function validateDigest(value: string): asserts value is `0x${string}` {
  if (!/^0x[0-9a-f]{64}$/.test(value)) {
    throw new RegistryBlobStoreError("integrity", "Registry blob digest is invalid");
  }
}

function validateStagingID(value: string): void {
  if (!/^[0-9a-f]{32}$/.test(value)) {
    throw new RegistryBlobStoreError("integrity", "Registry staging identifier is invalid");
  }
}

function validateRange(range: { start: number; end: number } | undefined, total: number): void {
  if (!range) return;
  if (!Number.isSafeInteger(range.start) || !Number.isSafeInteger(range.end)
      || range.start < 0 || range.end < range.start || range.end >= total) {
    throw new RegistryBlobStoreError("integrity", "Registry byte range is invalid");
  }
}

async function syncFileAndDirectory(path: string): Promise<void> {
  const file = await open(path, "r");
  try { await file.sync(); } finally { await file.close(); }
  const directory = await open(dirname(path), "r");
  try { await directory.sync(); } finally { await directory.close(); }
}

function filesystemError(error: unknown, operation: string): RegistryBlobStoreError {
  if (isNodeError(error, "ENOENT")) {
    return new RegistryBlobStoreError("not_found", `${operation}: object not found`, error);
  }
  return new RegistryBlobStoreError("transient", `${operation}: local storage unavailable`, error);
}

function r2Error(error: unknown, operation: string): RegistryBlobStoreError {
  const status = awsStatus(error);
  const name = awsName(error);
  if (status === 404 || name === "NoSuchKey" || name === "NotFound") {
    return new RegistryBlobStoreError("not_found", `${operation}: object not found`, error);
  }
  return new RegistryBlobStoreError("transient", `${operation}: R2 unavailable`, error);
}

function isPreconditionFailed(error: unknown): boolean {
  return awsStatus(error) === 412 || awsName(error) === "PreconditionFailed";
}

function awsStatus(error: unknown): number | undefined {
  if (error instanceof S3ServiceException) return error.$metadata.httpStatusCode;
  if (typeof error !== "object" || error === null) return undefined;
  const metadata = (error as { $metadata?: { httpStatusCode?: number } }).$metadata;
  return metadata?.httpStatusCode;
}

function awsName(error: unknown): string | undefined {
  if (typeof error !== "object" || error === null) return undefined;
  return (error as { name?: string }).name;
}

function isNodeError(error: unknown, code: string): boolean {
  return typeof error === "object" && error !== null
    && (error as NodeJS.ErrnoException).code === code;
}
