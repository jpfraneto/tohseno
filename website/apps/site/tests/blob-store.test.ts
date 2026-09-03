import { afterEach, describe, expect, test } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Readable } from "node:stream";
import { loadConfig, safeStartupSummary } from "../config.ts";
import {
  createRegistryBlobStore,
  FilesystemRegistryBlobStore,
  R2RegistryBlobStore,
  RegistryBlobStoreError,
  finalKey,
  pendingKey,
  type R2Client,
} from "../src/blob-store.ts";

const roots: string[] = [];
afterEach(async () => Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))));

function digest(bytes: Uint8Array): `0x${string}` {
  return `0x${new Bun.CryptoHasher("sha256").update(bytes).digest("hex")}`;
}

async function bytes(stream: ReadableStream<Uint8Array>): Promise<Uint8Array> {
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

describe("Registry blob storage", () => {
  test("filesystem mode stages, verifies, promotes create-only, and streams ranges", async () => {
    const root = await mkdtemp(join(tmpdir(), "tohseno-blob-store-"));
    roots.push(root);
    const local = join(root, "upload.bin");
    const value = new TextEncoder().encode("one exact source archive");
    await writeFile(local, value);
    const expected = { digest: digest(value), byteLength: value.byteLength };
    const store = new FilesystemRegistryBlobStore(join(root, "blobs"));
    await store.initialize();

    await store.stagePending("a".repeat(32), "source", local, expected);
    await store.verifyPending("a".repeat(32), "source", expected);
    expect(await store.promotePending("a".repeat(32), "source", expected)).toBe("created");
    expect(await store.promotePending("a".repeat(32), "source", expected)).toBe("existing");
    expect(await store.metadata({ digest: expected.digest })).toEqual({
      ...expected,
      contentType: "application/octet-stream",
    });
    expect(await bytes((await store.read(expected)).stream)).toEqual(value);
    expect(await bytes((await store.read(expected, { start: 4, end: 8 })).stream))
      .toEqual(value.slice(4, 9));
    await store.removePending("a".repeat(32));
    await expect(store.verifyPending("a".repeat(32), "source", expected))
      .rejects.toMatchObject({ kind: "not_found" });
  });

  test("R2 mode uses exact unsharded keys, conditional writes, metadata, and readback hashing", async () => {
    const client = new MemoryR2Client();
    const store = r2Store(client);
    const root = await mkdtemp(join(tmpdir(), "tohseno-r2-store-"));
    roots.push(root);
    const local = join(root, "source.bin");
    const iconPath = join(root, "icon.bin");
    const value = new TextEncoder().encode("durable source");
    const icon = new TextEncoder().encode("exact icon bytes");
    await writeFile(local, value);
    await writeFile(iconPath, icon);
    const expected = { digest: digest(value), byteLength: value.byteLength };
    const expectedIcon = { digest: digest(icon), byteLength: icon.byteLength };
    const id = "b".repeat(32);

    await store.stagePending(id, "source", local, expected);
    await store.stagePending(id, "icon", iconPath, expectedIcon);
    expect(client.puts[0]).toMatchObject({
      Key: `pending/${id}/source`,
      IfNoneMatch: "*",
      ContentType: "application/octet-stream",
    });
    expect(client.puts[1]).toMatchObject({
      Key: `pending/${id}/icon`,
      IfNoneMatch: "*",
      Metadata: {
        "tohseno-sha256": expectedIcon.digest.slice(2),
        "tohseno-byte-length": String(expectedIcon.byteLength),
      },
    });
    expect(await store.promotePending(id, "source", expected)).toBe("created");
    expect(client.puts[2]).toMatchObject({ Key: `sha256/${expected.digest.slice(2)}`, IfNoneMatch: "*" });
    expect(await store.promotePending(id, "icon", expectedIcon)).toBe("created");
    expect(client.puts[3]).toMatchObject({
      Key: `sha256/${expectedIcon.digest.slice(2)}`,
      IfNoneMatch: "*",
    });
    expect(await store.promotePending(id, "source", expected)).toBe("existing");
    expect(await bytes((await store.read(expected, { start: 2, end: 6 })).stream))
      .toEqual(value.slice(2, 7));
    expect(client.getRanges.at(-1)).toBe("bytes=2-6");
  });

  test("an existing R2 final object must match bytes, not merely an ETag", async () => {
    const client = new MemoryR2Client();
    const store = r2Store(client);
    const root = await mkdtemp(join(tmpdir(), "tohseno-r2-integrity-"));
    roots.push(root);
    const local = join(root, "source.bin");
    const value = new TextEncoder().encode("expected bytes");
    await writeFile(local, value);
    const expected = { digest: digest(value), byteLength: value.byteLength };
    const id = "c".repeat(32);
    await store.stagePending(id, "source", local, expected);
    client.objects.set(finalKey(expected.digest), {
      bytes: new TextEncoder().encode("wrong___bytes"),
      contentType: "application/octet-stream",
      metadata: {
        "tohseno-sha256": expected.digest.slice(2),
        "tohseno-byte-length": String(expected.byteLength),
      },
    });
    await expect(store.promotePending(id, "source", expected))
      .rejects.toMatchObject({ kind: "integrity" });
  });

  test("provider failures stay typed and do not become false not-found responses", async () => {
    const client = new MemoryR2Client();
    client.failure = Object.assign(new Error("provider detail that must remain private"), {
      name: "InternalError",
      $metadata: { httpStatusCode: 500 },
    });
    const store = r2Store(client);
    const root = await mkdtemp(join(tmpdir(), "tohseno-r2-transient-"));
    roots.push(root);
    const local = join(root, "source.bin");
    const value = new TextEncoder().encode("expected bytes");
    await writeFile(local, value);
    await expect(store.stagePending("d".repeat(32), "source", local, {
      digest: digest(value), byteLength: value.byteLength,
    })).rejects.toMatchObject({ kind: "transient" });
  });

  test("R2 selection fails closed and derives the one official account endpoint", () => {
    const common = {
      NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
      REGISTRY_ENABLED: "true", REGISTRY_ROOT: "/tmp/registry",
      ROBINHOOD_RPC_URL: "https://rpc.example.test", REGISTRY_BLOB_STORE: "r2",
    };
    expect(() => loadConfig(common)).toThrow("REGISTRY_R2_ACCOUNT_ID");
    const configured = loadConfig({
      ...common,
      REGISTRY_R2_ACCOUNT_ID: "a".repeat(32),
      REGISTRY_R2_BUCKET: "tohseno-registry",
      REGISTRY_R2_ACCESS_KEY_ID: "A".repeat(32),
      REGISTRY_R2_SECRET_ACCESS_KEY: "s".repeat(64),
    });
    expect(configured.registry.r2?.endpoint)
      .toBe(`https://${"a".repeat(32)}.r2.cloudflarestorage.com`);
    expect(configured.registry.blobStore).toBe("r2");
    const startup = JSON.stringify(safeStartupSummary(configured));
    expect(startup).toContain('"registryBlobStore":"r2"');
    expect(startup).not.toContain(configured.registry.r2!.accessKeyId);
    expect(startup).not.toContain(configured.registry.r2!.secretAccessKey);

    const rollback = loadConfig({
      ...common,
      REGISTRY_BLOB_STORE: "filesystem",
    });
    expect(createRegistryBlobStore(rollback.registry).kind).toBe("filesystem");
    expect(rollback.registry.r2).toBeUndefined();
  });

  test("object-key helpers reject alternate namespaces and formatting", () => {
    expect(finalKey(`0x${"12".repeat(32)}`)).toBe(`sha256/${"12".repeat(32)}`);
    expect(pendingKey("e".repeat(32), "icon")).toBe(`pending/${"e".repeat(32)}/icon`);
    expect(() => finalKey("12".repeat(32))).toThrow(RegistryBlobStoreError);
  });
});

function r2Store(client: R2Client): R2RegistryBlobStore {
  return new R2RegistryBlobStore({
    accountId: "a".repeat(32),
    bucket: "tohseno-registry",
    accessKeyId: "A".repeat(32),
    secretAccessKey: "s".repeat(64),
    endpoint: `https://${"a".repeat(32)}.r2.cloudflarestorage.com`,
  }, client);
}

interface StoredObject {
  bytes: Uint8Array;
  contentType: string;
  metadata: Record<string, string>;
}

class MemoryR2Client implements R2Client {
  readonly objects = new Map<string, StoredObject>();
  readonly puts: Array<Record<string, unknown>> = [];
  readonly getRanges: Array<string | undefined> = [];
  failure?: Error;

  async send(command: any): Promise<any> {
    if (this.failure) {
      const failure = this.failure;
      this.failure = undefined;
      throw failure;
    }
    const input = command.input as Record<string, any>;
    switch (command.constructor.name) {
    case "PutObjectCommand": {
      this.puts.push({ ...input, Body: undefined });
      if (input.IfNoneMatch === "*" && this.objects.has(input.Key)) {
        throw Object.assign(new Error("exists"), {
          name: "PreconditionFailed", $metadata: { httpStatusCode: 412 },
        });
      }
      const body = await collect(input.Body as AsyncIterable<Uint8Array>);
      this.objects.set(input.Key, {
        bytes: body,
        contentType: input.ContentType,
        metadata: { ...input.Metadata },
      });
      return { $metadata: { httpStatusCode: 200 } };
    }
    case "HeadObjectCommand": {
      return this.output(input.Key);
    }
    case "GetObjectCommand": {
      const stored = this.required(input.Key);
      this.getRanges.push(input.Range);
      const match = typeof input.Range === "string" ? input.Range.match(/^bytes=(\d+)-(\d+)$/) : null;
      const value = match
        ? stored.bytes.slice(Number(match[1]), Number(match[2]) + 1)
        : stored.bytes.slice();
      const body = Readable.from([value]);
      (body as any).transformToWebStream = () => Readable.toWeb(body);
      return { ...this.output(input.Key, value.byteLength), Body: body };
    }
    case "DeleteObjectCommand":
      this.objects.delete(input.Key);
      return { $metadata: { httpStatusCode: 204 } };
    default:
      throw new Error(`unexpected command ${command.constructor.name}`);
    }
  }

  private output(key: string, contentLength?: number): Record<string, unknown> {
    const stored = this.required(key);
    return {
      ContentLength: contentLength ?? stored.bytes.byteLength,
      ContentType: stored.contentType,
      Metadata: { ...stored.metadata },
    };
  }

  private required(key: string): StoredObject {
    const stored = this.objects.get(key);
    if (!stored) {
      throw Object.assign(new Error("missing"), {
        name: "NoSuchKey", $metadata: { httpStatusCode: 404 },
      });
    }
    return stored;
  }
}

async function collect(body: AsyncIterable<Uint8Array>): Promise<Uint8Array> {
  const chunks: Uint8Array[] = [];
  let length = 0;
  for await (const chunk of body) {
    const value = chunk instanceof Uint8Array ? chunk : new Uint8Array(chunk);
    chunks.push(value);
    length += value.byteLength;
  }
  const output = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return output;
}
