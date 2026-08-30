import { afterEach, describe, expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { p256 } from "@noble/curves/p256";
import { loadConfig } from "../config.ts";
import { canonicalCatalogJSON, canonicalRegistryActionDigest, createRegistryRouter } from "../src/registry.ts";

const roots: string[] = [];
afterEach(async () => Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))));

function hex(bytes: Uint8Array): `0x${string}` {
  return `0x${[...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function scalar(value: bigint): `0x${string}` {
  return `0x${value.toString(16).padStart(64, "0")}`;
}

function sha256(bytes: Uint8Array): `0x${string}` {
  return `0x${new Bun.CryptoHasher("sha256").update(bytes).digest("hex")}`;
}

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "tohseno-registry-test-"));
  roots.push(root);
  const config = loadConfig({ NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
    REGISTRY_ENABLED: "true", REGISTRY_ROOT: root, ROBINHOOD_RPC_URL: "https://rpc.example.test" });
  const source = new TextEncoder().encode("deterministic source archive");
  const privateKey = p256.utils.randomPrivateKey();
  const publicKey = p256.getPublicKey(privateKey, false);
  const release = {
    schema: "tohseno.catalog-release/1",
    generation: { contract_generation: "0.8.0", chain_id: 4663,
      builder_account_factory: config.registry.factoryAddress,
      shot_registry: config.registry.registryAddress,
      activation_signing_digest: config.registry.activationSigningDigest },
    shot_id: `0x${"11".repeat(32)}`, builder_id: `eip155:4663:0x${"22".repeat(20)}`,
    release_id: `0x${"33".repeat(32)}`, published_at: "2026-08-30T12:00:00Z",
    display: { name: "Prayer Lock", description: "A small daily ritual, made native.",
      icon_sha256: null, builder_handle: "small-maker", app_slug: "prayer-lock" },
    source: { format: "deterministic_tar", sha256: sha256(source), byte_length: source.byteLength,
      source_tree_sha256: `0x${"44".repeat(32)}`, file_count: 2, uncompressed_byte_length: 52 },
    build: { container_kind: "project", container_path: "PrayerLock.xcodeproj", scheme: "PrayerLock",
      original_bundle_identifier: "com.example.prayer-lock", minimum_ios: "17.0",
      device_families: ["iphone"], dependency_locks: [], safety: { classification: "green", reasons: [] } },
    permissions: { install_allowed: true, fork_allowed: true, distributor_rights_declared: true, spdx_license: "MIT" },
    parent: null, checkpoint_sequence: 1, public_checkpoint_digest: `0x${"55".repeat(32)}`,
  };
  const digest = sha256(new TextEncoder().encode(canonicalCatalogJSON(release)));
  const signature = p256.sign(digest.slice(2), privateKey, { prehash: false, lowS: true });
  const envelope = { schema: "tohseno.signed-catalog-release/1", release,
    signer: { x: hex(publicKey.slice(1, 33)), y: hex(publicKey.slice(33, 65)) },
    authorization: { algorithm: "p256", digest, signature: { r: scalar(signature.r), s: scalar(signature.s) }, low_s: true } };
  const verifier = { verify: async () => ({ transactionHash: `0x${"66".repeat(32)}` as const,
    blockNumber: "123", blockHash: `0x${"77".repeat(32)}` as const,
    controller: `0x${"22".repeat(20)}` as const, head: release.public_checkpoint_digest as `0x${string}`,
    checkpointSequence: 1, signerKeyID: `0x${"88".repeat(32)}` as const }),
    verifyBuilderKey: async () => {} };
  return { router: await createRegistryRouter(config, verifier), envelope, release, source,
    privateKey, publicKey, config };
}

function signedObject(
  payloadKey: "profile" | "claim",
  schema: string,
  payload: Record<string, unknown>,
  privateKey: Uint8Array,
  publicKey: Uint8Array,
) {
  const digest = sha256(new TextEncoder().encode(canonicalCatalogJSON(payload)));
  const signature = p256.sign(digest.slice(2), privateKey, { prehash: false, lowS: true });
  return { schema, [payloadKey]: payload,
    signer: { x: hex(publicKey.slice(1, 33)), y: hex(publicKey.slice(33, 65)) },
    authorization: { algorithm: "p256", digest,
      signature: { r: scalar(signature.r), s: scalar(signature.s) }, low_s: true } };
}

describe("public Registry trust bridge", () => {
  test("matches the normative generation-0.8 RegisterShot EIP-712 vector", () => {
    expect(canonicalRegistryActionDigest({ type: "REGISTER_SHOT",
      shot_id: `0x${"11".repeat(32)}`, controller: `0x${"88".repeat(20)}`,
      head: `0x${"22".repeat(32)}`, salt: `0x${"33".repeat(32)}`,
      nonce: 0, deadline: 2_000_000_000 }, `0x${"66".repeat(20)}`)).toBe(
      "0xb0bf0e838c81aeec737a617390a85a53aa8ce492bf1a4f5ac643531a0a48c9e8",
    );
  });

  test("promotes only signed, content-addressed source paired with chain evidence", async () => {
    const { router, envelope, release, source } = await fixture();
    const staged = await router.fetch(new Request("http://localhost/api/registry/v1/staging", {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ envelope }),
    }));
    expect(staged.status).toBe(201);
    const reservation = await staged.json() as Record<string, string>;
    const uploaded = await router.fetch(new Request(`http://localhost${reservation.source_url}`, {
      method: "PUT", headers: { authorization: `Bearer ${reservation.upload_token}`,
        "content-length": String(source.byteLength) }, body: source,
    }));
    expect(uploaded.status).toBe(200);
    const finalized = await router.fetch(new Request(`http://localhost${reservation.finalize_url}`, {
      method: "POST", headers: { authorization: `Bearer ${reservation.upload_token}`, "content-type": "application/json" },
      body: JSON.stringify({ transaction_hash: `0x${"66".repeat(32)}` }),
    }));
    expect(finalized.status).toBe(201);
    const catalog = await router.fetch(new Request("http://localhost/api/registry/v1/shots"));
    const page = await catalog.json() as { releases: Array<Record<string, unknown>> };
    expect(page.releases).toHaveLength(1);
    expect(page.releases[0]?.route).toBe(`/s/${String(release.shot_id).slice(2)}`);
    expect((page.releases[0]?.release as Record<string, unknown>).shot_id).toBe(release.shot_id);
    expect(await router.renderShot(release.shot_id)).toContain("Install on iPhone");
    expect(await router.renderRegistry("daily ritual")).toContain("Prayer Lock");
    expect(await router.renderRegistry("no such app")).not.toContain("Prayer Lock");
  });

  test("rejects a manifest altered after Companion signature", async () => {
    const { router, envelope } = await fixture();
    envelope.release.display.description = "substituted by server";
    const response = await router.fetch(new Request("http://localhost/api/registry/v1/staging", {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ envelope }),
    })).catch((error: Error) => new Response(error.message, { status: 422 }));
    expect(response.status).toBe(422);
  });

  test("keeps app slugs unique inside one Builder namespace", async () => {
    const { router, envelope, release, source, privateKey, publicKey } = await fixture();
    const staged = await router.fetch(new Request("http://localhost/api/registry/v1/staging", {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ envelope }),
    }));
    const reservation = await staged.json() as Record<string, string>;
    await router.fetch(new Request(`http://localhost${reservation.source_url}`, {
      method: "PUT", headers: { authorization: `Bearer ${reservation.upload_token}`,
        "content-length": String(source.byteLength) }, body: source,
    }));
    await router.fetch(new Request(`http://localhost${reservation.finalize_url}`, {
      method: "POST", headers: { authorization: `Bearer ${reservation.upload_token}`,
        "content-type": "application/json" },
      body: JSON.stringify({ transaction_hash: `0x${"66".repeat(32)}` }),
    }));

    const conflicting = structuredClone(release);
    conflicting.shot_id = `0x${"12".repeat(32)}`;
    conflicting.release_id = `0x${"34".repeat(32)}`;
    conflicting.public_checkpoint_digest = `0x${"56".repeat(32)}`;
    const digest = sha256(new TextEncoder().encode(canonicalCatalogJSON(conflicting)));
    const signature = p256.sign(digest.slice(2), privateKey, { prehash: false, lowS: true });
    const conflictingEnvelope = { schema: "tohseno.signed-catalog-release/1", release: conflicting,
      signer: { x: hex(publicKey.slice(1, 33)), y: hex(publicKey.slice(33, 65)) },
      authorization: { algorithm: "p256", digest,
        signature: { r: scalar(signature.r), s: scalar(signature.s) }, low_s: true } };
    const response = await router.fetch(new Request("http://localhost/api/registry/v1/staging", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ envelope: conflictingEnvelope }),
    })).catch((error: Error) => new Response(error.message, { status: 409 }));
    expect(response.status).toBe(409);
  });

  test("accepts only signed monotonic Builder profiles and permissioned alias requests", async () => {
    const { router, envelope, release, source, privateKey, publicKey } = await fixture();
    const staged = await router.fetch(new Request("http://localhost/api/registry/v1/staging", {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ envelope }),
    }));
    const reservation = await staged.json() as Record<string, string>;
    await router.fetch(new Request(`http://localhost${reservation.source_url}`, {
      method: "PUT", headers: { authorization: `Bearer ${reservation.upload_token}`,
        "content-length": String(source.byteLength) }, body: source,
    }));
    await router.fetch(new Request(`http://localhost${reservation.finalize_url}`, {
      method: "POST", headers: { authorization: `Bearer ${reservation.upload_token}`,
        "content-type": "application/json" },
      body: JSON.stringify({ transaction_hash: `0x${"66".repeat(32)}` }),
    }));

    const now = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
    const profile = { schema: "tohseno.builder-profile/1", builder_id: release.builder_id,
      display_name: "Small Maker", handle: "small-maker", avatar_sha256: null,
      external_attestations: [], updated_at: now, nonce: 1 };
    const profileEnvelope = signedObject("profile", "tohseno.signed-builder-profile/1",
      profile, privateKey, publicKey);
    const updated = await router.fetch(new Request(
      `http://localhost/api/registry/v1/builders/${release.builder_id}/profile`, {
        method: "PUT", headers: { "content-type": "application/json" },
        body: JSON.stringify({ envelope: profileEnvelope }),
      }));
    expect(updated.status).toBe(200);
    const builder = await (await router.fetch(new Request(
      `http://localhost/api/registry/v1/builders/${release.builder_id}`))).json() as Record<string, unknown>;
    expect((builder.profile as Record<string, unknown>).display_name).toBe("Small Maker");
    expect(await router.renderHumanRoute("/@small-maker/prayer-lock")).toContain("Install on iPhone");
    const replay = await router.fetch(new Request(
      `http://localhost/api/registry/v1/builders/${release.builder_id}/profile`, {
        method: "PUT", headers: { "content-type": "application/json" },
        body: JSON.stringify({ envelope: profileEnvelope }),
      })).catch((error: Error) => new Response(error.message, { status: 409 }));
    expect(replay.status).toBe(409);

    const claim = { schema: "tohseno.alias-claim/1", builder_id: release.builder_id,
      shot_id: release.shot_id, alias: "prayer", request_id: `0x${"99".repeat(32)}`,
      nonce: 1, deadline: Math.floor(Date.now() / 1000) + 900, requested_at: now };
    const claimEnvelope = signedObject("claim", "tohseno.signed-alias-claim/1",
      claim, privateKey, publicKey);
    const claimed = await router.fetch(new Request("http://localhost/api/registry/v1/aliases/claims", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ envelope: claimEnvelope }),
    }));
    expect(claimed.status).toBe(202);
    expect((await claimed.json() as Record<string, unknown>).status).toBe("pending_policy_review");
    const claimReplay = await router.fetch(new Request("http://localhost/api/registry/v1/aliases/claims", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ envelope: claimEnvelope }),
    })).catch((error: Error) => new Response(error.message, { status: 409 }));
    expect(claimReplay.status).toBe(409);
  });

  test("rejects source bytes whose declared length differs from the signed release", async () => {
    const { router, envelope, source } = await fixture();
    const staged = await router.fetch(new Request("http://localhost/api/registry/v1/staging", {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ envelope }),
    }));
    const reservation = await staged.json() as Record<string, string>;
    const response = await router.fetch(new Request(`http://localhost${reservation.source_url}`, {
      method: "PUT", headers: { authorization: `Bearer ${reservation.upload_token}`,
        "content-length": String(source.byteLength - 1) }, body: source.slice(0, -1),
    })).catch((error: Error) => new Response(error.message, { status: 422 }));
    expect(response.status).toBe(422);
  });

  test("rejects duplicate JSON members before signature validation", async () => {
    const { router } = await fixture();
    const response = await router.fetch(new Request("http://localhost/api/registry/v1/staging", {
      method: "POST", headers: { "content-type": "application/json" },
      body: `{"envelope":{},"envel\\u006fpe":{}}`,
    })).catch((error: Error) => new Response(error.message, { status: 400 }));
    expect(response.status).toBe(400);
    expect(await response.text()).toContain("duplicate-free");
  });
});
