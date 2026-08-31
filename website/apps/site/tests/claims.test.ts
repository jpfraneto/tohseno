import { afterEach, describe, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { p256 } from "@noble/curves/p256";
import { keccak_256 as keccak } from "@noble/hashes/sha3";
import { loadConfig, RELEASED_CLAIMS_ACTIVATION } from "../config.ts";
import {
  createClaimsRouter,
  canonicalClaimsActionDigest,
  RobinhoodClaimsReader,
  validateCanonicalClaimMark,
  type ClaimEditionSnapshot,
  type ClaimsLiveStatus,
  type ClaimsReader,
  type ClaimsWriter,
  type SoftwareClaimSnapshot,
  type VerifiedActivation,
} from "../src/claims.ts";
import { canonicalCatalogJSON } from "../src/registry.ts";

const roots: string[] = [];
afterEach(async () => Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))));

type Hex = `0x${string}`;
const CLAIMS = `0x${"66".repeat(20)}` as Hex;
const REGISTRY = "0x3fe6508ba2660bc575080024f402c192a2e035a0" as Hex;
const RUNTIME = `0x${"22".repeat(32)}` as Hex;
const SHOT = `0x${"11".repeat(32)}` as Hex;
const CLAIMANT = `0x${"44".repeat(20)}` as Hex;

function bytesHex(bytes: Uint8Array): Hex {
  return `0x${[...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function scalar(value: bigint): Hex {
  return `0x${value.toString(16).padStart(64, "0")}`;
}

function sha256(bytes: Uint8Array): Hex {
  return `0x${new Bun.CryptoHasher("sha256").update(bytes).digest("hex")}`;
}

async function activatedConfig(tamper = false) {
  const root = await mkdtemp(join(tmpdir(), "tohseno-claims-test-"));
  roots.push(root);
  const keys = [1, 2, 3].map((seed) => {
    const privateKey = new Uint8Array(32).fill(seed);
    const publicKey = p256.getPublicKey(privateKey, false);
    const x = bytesHex(publicKey.slice(1, 33));
    const y = bytesHex(publicKey.slice(33, 65));
    const domain = new TextEncoder().encode("TOHSENO-RELEASE-AUTHORITY-KEY-V1\0");
    const material = new Uint8Array(domain.length + 64);
    material.set(domain); material.set(publicKey.slice(1), domain.length);
    return { privateKey, x, y, keyID: sha256(material) };
  }).sort((left, right) => left.keyID.localeCompare(right.keyID));
  const policy = {
    schema: "tohseno.release-authority-policy/1", protocol: "tohseno", protocol_major: 2,
    purpose: "contract_generation_activation", threshold: 2,
    authorities: keys.map((key) => ({ key_id: key.keyID, public_key: { x: key.x, y: key.y } })),
    issued_at: "2026-08-30T12:00:00Z",
  };
  const activation = {
    schema: "tohseno.claims-activation/1", protocol: "tohseno", component: "TohsenoClaimsV1",
    contract_version: 1, activation_sequence: 1, previous_activation: null,
    authority_policy_sha256: sha256(new TextEncoder().encode(canonicalCatalogJSON(policy))),
    chain_id: 4663, claims_contract: CLAIMS, shot_registry: REGISTRY,
    creation_code_keccak256: `0x${"10".repeat(32)}`, runtime_code_keccak256: RUNTIME,
    source_commit: "a".repeat(40), source_tree_sha256: `0x${"33".repeat(32)}`,
    deployment: { transaction_hash: `0x${"44".repeat(32)}`, block_number: 12_345,
      block_hash: `0x${"55".repeat(32)}` }, issued_at: "2026-08-30T13:00:00Z",
  };
  const canonical = new TextEncoder().encode(canonicalCatalogJSON(activation));
  const domain = new TextEncoder().encode("TOHSENO-CLAIMS-ACTIVATION-V1\0");
  const material = new Uint8Array(domain.length + canonical.length);
  material.set(domain); material.set(canonical, domain.length);
  const digest = sha256(material);
  const approvals = keys.slice(0, 2).map((key) => {
    const signature = p256.sign(digest.slice(2), key.privateKey, { prehash: false, lowS: true });
    return { key_id: key.keyID, authorization: { algorithm: "p256", digest,
      signature: { r: scalar(signature.r), s: scalar(signature.s) }, low_s: true } };
  });
  const signed = { schema: "tohseno.signed-claims-activation/1",
    activation: tamper ? { ...activation, runtime_code_keccak256: `0x${"99".repeat(32)}` } : activation,
    approvals };
  const policyPath = join(root, "policy.json");
  const activationPath = join(root, "activation.json");
  await writeFile(policyPath, `${canonicalCatalogJSON(policy)}\n`);
  await writeFile(activationPath, `${canonicalCatalogJSON(signed)}\n`);
  const config = loadConfig({ NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
    REGISTRY_ENABLED: "true", REGISTRY_ROOT: root, ROBINHOOD_RPC_URL: "https://rpc.example.test",
    CLAIMS_CONTRACT_ADDRESS: CLAIMS, CLAIMS_ACTIVATION_SIGNING_DIGEST: digest,
    CLAIMS_ACTIVATION_EVIDENCE_PATH: activationPath, CLAIMS_AUTHORITY_POLICY_PATH: policyPath,
    CLAIMS_DEPLOYMENT_BLOCK: "12345", CLAIMS_INDEXER_ENABLED: "true" });
  return config;
}

class FixtureReader implements ClaimsReader {
  statusCalls = 0;
  readonly receipt: SoftwareClaimSnapshot = { tokenID: 7n, shotID: SHOT, claimNumber: 3n,
    claimant: CLAIMANT, releaseDigest: `0x${"55".repeat(32)}`,
    checkpointDigest: `0x${"77".repeat(32)}`, gestureCommitment: `0x${"88".repeat(32)}`,
    transactionHash: `0x${"99".repeat(32)}`, blockNumber: 12_400n, blockHash: `0x${"aa".repeat(32)}` };

  async liveStatus(): Promise<ClaimsLiveStatus> {
    this.statusCalls += 1;
    return { runtimeCodeKeccak256: RUNTIME, shotRegistry: REGISTRY };
  }
  async edition(): Promise<ClaimEditionSnapshot> { return { opened: true, maxClaims: 888n,
    totalClaims: 3n, openedAt: 2_000_000_000n, closesAt: 0n, closed: false }; }
  async tokenFor(_shot: Hex, claimant: Hex): Promise<bigint> { return claimant === CLAIMANT ? 7n : 0n; }
  async claim(): Promise<SoftwareClaimSnapshot> { return this.receipt; }
  async claimsForShot(): Promise<SoftwareClaimSnapshot[]> { return [this.receipt]; }
}

describe("separately activated Claims Registry", () => {
  test("production accepts only the released Claims coordinates", () => {
    const common = {
      NODE_ENV: "production", PORT: "3000", BASE_URL: "https://tohseno.com",
      CLAIMS_ACTIVATION_EVIDENCE_PATH: "/evidence/activation.json",
      CLAIMS_AUTHORITY_POLICY_PATH: "/evidence/policy.json",
    };
    expect(() => loadConfig({ ...common,
      CLAIMS_CONTRACT_ADDRESS: CLAIMS,
      CLAIMS_ACTIVATION_SIGNING_DIGEST: RELEASED_CLAIMS_ACTIVATION.signingDigest,
      CLAIMS_DEPLOYMENT_BLOCK: RELEASED_CLAIMS_ACTIVATION.deploymentBlock.toString(),
    })).toThrow("production Claims coordinates differ from the released signed activation");
    const config = loadConfig({ ...common,
      CLAIMS_CONTRACT_ADDRESS: RELEASED_CLAIMS_ACTIVATION.contractAddress,
      CLAIMS_ACTIVATION_SIGNING_DIGEST: RELEASED_CLAIMS_ACTIVATION.signingDigest,
      CLAIMS_DEPLOYMENT_BLOCK: RELEASED_CLAIMS_ACTIVATION.deploymentBlock.toString(),
    });
    expect(config.claims.contractAddress).toBe(RELEASED_CLAIMS_ACTIVATION.contractAddress);
  });

  test("rebuilds its durable index on restart and refuses a moving canonical head", async () => {
    const config = await activatedConfig();
    const activation: VerifiedActivation = {
      signingDigest: config.claims.activationSigningDigest!, claimsContract: CLAIMS,
      shotRegistry: REGISTRY, runtimeCodeKeccak256: RUNTIME, deploymentBlock: 12_345n,
      deploymentTransaction: `0x${"44".repeat(32)}`, sourceCommit: "a".repeat(40),
    };
    const head = { number: 12_500n, hash: `0x${"ab".repeat(32)}` as Hex,
      timestamp: 2_000_000_000n };
    let scans = 0;
    const stableClient = {
      async getBlock(argument: { blockTag?: string; blockNumber?: bigint }) {
        expect(argument.blockTag === "latest" || argument.blockNumber === head.number).toBe(true);
        return head;
      },
      async getLogs() { scans += 1; return []; },
    };
    await new RobinhoodClaimsReader(config, activation, stableClient as never).claimsForShot(SHOT);
    await new RobinhoodClaimsReader(config, activation, stableClient as never).claimsForShot(SHOT);
    expect(scans).toBe(2);
    const state = JSON.parse(await readFile(join(config.registry.root!, "claims-v1/index.json"), "utf8")) as {
      schema: string; claims_contract: string; claimsContract: string; indexedThrough: { hash: string };
    };
    expect(state.schema).toBe("tohseno.claims-index/1");
    expect(state.claimsContract).toBe(CLAIMS);
    expect(state.indexedThrough.hash).toBe(head.hash);

    const reorganizingClient = {
      async getBlock(argument: { blockTag?: string }) {
        return argument.blockTag === "latest" ? head
          : { ...head, hash: `0x${"cd".repeat(32)}` as Hex };
      },
      async getLogs() { return []; },
    };
    await expect(new RobinhoodClaimsReader(
      config, activation, reorganizingClient as never,
    ).claimsForShot(SHOT)).rejects.toThrow("reorganized while Claims were indexing");
  });

  test("derives finite and timed edition closure from canonical evidence", async () => {
    const config = await activatedConfig();
    const filled: SoftwareClaimSnapshot = { tokenID: 3n, shotID: SHOT, claimNumber: 3n,
      claimant: CLAIMANT, releaseDigest: `0x${"55".repeat(32)}`,
      checkpointDigest: `0x${"77".repeat(32)}`, gestureCommitment: `0x${"88".repeat(32)}`,
      transactionHash: `0x${"99".repeat(32)}`, blockNumber: 12_500n,
      blockHash: `0x${"aa".repeat(32)}`, claimedAt: "2026-08-31T01:00:00.000Z",
      transactionIndex: 4, logIndex: 7 };
    const reader: ClaimsReader = {
      async liveStatus() { return { runtimeCodeKeccak256: RUNTIME, shotRegistry: REGISTRY }; },
      async edition() { return { opened: true, maxClaims: 3n, totalClaims: 3n,
        openedAt: 1n, closesAt: 0n, closed: true }; },
      async tokenFor() { return 0n; }, async claim() { return filled; },
      async claimsForShot() { return [filled]; },
    };
    const router = await createClaimsRouter(config, reader);
    expect(await router.closureForTimeline(SHOT)).toEqual({ reason: "supply_filled",
      occurredAt: "2026-08-31T01:00:00Z", canonicalBlock: { number: "12500",
        hash: filled.blockHash!, transactionIndex: 4, logIndex: 7 } });

    const timedReader: ClaimsReader = {
      async liveStatus() { return { runtimeCodeKeccak256: RUNTIME, shotRegistry: REGISTRY }; },
      async edition() { return { opened: true, maxClaims: 0n, totalClaims: 0n,
        openedAt: 1n, closesAt: 2_000_000_000n, closed: true }; },
      async tokenFor() { return 0n; }, async claim() { throw new Error("not used"); },
      async claimsForShot() { return []; },
      async canonicalBlockAtOrAfter(timestamp) {
        expect(timestamp).toBe(2_000_000_000n);
        return { number: 12_600n, hash: `0x${"bb".repeat(32)}`,
          timestamp: "2033-05-18T03:33:20.000Z" };
      } };
    const timedRouter = await createClaimsRouter(config, timedReader);
    expect(await timedRouter.closureForTimeline(SHOT)).toEqual({ reason: "time_elapsed",
      occurredAt: "2033-05-18T03:33:20Z", canonicalBlock: { number: "12600",
        hash: `0x${"bb".repeat(32)}`, transactionIndex: null, logIndex: null } });
  });

  test("matches the frozen Solidity, Rust, and Swift Claims EIP-712 vectors", async () => {
    const fixture = JSON.parse(await readFile(join(import.meta.dir, "../../../../fixtures/claim-actions-v1.json"), "utf8")) as {
      domain: { verifying_contract: Hex };
      open_claim_edition: { action: Record<string, unknown>; digest: Hex };
      claim_software: { action: Record<string, unknown>; digest: Hex };
    };
    expect(canonicalClaimsActionDigest({ type: "OPEN_CLAIM_EDITION",
      ...fixture.open_claim_edition.action } as Parameters<typeof canonicalClaimsActionDigest>[0],
    fixture.domain.verifying_contract)).toBe(fixture.open_claim_edition.digest);
    expect(canonicalClaimsActionDigest({ type: "CLAIM_SOFTWARE",
      ...fixture.claim_software.action } as Parameters<typeof canonicalClaimsActionDigest>[0],
    fixture.domain.verifying_contract)).toBe(fixture.claim_software.digest);
  });

  test("relays only the exact DeviceKey-approved first-Ship edition and verifies canonical completion", async () => {
    const config = await activatedConfig();
    config.claims.relayerEnabled = true;
    config.claims.indexerEnabled = true;
    config.claims.relayerPrivateKey = `0x${"01".repeat(32)}`;
    let confirmed = false;
    const reader: ClaimsReader = {
      async liveStatus() { return { runtimeCodeKeccak256: RUNTIME, shotRegistry: REGISTRY,
        relayerAddress: `0x${"aa".repeat(20)}`, relayerBalance: 1n }; },
      async edition() { return { opened: confirmed, maxClaims: confirmed ? 888n : 0n,
        totalClaims: 0n, openedAt: confirmed ? 100n : 0n, closesAt: 0n, closed: false }; },
      async tokenFor() { return 0n; },
      async claim() { throw new Error("not used"); },
      async claimsForShot() { return []; },
    };
    const transaction = `0x${"ab".repeat(32)}` as Hex;
    const writer: ClaimsWriter = {
      async editionNonce() { return 0n; },
      async claimNonce() { return 0n; },
      async accountState() { return { address: controller, deployed: true }; },
      async submitAccountBootstrap() { return transaction; },
      async submitOpenEdition() { return transaction; },
      async submitClaim() { return transaction; },
      async transactionConfirmed(hash) { expect(hash).toBe(transaction); confirmed = true; return true; },
    };
    const privateKey = new Uint8Array(32).fill(7);
    const publicKey = p256.getPublicKey(privateKey, false);
    const x = bytesHex(publicKey.slice(1, 33));
    const y = bytesHex(publicKey.slice(33, 65));
    const controller = `0x${"22".repeat(20)}` as Hex;
    const deadline = Math.floor(Date.now() / 1000) + 3600;
    const action = { type: "OPEN_CLAIM_EDITION" as const, shot_registry: REGISTRY,
      shot_id: SHOT, max_claims: 888, closes_at: 0, controller, nonce: 0, deadline };
    const digest = canonicalClaimsActionDigest(action, CLAIMS);
    const signed = p256.sign(digest.slice(2), privateKey, { prehash: false, lowS: true });
    const approval = { policy: { kind: "limited", max_claims: 888, closes_at: 0 },
      action: { shot_registry: REGISTRY, shot_id: SHOT, max_claims: 888, closes_at: 0,
        controller, nonce: 0, deadline }, digest,
      signature: { schema: "tohseno.builder-device-signature/1",
        signer: { schema: "tohseno.builder-device-announcement/1", key_id: `0x${"33".repeat(32)}`,
          x, y, security_level: "secure_enclave", test_only: false },
        algorithm: "p256", digest, r: scalar(signed.r), s: scalar(signed.s), low_s: true } };
    const envelope = { release: { checkpoint_sequence: 1, shot_id: SHOT,
      builder_id: `eip155:4663:${controller}` }, signer: { x, y } };
    const router = await createClaimsRouter(config, reader, writer);
    await router.verifyOpenEdition(approval, envelope);
    const submitted = await router.advanceOpenEdition(approval, envelope);
    expect(submitted).toEqual({ transactionHash: transaction, confirmed: false });
    const completed = await router.advanceOpenEdition(approval, envelope, transaction);
    expect(completed).toEqual({ transactionHash: transaction, confirmed: true });
    await expect(router.verifyOpenEdition({ ...approval, digest: `0x${"ff".repeat(32)}` }, envelope)).rejects.toThrow();
  });

  test("persists account bootstrap and Software Claim before reporting the canonical receipt", async () => {
    const config = await activatedConfig();
    config.claims.relayerEnabled = true;
    config.claims.indexerEnabled = true;
    config.claims.relayerPrivateKey = `0x${"01".repeat(32)}`;
    const fixture = JSON.parse(await readFile(join(import.meta.dir, "../../../../fixtures/claim-mark-v1.json"), "utf8")) as {
      vectors: Array<{ accepted: boolean; canonical_hex: string; gesture_commitment: Hex }>;
    };
    const mark = fixture.vectors.find((value) => value.accepted)!;
    const privateKey = new Uint8Array(32).fill(9);
    const publicKey = p256.getPublicKey(privateKey, false);
    const x = bytesHex(publicKey.slice(1, 33));
    const y = bytesHex(publicKey.slice(33, 65));
    const keyID = bytesHex(keccak(publicKey.slice(1)));
    let deployed = false;
    let claimed = false;
    const accountTransaction = `0x${"aa".repeat(32)}` as Hex;
    const claimTransaction = `0x${"bb".repeat(32)}` as Hex;
    const receipt: SoftwareClaimSnapshot = { tokenID: 1n, shotID: SHOT, claimNumber: 1n,
      claimant: CLAIMANT, releaseDigest: `0x${"55".repeat(32)}`,
      checkpointDigest: `0x${"77".repeat(32)}`, gestureCommitment: mark.gesture_commitment,
      transactionHash: claimTransaction, blockNumber: 12_401n, blockHash: `0x${"cc".repeat(32)}` };
    const reader: ClaimsReader = {
      async liveStatus() { return { runtimeCodeKeccak256: RUNTIME, shotRegistry: REGISTRY,
        relayerAddress: `0x${"dd".repeat(20)}`, relayerBalance: 1n }; },
      async edition() { return { opened: true, maxClaims: 888n, totalClaims: claimed ? 1n : 0n,
        openedAt: 100n, closesAt: 0n, closed: false }; },
      async tokenFor() { return claimed ? 1n : 0n; },
      async claim() { return receipt; },
      async claimsForShot() { return claimed ? [receipt] : []; },
    };
    const writer: ClaimsWriter = {
      async editionNonce() { return 0n; },
      async claimNonce() { return 0n; },
      async accountState() { return { address: CLAIMANT, deployed }; },
      async submitAccountBootstrap() { return accountTransaction; },
      async submitOpenEdition() { throw new Error("not used"); },
      async submitClaim() { return claimTransaction; },
      async transactionConfirmed(hash, target) {
        if (hash === accountTransaction) { expect(target).toBe(config.registry.factoryAddress); deployed = true; }
        if (hash === claimTransaction) { expect(target).toBe(CLAIMS); claimed = true; }
        return true;
      },
    };
    const claimContext = { shotID: SHOT,
      builderID: `eip155:4663:0x${"22".repeat(20)}`, releaseDigest: receipt.releaseDigest,
      checkpointDigest: receipt.checkpointDigest, checkpointSequence: 1, appName: "Prayer Lock",
      appDescription: "A small ritual.", sourceURL: "/source", canonicalBlock: {
        number: "123", hash: `0x${"ee".repeat(32)}` as Hex } };
    const catalog = { async currentClaimContext() { return claimContext; },
      async claimReceiptContext() { return claimContext; } };
    const router = await createClaimsRouter(config, reader, writer, catalog);
    const device = { schema: "tohseno.builder-device-announcement/1", key_id: keyID,
      x, y, security_level: "secure_enclave", test_only: false };
    const preparedResponse = await router.fetch(new Request(`http://localhost/api/registry/v1/shots/${SHOT}/claims/prepare`, {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({
        release_digest: receipt.releaseDigest, claimant: CLAIMANT,
        claim_mark: mark.canonical_hex, builder_device: device,
      }),
    }));
    expect(preparedResponse.status).toBe(201);
    const prepared = await preparedResponse.json() as Record<string, unknown>;
    const action = { type: "CLAIM_SOFTWARE" as const, shot_registry: REGISTRY, shot_id: SHOT,
      claimant: CLAIMANT, release_digest: receipt.releaseDigest,
      checkpoint_digest: receipt.checkpointDigest, gesture_commitment: mark.gesture_commitment,
      nonce: prepared.nonce as number, deadline: prepared.deadline as number };
    const digest = canonicalClaimsActionDigest(action, CLAIMS);
    const signature = p256.sign(digest.slice(2), privateKey, { prehash: false, lowS: true });
    const authorization = { action: { shot_registry: action.shot_registry, shot_id: action.shot_id,
      claimant: action.claimant, release_digest: action.release_digest,
      checkpoint_digest: action.checkpoint_digest, gesture_commitment: action.gesture_commitment,
      nonce: action.nonce, deadline: action.deadline }, digest,
      signature: { schema: "tohseno.builder-device-signature/1", signer: device,
        algorithm: "p256", digest, r: scalar(signature.r), s: scalar(signature.s), low_s: true } };
    const headers = { authorization: `Bearer ${prepared.job_token as string}`, "content-type": "application/json" };
    const submitted = await router.fetch(new Request(`http://localhost/api/registry/v1/claims/jobs/${prepared.job_id as string}/submit`, {
      method: "POST", headers, body: JSON.stringify(authorization),
    }));
    expect(submitted.status).toBe(202);
    const statusURL = `http://localhost/api/registry/v1/claims/jobs/${prepared.job_id as string}`;
    const first = await router.fetch(new Request(statusURL, { headers }));
    expect((await first.json() as Record<string, unknown>).status).toBe("account_pending");
    const second = await router.fetch(new Request(statusURL, { headers }));
    expect((await second.json() as Record<string, unknown>).status).toBe("claim_submitted");
    const third = await router.fetch(new Request(statusURL, { headers }));
    const complete = await third.json() as Record<string, unknown>;
    expect(third.status).toBe(200);
    expect(complete.status).toBe("complete");
    expect((complete.claim as Record<string, unknown>).token_id).toBe("1");
  });

  test("rejects duplicate JSON members and bounds Claim preparation per source", async () => {
    const fixture = JSON.parse(await readFile(
      join(import.meta.dir, "../../../../fixtures/claim-mark-v1.json"), "utf8",
    )) as { vectors: Array<{ accepted: boolean; canonical_hex: string }> };
    const mark = fixture.vectors.find((value) => value.accepted)!;
    const privateKey = new Uint8Array(32).fill(13);
    const publicKey = p256.getPublicKey(privateKey, false);
    const device = { schema: "tohseno.builder-device-announcement/1",
      key_id: bytesHex(keccak(publicKey.slice(1))), x: bytesHex(publicKey.slice(1, 33)),
      y: bytesHex(publicKey.slice(33, 65)), security_level: "secure_enclave", test_only: false };
    const releaseDigest = `0x${"55".repeat(32)}` as Hex;
    const checkpointDigest = `0x${"77".repeat(32)}` as Hex;
    const config = await activatedConfig();
    config.claims.relayerEnabled = true;
    config.claims.indexerEnabled = true;
    config.claims.relayerPrivateKey = `0x${"01".repeat(32)}`;
    const reader: ClaimsReader = {
      async liveStatus() { return { runtimeCodeKeccak256: RUNTIME, shotRegistry: REGISTRY,
        relayerAddress: `0x${"dd".repeat(20)}`, relayerBalance: 1n }; },
      async edition() { return { opened: true, maxClaims: 0n, totalClaims: 0n,
        openedAt: 1n, closesAt: 0n, closed: false }; },
      async tokenFor() { return 0n; }, async claim() { throw new Error("not used"); },
      async claimsForShot() { return []; },
    };
    const writer: ClaimsWriter = {
      async editionNonce() { return 0n; }, async claimNonce() { return 0n; },
      async accountState() { return { address: CLAIMANT, deployed: true }; },
      async submitAccountBootstrap() { throw new Error("not used"); },
      async submitOpenEdition() { throw new Error("not used"); },
      async submitClaim() { throw new Error("not used"); },
      async transactionConfirmed() { return false; },
    };
    const context = { shotID: SHOT, builderID: `eip155:4663:0x${"22".repeat(20)}`,
      releaseDigest, checkpointDigest, checkpointSequence: 1, appName: "Prayer Lock",
      appDescription: "A small ritual.", sourceURL: "/source",
      canonicalBlock: { number: "123", hash: `0x${"ee".repeat(32)}` as Hex } };
    const catalog = { async currentClaimContext() { return context; },
      async claimReceiptContext() { return context; } };
    const body = { release_digest: releaseDigest, claimant: CLAIMANT,
      claim_mark: mark.canonical_hex, builder_device: device };
    const url = `http://localhost/api/registry/v1/shots/${SHOT}/claims/prepare`;
    const duplicateRouter = await createClaimsRouter(config, reader, writer, catalog);
    const duplicate = `{"release_digest":"${releaseDigest}","release_digest":"${releaseDigest}",`
      + `"claimant":"${CLAIMANT}","claim_mark":"${mark.canonical_hex}",`
      + `"builder_device":${JSON.stringify(device)}}`;
    await expect(duplicateRouter.fetch(new Request(url, { method: "POST",
      headers: { "content-type": "application/json" }, body: duplicate })))
      .rejects.toMatchObject({ status: 400 });

    const rateRouter = await createClaimsRouter(config, reader, writer, catalog);
    for (let index = 0; index < 12; index += 1) {
      const response = await rateRouter.fetch(new Request(url, { method: "POST",
        headers: { "content-type": "application/json" }, body: JSON.stringify(body) }));
      expect(response.status).toBe(201);
    }
    await expect(rateRouter.fetch(new Request(url, { method: "POST",
      headers: { "content-type": "application/json" }, body: JSON.stringify(body) })))
      .rejects.toMatchObject({ status: 429 });
  });

  test("validates the exact Rust and Swift Claim mark fixture including accessibility", async () => {
    const fixture = JSON.parse(await readFile(join(import.meta.dir, "../../../../fixtures/claim-mark-v1.json"), "utf8")) as {
      vectors: Array<{ id: string; accepted: boolean; canonical_hex: string | null; gesture_commitment: string | null }>;
    };
    for (const vector of fixture.vectors.filter((value) => value.accepted)) {
      const mark = validateCanonicalClaimMark(vector.canonical_hex!, vector.gesture_commitment!);
      expect(mark.gestureCommitment).toBe(vector.gesture_commitment! as Hex);
      expect(mark.points).toHaveLength(64);
      expect(mark.kind).toBe(vector.id === "accessibility-hold" ? "accessibility_hold" : "drawn");
    }
    const accessibility = fixture.vectors.find((value) => value.id === "accessibility-hold")!;
    const altered = `${accessibility.canonical_hex!.slice(0, -2)}01`;
    expect(() => validateCanonicalClaimMark(altered)).toThrow("fixed representation");
  });

  test("stays explicitly dark when no Claims activation exists", async () => {
    const config = loadConfig({ NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000" });
    const router = await createClaimsRouter(config);
    const response = await router.fetch(new Request("http://localhost/api/registry/v1/claims/status"));
    expect(response.status).toBe(503);
    const status = await response.json() as Record<string, unknown>;
    expect(status.configured).toBe(false);
    expect(status.activation_verified).toBe(false);
    expect(status.contract_code_verified).toBe(false);
    expect((status.relayer as Record<string, unknown>).enabled).toBe(false);
  });

  test("serves editions, account state, receipts, pagination, and token metadata only after exact activation", async () => {
    const reader = new FixtureReader();
    const context = { shotID: SHOT, builderID: `eip155:4663:0x${"22".repeat(20)}`,
      releaseDigest: reader.receipt.releaseDigest, checkpointDigest: reader.receipt.checkpointDigest,
      checkpointSequence: 1, appName: "Prayer Lock", appDescription: "A small ritual.",
      sourceURL: "/source", canonicalBlock: { number: "123", hash: `0x${"ee".repeat(32)}` as Hex } };
    const router = await createClaimsRouter(await activatedConfig(), reader, undefined, {
      async currentClaimContext() { return context; }, async claimReceiptContext() { return context; },
    });
    const status = await router.fetch(new Request("http://localhost/api/registry/v1/claims/status"));
    expect(status.status).toBe(200);
    expect((await status.json() as Record<string, unknown>).contract_code_verified).toBe(true);

    const edition = await (await router.fetch(new Request(
      `http://localhost/api/registry/v1/shots/${SHOT}/claim-edition`,
    ))).json() as Record<string, unknown>;
    expect((edition.policy as Record<string, unknown>).kind).toBe("limited");
    expect((edition.policy as Record<string, unknown>).max_claims).toBe("888");
    expect(edition.total_claims).toBe("3");

    const state = await (await router.fetch(new Request(
      `http://localhost/api/registry/v1/shots/${SHOT}/claims/${CLAIMANT}`,
    ))).json() as Record<string, unknown>;
    expect(state.claimed).toBe(true);
    expect((state.claim as Record<string, unknown>).token_id).toBe("7");

    const page = await (await router.fetch(new Request(
      `http://localhost/api/registry/v1/shots/${SHOT}/claims?limit=1`,
    ))).json() as { claims: Array<Record<string, unknown>> };
    expect(page.claims).toHaveLength(1);
    expect(page.claims[0]?.claim_number).toBe("3");

    const receipt = await (await router.fetch(new Request(
      "http://localhost/api/registry/v1/claims/7",
    ))).json() as Record<string, unknown>;
    expect(receipt.transferable).toBe(false);
    const metadata = await (await router.fetch(new Request(
      "http://localhost/api/claims/v1/token/7",
    ))).json() as Record<string, unknown>;
    expect(metadata.name).toBe("Prayer Lock · Claim #3");
    expect(metadata.description).not.toContain("marketplace");
    expect(reader.statusCalls).toBe(1);
  });

  test("rejects altered activation before consulting or advertising live state", async () => {
    const reader = new FixtureReader();
    const router = await createClaimsRouter(await activatedConfig(true), reader);
    const response = await router.fetch(new Request("http://localhost/api/registry/v1/claims/status"));
    expect(response.status).toBe(503);
    const status = await response.json() as Record<string, unknown>;
    expect(status.activation_verified).toBe(false);
    expect(status.contract_code_verified).toBe(false);
    expect(reader.statusCalls).toBe(0);
  });
});
