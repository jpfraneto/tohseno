import { lstat, mkdir, open, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { p256 } from "@noble/curves/p256";
import { keccak_256 } from "@noble/hashes/sha3";
import {
  createPublicClient,
  createWalletClient,
  decodeEventLog,
  defineChain,
  http,
  parseAbi,
  type Hex,
  type PublicClient,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import type { AppConfig } from "../config.ts";
import type { ClaimCatalogContext, ClaimsPublicationBridge } from "./claims.ts";
import { HttpError, withSecurityHeaders } from "./security.ts";

const RELEASE_SCHEMA = "tohseno.catalog-release/1";
const SIGNED_SCHEMA = "tohseno.signed-catalog-release/1";
const STAGING_SCHEMA = "tohseno.catalog-staging/1";
const RECORD_SCHEMA = "tohseno.catalog-record/1";
const PROFILE_SCHEMA = "tohseno.builder-profile/1";
const SIGNED_PROFILE_SCHEMA = "tohseno.signed-builder-profile/1";
const ALIAS_CLAIM_SCHEMA = "tohseno.alias-claim/1";
const SIGNED_ALIAS_CLAIM_SCHEMA = "tohseno.signed-alias-claim/1";
const MAX_SOURCE_BYTES = 512 * 1024 * 1024;
const MAX_ICON_BYTES = 5 * 1024 * 1024;
const HEX32 = /^0x[0-9a-f]{64}$/;
const ADDRESS = /^0x[0-9a-f]{40}$/;
const IDENTIFIER = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const HALF_ORDER = BigInt("0x7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8");

const REGISTRY_ABI = parseAbi([
  "function getShot(bytes32 shotId) view returns ((address controller, bytes32 head, uint64 checkpointSequence, uint64 nonce))",
  "event ShotRegistered(bytes32 indexed shotId,address indexed controller,bytes32 indexed head,bytes32 commitment,uint64 checkpointSequence,uint64 actionNonce,address relayer)",
  "event CheckpointAppended(bytes32 indexed shotId,bytes32 indexed previousHead,bytes32 indexed newHead,uint64 checkpointSequence,uint64 actionNonce,address relayer)",
]);
const ACCOUNT_ABI = parseAbi([
  "function isAuthorizedKey(bytes32 keyId) view returns (bool)",
]);
const FACTORY_ABI = parseAbi([
  "function createAccount(bytes32 salt,uint256 initialX,uint256 initialY) returns (address account)",
  "function predictAccount(bytes32 salt,uint256 initialX,uint256 initialY) view returns (address predicted)",
]);
const RELAYER_REGISTRY_ABI = parseAbi([
  "function commitShot(bytes32 commitment) returns (bool recorded)",
  "function getCommitment(bytes32 commitment) view returns ((uint64 committedAt,bool exists))",
  "function getShot(bytes32 shotId) view returns ((address controller,bytes32 head,uint64 checkpointSequence,uint64 nonce))",
  "function registerShot((bytes32 shotId,address controller,bytes32 head,bytes32 salt,uint64 nonce,uint64 deadline) action,bytes signature)",
  "function appendCheckpoint((bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 checkpointSequence,uint64 nonce,uint64 deadline) action,bytes signature)",
]);

type JsonObject = Record<string, unknown>;

export interface RegistryRouter {
  handles(pathname: string): boolean;
  fetch(request: Request): Promise<Response>;
  renderRegistry(query?: string): Promise<string>;
  renderShot(shotID: string): Promise<string | undefined>;
  renderBuilder(builder: string): Promise<string | undefined>;
  renderHumanRoute(pathname: string): Promise<string | undefined>;
  currentClaimContext(shotID: Hex, releaseDigest: Hex): Promise<ClaimCatalogContext>;
  claimReceiptContext(shotID: Hex, releaseDigest: Hex): Promise<ClaimCatalogContext>;
}

export interface ChainVerifier {
  verify(envelope: JsonObject, transactionHash: Hex): Promise<ChainEvidence>;
  verifyBuilderKey?(builderID: string, keyID: Hex): Promise<void>;
  revalidate?(record: CatalogRecord): Promise<boolean>;
}

interface ChainEvidence {
  transactionHash: Hex;
  blockNumber: string;
  blockHash: Hex;
  controller: Hex;
  head: Hex;
  checkpointSequence: number;
  signerKeyID: Hex;
  blockTimestamp?: string;
  transactionIndex?: number;
  logIndex?: number;
}

interface CatalogRecord {
  schema: typeof RECORD_SCHEMA;
  releaseDigest: Hex;
  route: string;
  envelope: JsonObject;
  chain: ChainEvidence;
  promotedAt: string;
  sourceURL: string;
  iconURL?: string;
}

export interface StagingRecord {
  schema: typeof STAGING_SCHEMA;
  stagingID: string;
  tokenSHA256: string;
  envelope: JsonObject;
  releaseDigest: Hex;
  createdAt: string;
  expiresAt: string;
  sourceUploaded: boolean;
  iconUploaded: boolean;
}

export interface PublicationJob {
  schema: "tohseno.registry-publication-job/2";
  jobID: string;
  stagingID: string;
  tokenSHA256: string;
  registry: JsonObject;
  claimEdition?: JsonObject;
  status: "prepared" | "account_ready" | "committed" | "waiting_maturity" | "submitted" | "claims_submitted" | "complete" | "failed";
  accountTransactionHash?: Hex;
  commitTransactionHash?: Hex;
  registryTransactionHash?: Hex;
  claimsTransactionHash?: Hex;
  committedAt?: number;
  publicRecord?: JsonObject;
  failure?: string;
  createdAt: string;
  updatedAt: string;
}

interface SignedProfileRecord {
  schema: "tohseno.builder-profile-record/1";
  digest: Hex;
  keyID: Hex;
  envelope: JsonObject;
  acceptedAt: string;
}

interface AliasClaimRecord {
  schema: "tohseno.alias-claim-record/1";
  status: "pending_policy_review";
  digest: Hex;
  key_id: Hex;
  envelope: JsonObject;
  received_at: string;
}

interface AliasApprovalRecord {
  schema: "tohseno.alias-approval/1";
  request_id: Hex;
  alias: string;
  shot_id: Hex;
  builder_id: string;
  claim_digest: Hex;
  approved_at: string;
}

interface TimelineEvent {
  schema: "tohseno.timeline-event/1";
  event_id: Hex;
  kind: "shot.shipped" | "shot.updated" | "shot.forked" | "claim.edition_closed";
  shot_id: Hex;
  builder_id: string;
  release_digest: Hex;
  checkpoint_sequence: number;
  occurred_at: string;
  canonical_block: { number: string; hash: Hex; transaction_index: number | null; log_index: number | null };
  parent: { shot_id: Hex; release_digest: Hex } | null;
  closure_reason?: "supply_filled" | "time_elapsed";
}

export async function createRegistryRouter(
  config: AppConfig,
  verifier?: ChainVerifier,
  claims?: ClaimsPublicationBridge,
  injectedRelayer?: ConstrainedRelayer,
): Promise<RegistryRouter> {
  if (!config.registry.enabled) return unavailableRouter();
  const root = config.registry.root!;
  await mkdir(root, { recursive: true, mode: 0o700 });
  const rootMetadata = await lstat(root);
  if (!rootMetadata.isDirectory() || rootMetadata.isSymbolicLink()) {
    throw new Error("REGISTRY_ROOT must be a real directory, not a symlink");
  }
  const directories = {
    blobs: join(root, "blobs", "sha256"),
    staging: join(root, "staging"),
    releases: join(root, "releases"),
    shots: join(root, "shots"),
    builders: join(root, "builders"),
    jobs: join(root, "jobs"),
    profiles: join(root, "profiles"),
    handles: join(root, "handles"),
    aliases: join(root, "aliases"),
    aliasClaims: join(root, "alias-claims"),
    aliasApprovals: join(root, "alias-approvals"),
    aliasNonces: join(root, "alias-nonces"),
  };
  await Promise.all(Object.values(directories).map((path) => mkdir(path, { recursive: true, mode: 0o700 })));
  const chain = verifier ?? new RobinhoodVerifier(config);
  const relayer = injectedRelayer ?? (config.registry.relayerEnabled
    ? createRelayer(config)
    : undefined);
  const publicLaunch = config.registry.relayerEnabled
    && config.distribution.macosEnabled;
  const limiter = new RateLimiter();
  const chainCache = new Map<string, { checkedAt: number; valid: boolean }>();
  let mutation = Promise.resolve();

  const serialized = async <T>(operation: () => Promise<T>): Promise<T> => {
    const previous = mutation;
    let release!: () => void;
    mutation = new Promise<void>((resolve) => { release = resolve; });
    await previous;
    try { return await operation(); }
    finally { release(); }
  };

  const discoverable = async (records: CatalogRecord[]): Promise<CatalogRecord[]> => {
    if (!chain.revalidate) return records;
    const result: CatalogRecord[] = [];
    const now = Date.now();
    for (let offset = 0; offset < Math.min(records.length, 10_000); offset += 8) {
      const batch = records.slice(offset, offset + 8);
      const checked = await Promise.all(batch.map(async (record) => {
        const cached = chainCache.get(record.releaseDigest);
        if (cached && now - cached.checkedAt < 15_000) return cached.valid;
        const valid = await chain.revalidate!(record);
        chainCache.set(record.releaseDigest, { checkedAt: Date.now(), valid });
        return valid;
      }));
      checked.forEach((valid, index) => { if (valid) result.push(batch[index]!); });
    }
    if (chainCache.size > 20_000) {
      for (const [digest, cached] of chainCache) {
        if (now - cached.checkedAt > 60_000) chainCache.delete(digest);
      }
    }
    return result;
  };

  const discoverableShot = async (shotID: Hex): Promise<CatalogRecord | undefined> => {
    const records = await discoverable((await allRecords(directories.releases))
      .filter((record) => releaseOf(record).shot_id === shotID).sort(newestFirst));
    return records[0];
  };

  const reviewableAliasRequest = async (requestID: Hex) => {
    const claimRecord = await readJSON<AliasClaimRecord>(
      join(directories.aliasClaims, `${requestID.slice(2)}.json`),
    );
    if (!claimRecord || claimRecord.schema !== "tohseno.alias-claim-record/1"
        || claimRecord.status !== "pending_policy_review") {
      throw new HttpError(404, "Pending alias request not found");
    }
    const verified = verifyStoredAliasClaim(claimRecord);
    const claim = object(claimRecord.envelope.claim, "alias claim");
    if (normalizeHex32(claim.request_id) !== requestID || claimRecord.digest !== verified.digest
        || claimRecord.key_id !== verified.keyID) {
      throw new HttpError(409, "Stored alias request evidence is inconsistent");
    }
    const builder = normalizeBuilder(claim.builder_id);
    const shotID = normalizeHex32(claim.shot_id);
    const alias = normalizeGlobalAlias(claim.alias);
    await verifyCurrentBuilderKey(chain, builder, verified.keyID);
    const release = await discoverableShot(shotID);
    if (!release || releaseOf(release).builder_id !== builder
        || object(releaseOf(release).permissions, "permissions").install_allowed !== true) {
      throw new HttpError(422, "Alias review requires a current installable Shot from this Builder");
    }
    return { alias, builder, claimRecord, release, requestID, shotID, verified };
  };

  const claimContextOf = (record: CatalogRecord): ClaimCatalogContext => {
    const release = releaseOf(record);
    const display = object(release.display, "release.display");
    return { shotID: normalizeHex32(release.shot_id), builderID: normalizeBuilder(release.builder_id),
      releaseDigest: record.releaseDigest, checkpointDigest: normalizeDigest(release.public_checkpoint_digest),
      checkpointSequence: positiveSafeInteger(release.checkpoint_sequence, "checkpoint_sequence"),
      appName: String(display.name), appDescription: String(display.description), sourceURL: record.sourceURL,
      canonicalBlock: { number: record.chain.blockNumber, hash: record.chain.blockHash } };
  };

  async function finalizeStaging(
    stagingID: string,
    staging: StagingRecord,
    transactionHash: Hex,
  ): Promise<JsonObject> {
    const release = releaseOf(staging.envelope);
    const display = object(release.display, "release.display");
    if (!staging.sourceUploaded) throw new HttpError(409, "Source artifact has not been staged");
    if (display.icon_sha256 !== null && display.icon_sha256 !== undefined && !staging.iconUploaded) {
      throw new HttpError(409, "Icon artifact has not been staged");
    }
    const record = await (async () => {
      const existing = await readJSON<CatalogRecord>(join(directories.releases, `${staging.releaseDigest.slice(2)}.json`));
      if (existing) {
        if (!chain.revalidate || await chain.revalidate(existing)) return existing;
        throw new HttpError(409, "Existing release evidence is no longer canonical");
      }
      await requireBuilderLocalSlugAvailable(directories.releases, release);
      const evidence = await chain.verify(staging.envelope, transactionHash);
      const sourceDigest = normalizeDigest(object(release.source, "release.source").sha256);
      await promote(join(directories.staging, `${stagingID}.source`), blobPath(directories.blobs, sourceDigest), sourceDigest);
      let iconURL: string | undefined;
      if (display.icon_sha256) {
        const iconDigest = normalizeDigest(display.icon_sha256);
        await promote(join(directories.staging, `${stagingID}.icon`), blobPath(directories.blobs, iconDigest), iconDigest);
        iconURL = `/api/registry/v1/blobs/${iconDigest}`;
      }
      const route = canonicalRoute(release);
      const next: CatalogRecord = { schema: RECORD_SCHEMA, releaseDigest: staging.releaseDigest,
        route, envelope: staging.envelope, chain: evidence, promotedAt: new Date().toISOString(),
        sourceURL: `/api/registry/v1/blobs/${sourceDigest}`,
        ...(iconURL ? { iconURL } : {}) };
      await atomicJSON(join(directories.releases, `${staging.releaseDigest.slice(2)}.json`), next, true);
      await updateIndexes(directories, next);
      await rm(join(directories.staging, `${stagingID}.json`), { force: true });
      return next;
    })();
    return publicRecord(record);
  }

  async function publicationStatus(request: Request, id: string): Promise<Response> {
    if (!relayer) throw new HttpError(503, "The constrained Registry relayer is not enabled");
    const jobID = normalizeStagingID(id);
    const path = join(directories.jobs, `${jobID}.json`);
    let job = await readJSON<PublicationJob>(path);
    if (!job || job.schema !== "tohseno.registry-publication-job/2") throw new HttpError(404, "Publication job not found");
    authorizeToken(request, job.tokenSHA256);
    job = await serialized(async () => {
      const current = await readJSON<PublicationJob>(path);
      if (!current) throw new HttpError(404, "Publication job not found");
      if (!["complete", "failed"].includes(current.status)) {
        try {
          const staging = await readJSON<StagingRecord>(join(directories.staging, `${current.stagingID}.json`));
          if (!staging) throw new HttpError(409, "Publication staging reservation is unavailable");
          await relayer.advance(current, staging);
          if (current.status === "submitted" && current.registryTransactionHash) {
            const release = releaseOf(staging.envelope);
            if (release.checkpoint_sequence === 1) {
              if (!claims || !current.claimEdition) {
                throw new HttpError(503, "First Ship requires the separately activated Claims publication path");
              }
              const claim = await claims.advanceOpenEdition(
                current.claimEdition,
                staging.envelope,
                current.claimsTransactionHash,
              );
              current.claimsTransactionHash = claim.transactionHash;
              if (!claim.confirmed) throw new PublicationPending("claims_submitted");
            }
            current.publicRecord = await finalizeStaging(
              current.stagingID,
              staging,
              current.registryTransactionHash,
            );
            current.status = "complete";
          }
        } catch (error) {
          if (error instanceof PublicationPending) {
            current.status = error.status;
          } else {
            current.status = "failed";
            current.failure = error instanceof Error ? error.message.slice(0, 300) : "publication failed";
          }
        }
        current.updatedAt = new Date().toISOString();
        await atomicJSON(path, current, false);
      }
      return current;
    });
    return json(publicPublicationJob(job), job.status === "complete" ? 200 : job.status === "failed" ? 422 : 202);
  }

  async function fetchRoute(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const method = request.method.toUpperCase();
    const parts = url.pathname.split("/").filter(Boolean);
    const source = sourceKey(request, config);
    if (!limiter.take("global", config.registry.globalRequestsPerMinute)
        || !limiter.take(source, config.registry.sourceRequestsPerMinute)) {
      throw new HttpError(429, "Registry request rate exceeded; retry later");
    }
    if (url.pathname === "/api/registry/v1/status" && (method === "GET" || method === "HEAD")) {
      return head(json({ schema: "tohseno.registry-status/1", available: true,
        generation: "0.8.0", chain_id: config.registry.chainId,
        factory: config.registry.factoryAddress, registry: config.registry.registryAddress,
        relayer: relayer ? { available: true, address: relayer.address } : { available: false } }), method);
    }
    if (url.pathname === "/api/registry/v1/shots" && (method === "GET" || method === "HEAD")) {
      const limit = boundedLimit(url.searchParams.get("limit"));
      const query = url.searchParams.get("q")?.trim().toLocaleLowerCase("en-US");
      if (query && (query.length > 100 || /[\u0000-\u001f\u007f]/.test(query))) {
        throw new HttpError(400, "search query is invalid");
      }
      const records = latestPerShot((await discoverable(await allRecords(directories.releases))).sort(newestFirst))
        .filter((record) => !query || searchableRelease(releaseOf(record)).includes(query))
        .slice(0, limit);
      return head(json({ schema: "tohseno.catalog-page/1", releases: records.map(publicRecord), next_cursor: null }), method);
    }
    if (url.pathname === "/api/registry/v1/timeline" && (method === "GET" || method === "HEAD")) {
      const events = await timelineEvents(
        await discoverable(await allRecords(directories.releases)), claims,
      );
      return head(json(timelinePage(events, url)), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/shots" && (method === "GET" || method === "HEAD")) {
      const shotID = normalizeHex32(parts[4]);
      const record = await discoverableShot(shotID);
      if (!record) throw new HttpError(404, "Published Shot not found");
      return head(json(publicRecord(record)), method);
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/shots"
        && parts[5] === "releases" && (method === "GET" || method === "HEAD")) {
      const shotID = normalizeHex32(parts[4]);
      const records = (await discoverable(await allRecords(directories.releases)))
        .filter((record) => releaseOf(record).shot_id === shotID)
        .sort(newestFirst);
      if (!records.length) throw new HttpError(404, "Published Shot not found");
      return head(json({ schema: "tohseno.catalog-page/1", releases: records.map(publicRecord), next_cursor: null }), method);
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/shots"
        && parts[5] === "timeline" && (method === "GET" || method === "HEAD")) {
      const shotID = normalizeHex32(parts[4]);
      const events = await timelineEvents((await discoverable(await allRecords(directories.releases)))
        .filter((record) => releaseOf(record).shot_id === shotID), claims);
      if (!events.length) throw new HttpError(404, "Published Shot not found");
      return head(json(timelinePage(events, url)), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/releases"
        && (method === "GET" || method === "HEAD")) {
      const digest = normalizeDigest(parts[4]);
      const record = await readJSON<CatalogRecord>(join(directories.releases, `${digest.slice(2)}.json`));
      if (!record || record.schema !== RECORD_SCHEMA || record.releaseDigest !== digest) {
        throw new HttpError(404, "Catalog release not found");
      }
      if (chain.revalidate && !await chain.revalidate(record)) {
        throw new HttpError(409, "Catalog release evidence is no longer canonical");
      }
      return head(json(releaseEvidence(record)), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/builders" && (method === "GET" || method === "HEAD")) {
      const builder = normalizeBuilder(parts[4]);
      const records = (await discoverable(await allRecords(directories.releases)))
        .filter((record) => releaseOf(record).builder_id === builder);
      const profileRecord = await profileForBuilder(directories.profiles, builder);
      if (!records.length && !profileRecord) throw new HttpError(404, "Builder not found");
      return head(json({ schema: "tohseno.builder-page/1", builder_id: builder,
        profile: profileRecord ? object(profileRecord.envelope.profile, "profile") : null,
        profile_digest: profileRecord?.digest ?? null,
        releases: records.sort(newestFirst).map(publicRecord) }), method);
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/builders"
        && parts[5] === "profile" && method === "PUT") {
      requireJSON(request);
      const builder = normalizeBuilder(parts[4]);
      const body = await boundedJSON(request, 128 * 1024);
      exactKeys(body, ["envelope"], "profile update request");
      const envelope = object(body.envelope, "profile envelope");
      const verified = verifySignedProfile(envelope, builder);
      await verifyCurrentBuilderKey(chain, builder, verified.keyID);
      const record = await serialized(async () => {
        const existing = await profileForBuilder(directories.profiles, builder);
        const profile = object(envelope.profile, "profile");
        const nonce = positiveSafeInteger(profile.nonce, "profile.nonce");
        const oldProfile = existing ? object(existing.envelope.profile, "profile") : undefined;
        if (existing && nonce <= positiveSafeInteger(oldProfile!.nonce, "profile.nonce")) {
          throw new HttpError(409, "Profile nonce was already used");
        }
        const handle = profile.handle === null ? undefined : normalizeName(profile.handle, "profile.handle", 32);
        if (handle) {
          const claimed = await readJSON<{ builderID: string }>(join(directories.handles, `${handle}.json`));
          if (claimed && claimed.builderID !== builder) throw new HttpError(409, "Builder handle is already granted");
        }
        const next: SignedProfileRecord = {
          schema: "tohseno.builder-profile-record/1", digest: verified.digest,
          keyID: verified.keyID, envelope, acceptedAt: new Date().toISOString(),
        };
        await atomicJSON(join(directories.profiles, `${builder.split(":").at(-1)!.slice(2)}.json`), next, false);
        if (handle) await atomicJSON(join(directories.handles, `${handle}.json`), { builderID: builder }, false);
        const oldHandle = oldProfile?.handle;
        if (typeof oldHandle === "string" && oldHandle !== handle) {
          await rm(join(directories.handles, `${oldHandle}.json`), { force: true });
        }
        return next;
      });
      return json({ schema: "tohseno.builder-profile-receipt/1", digest: record.digest,
        profile: object(record.envelope.profile, "profile") });
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/aliases"
        && parts[4] === "claims" && method === "POST") {
      requireJSON(request);
      const body = await boundedJSON(request, 128 * 1024);
      exactKeys(body, ["envelope"], "alias claim request");
      const envelope = object(body.envelope, "alias claim envelope");
      const verified = verifySignedAliasClaim(envelope);
      const claim = object(envelope.claim, "alias claim");
      const builder = normalizeBuilder(claim.builder_id);
      await verifyCurrentBuilderKey(chain, builder, verified.keyID);
      const shotID = normalizeHex32(claim.shot_id);
      const release = await discoverableShot(shotID);
      if (!release || releaseOf(release).builder_id !== builder
          || object(releaseOf(release).permissions, "permissions").install_allowed !== true) {
        throw new HttpError(422, "Alias claims require an installable Shot controlled by this Builder");
      }
      const alias = normalizeGlobalAlias(claim.alias);
      const requestID = normalizeHex32(claim.request_id);
      const nonce = positiveSafeInteger(claim.nonce, "alias nonce");
      await serialized(async () => {
        if (await readJSON(join(directories.aliases, `${alias}.json`))) {
          throw new HttpError(409, "Alias is unavailable");
        }
        const builderAddress = builder.split(":").at(-1)!.slice(2);
        const noncePath = join(directories.aliasNonces, `${builderAddress}.json`);
        const prior = await readJSON<{ nonce: number }>(noncePath);
        if (prior && nonce <= prior.nonce) throw new HttpError(409, "Alias claim nonce was already used");
        await atomicJSON(noncePath, { schema: "tohseno.alias-claim-nonce/1", nonce }, false);
        await atomicJSON(join(directories.aliasClaims, `${requestID.slice(2)}.json`), {
          schema: "tohseno.alias-claim-record/1", status: "pending_policy_review",
          digest: verified.digest, key_id: verified.keyID, envelope, received_at: new Date().toISOString(),
        }, true);
      });
      return json({ schema: "tohseno.alias-claim-receipt/1", request_id: requestID,
        alias, status: "pending_policy_review" }, 202);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/alias-reviews"
        && (method === "GET" || method === "HEAD")) {
      if (!config.registry.aliasReviewTokenSha256) {
        throw new HttpError(503, "Global alias review is not enabled");
      }
      authorizeToken(request, config.registry.aliasReviewTokenSha256);
      const requestID = normalizeHex32(parts[4]);
      const context = await reviewableAliasRequest(requestID);
      const approval = await readJSON<AliasApprovalRecord>(
        join(directories.aliasApprovals, `${requestID.slice(2)}.json`),
      );
      if (approval && (approval.schema !== "tohseno.alias-approval/1"
          || approval.alias !== context.alias || approval.shot_id !== context.shotID
          || approval.builder_id !== context.builder
          || approval.claim_digest !== context.verified.digest)) {
        throw new HttpError(409, "Alias request has inconsistent approval evidence");
      }
      const release = releaseOf(context.release);
      const display = object(release.display, "release.display");
      return head(json({ schema: "tohseno.alias-review/1", request_id: requestID,
        status: approval ? "approved" : "pending_policy_review", alias: context.alias,
        route: `/${context.alias}`, shot_id: context.shotID, builder_id: context.builder,
        claim_digest: context.verified.digest, signer_key_id: context.verified.keyID,
        received_at: context.claimRecord.received_at, approved_at: approval?.approved_at ?? null,
        current_release: { name: display.name, release_digest: context.release.releaseDigest,
          checkpoint_sequence: release.checkpoint_sequence,
          canonical_route: context.release.route } }), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/alias-reviews"
        && method === "POST") {
      if (!config.registry.aliasReviewTokenSha256) {
        throw new HttpError(503, "Global alias review is not enabled");
      }
      authorizeToken(request, config.registry.aliasReviewTokenSha256);
      requireJSON(request);
      const body = await boundedJSON(request, 4 * 1024);
      exactKeys(body, ["decision"], "alias review");
      if (body.decision !== "approve") {
        throw new HttpError(422, "The bounded alias review action is approve");
      }
      const requestID = normalizeHex32(parts[4]);
      const { alias, builder, shotID, verified } = await reviewableAliasRequest(requestID);
      const result = await serialized(async () => {
        const approvalPath = join(directories.aliasApprovals, `${requestID.slice(2)}.json`);
        const pointerPath = join(directories.aliases, `${alias}.json`);
        const existingApproval = await readJSON<AliasApprovalRecord>(approvalPath);
        const existingPointer = await readJSON<JsonObject>(pointerPath);
        if (existingApproval && (existingApproval.schema !== "tohseno.alias-approval/1"
            || existingApproval.alias !== alias || existingApproval.shot_id !== shotID
            || existingApproval.builder_id !== builder || existingApproval.claim_digest !== verified.digest)) {
          throw new HttpError(409, "Alias request already has different approval evidence");
        }
        if (existingPointer && (existingPointer.schema !== "tohseno.global-alias/1"
            || existingPointer.request_id !== requestID || existingPointer.shot_id !== shotID
            || existingPointer.builder_id !== builder)) {
          throw new HttpError(409, "Alias is unavailable");
        }
        const approval: AliasApprovalRecord = existingApproval ?? {
          schema: "tohseno.alias-approval/1", request_id: requestID, alias, shot_id: shotID,
          builder_id: builder, claim_digest: verified.digest, approved_at: new Date().toISOString(),
        };
        if (!existingApproval) await atomicJSON(approvalPath, approval, true);
        if (!existingPointer) {
          await atomicJSON(pointerPath, {
            schema: "tohseno.global-alias/1", alias, shot_id: shotID, builder_id: builder,
            request_id: requestID, claim_digest: verified.digest,
            approved_at: approval.approved_at,
          }, true);
        }
        return { approval, created: !existingPointer };
      });
      return json({ schema: "tohseno.alias-approval-receipt/1", alias,
        route: `/${alias}`, shot_id: shotID, request_id: requestID,
        approved_at: result.approval.approved_at }, result.created ? 201 : 200);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/aliases"
        && parts[4] !== "claims" && (method === "GET" || method === "HEAD")) {
      const alias = normalizeGlobalAlias(parts[4]);
      const value = await readJSON<JsonObject>(join(directories.aliases, `${alias}.json`));
      if (!value) throw new HttpError(404, "Alias not found");
      return head(json(value), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/blobs" && (method === "GET" || method === "HEAD")) {
      const digest = normalizeDigest(parts[4]);
      const path = blobPath(directories.blobs, digest);
      const metadata = await stat(path).catch(() => undefined);
      if (!metadata?.isFile()) throw new HttpError(404, "Blob not found");
      const range = method === "GET" ? byteRange(request.headers.get("range"), metadata.size) : undefined;
      const headers = {
        "content-type": "application/octet-stream", "content-length": String(metadata.size),
        "cache-control": "public, max-age=31536000, immutable", "content-disposition": "attachment",
        "accept-ranges": "bytes", "x-content-sha256": digest,
      };
      if (range) {
        const bytes = await Bun.file(path).slice(range.start, range.end + 1).arrayBuffer();
        return withSecurityHeaders(new Response(bytes, {
          status: 206, headers: { ...headers, "content-length": String(range.end - range.start + 1),
            "content-range": `bytes ${range.start}-${range.end}/${metadata.size}` },
        }));
      }
      return head(withSecurityHeaders(new Response(Bun.file(path), { headers })), method);
    }
    if (url.pathname === "/api/registry/v1/staging" && method === "POST") {
      requireJSON(request);
      const body = await boundedJSON(request, 256 * 1024);
      exactKeys(body, ["envelope"], "staging request");
      const envelope = object(body.envelope, "envelope");
      const releaseDigest = verifyEnvelope(envelope, config);
      const token = randomHex(32);
      const record = await serialized(async () => {
        await collectExpiredStaging(directories.staging);
        await requireStagingCapacity(directories.staging, releaseOf(envelope), config);
        await requireBuilderLocalSlugAvailable(directories.releases, releaseOf(envelope));
        await requireBuilderLocalSlugAvailableInStaging(directories.staging, releaseOf(envelope));
        const stagingID = crypto.randomUUID().replaceAll("-", "");
        const now = new Date();
        const value: StagingRecord = {
          schema: STAGING_SCHEMA, stagingID, tokenSHA256: sha256Hex(new TextEncoder().encode(token)).slice(2),
          envelope, releaseDigest, createdAt: now.toISOString(),
          expiresAt: new Date(now.getTime() + 30 * 60 * 1000).toISOString(),
          sourceUploaded: false, iconUploaded: false,
        };
        await atomicJSON(join(directories.staging, `${stagingID}.json`), value, true);
        return value;
      });
      return json({ schema: "tohseno.catalog-staging-receipt/1", staging_id: record.stagingID,
        upload_token: token, expires_at: record.expiresAt,
        source_url: `/api/registry/v1/staging/${record.stagingID}/source`,
        finalize_url: `/api/registry/v1/staging/${record.stagingID}/finalize` }, 201);
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/staging" && method === "PUT") {
      const stagingID = normalizeStagingID(parts[4]);
      const kind = parts[5];
      if (kind !== "source" && kind !== "icon") throw new HttpError(404, "Not found");
      const staging = await authorizedStaging(request, directories.staging, stagingID);
      const release = releaseOf(staging.envelope);
      const expected = kind === "source" ? normalizeDigest(object(release.source, "release.source").sha256)
        : normalizeDigest(object(release.display, "release.display").icon_sha256);
      const maximum = kind === "source" ? MAX_SOURCE_BYTES : MAX_ICON_BYTES;
      if (kind === "source") {
        const declared = positiveSafeInteger(object(release.source, "release.source").byte_length, "source.byte_length");
        if (request.headers.get("content-length") !== String(declared)) {
          throw new HttpError(422, "Source Content-Length differs from the signed manifest");
        }
      }
      const temporary = join(directories.staging, `${stagingID}.${kind}.partial`);
      const observed = await writeBoundedBody(request, temporary, maximum);
      if (observed !== expected) { await rm(temporary, { force: true }); throw new HttpError(422, `${kind} digest mismatch`); }
      await rename(temporary, join(directories.staging, `${stagingID}.${kind}`));
      if (kind === "source") staging.sourceUploaded = true; else staging.iconUploaded = true;
      await atomicJSON(join(directories.staging, `${stagingID}.json`), staging, false);
      return json({ schema: "tohseno.catalog-blob-staged/1", kind, sha256: observed });
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/staging" && parts[5] === "finalize" && method === "POST") {
      requireJSON(request);
      const stagingID = normalizeStagingID(parts[4]);
      const staging = await authorizedStaging(request, directories.staging, stagingID);
      if (config.claims.configured && releaseOf(staging.envelope).checkpoint_sequence === 1) {
        throw new HttpError(409, "First Ship must atomically witness its immutable Claim Edition before catalog promotion");
      }
      const body = await boundedJSON(request, 16 * 1024);
      exactKeys(body, ["transaction_hash"], "finalize request");
      const transactionHash = normalizeDigest(body.transaction_hash) as Hex;
      const record = await serialized(() => finalizeStaging(stagingID, staging, transactionHash));
      return json(record, 201);
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/staging" && parts[5] === "publish" && method === "POST") {
      if (!relayer) throw new HttpError(503, "The constrained Registry relayer is not enabled");
      requireJSON(request);
      const stagingID = normalizeStagingID(parts[4]);
      const staging = await authorizedStaging(request, directories.staging, stagingID);
      if (!staging.sourceUploaded) throw new HttpError(409, "Source artifact has not been staged");
      const body = await boundedJSON(request, 256 * 1024);
      const firstShip = releaseOf(staging.envelope).checkpoint_sequence === 1;
      exactKeys(body, firstShip ? ["registry", "claim_edition"] : ["registry"], "publication request");
      const signedRegistry = object(body.registry, "registry authorization");
      verifyRegistryAuthorization(signedRegistry, staging.envelope, config);
      const claimEdition = firstShip ? object(body.claim_edition, "Claim Edition approval") : undefined;
      if (firstShip) {
        if (!claims) throw new HttpError(503, "First Ship requires the separately activated Claims publication path");
        await claims.verifyOpenEdition(claimEdition!, staging.envelope);
      }
      const now = new Date().toISOString();
      const job: PublicationJob = {
        schema: "tohseno.registry-publication-job/2", jobID: stagingID, stagingID,
        tokenSHA256: staging.tokenSHA256, registry: signedRegistry, status: "prepared",
        ...(claimEdition ? { claimEdition } : {}),
        createdAt: now, updatedAt: now,
      };
      const path = join(directories.jobs, `${stagingID}.json`);
      const existing = await readJSON<PublicationJob>(path);
      if (existing) {
        if (canonicalCatalogJSON(existing.registry) !== canonicalCatalogJSON(signedRegistry)
            || canonicalCatalogJSON(existing.claimEdition ?? {}) !== canonicalCatalogJSON(claimEdition ?? {})) {
          throw new HttpError(409, "Publication job already has different authorization");
        }
        return json(publicPublicationJob(existing), 202);
      }
      await atomicJSON(path, job, true);
      return json(publicPublicationJob(job), 202);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/publications"
        && (method === "GET" || method === "POST")) {
      return publicationStatus(request, parts[4]);
    }
    if (url.pathname.startsWith("/api/registry/v1/")) {
      if (!["GET", "HEAD", "POST", "PUT"].includes(method)) return methodNotAllowed();
      throw new HttpError(404, "Not found");
    }
    throw new HttpError(404, "Not found");
  }

  return {
    handles: (pathname) => pathname.startsWith("/api/registry/v1/"),
    fetch: fetchRoute,
    renderRegistry: async (rawQuery) => {
      const query = rawQuery?.trim().toLocaleLowerCase("en-US");
      if (query && (query.length > 100 || /[\u0000-\u001f\u007f]/.test(query))) return registryHTML([], "", publicLaunch);
      const all = (await discoverable(await allRecords(directories.releases))).sort(newestFirst);
      const filtered = all.filter((record) => !query || searchableRelease(releaseOf(record)).includes(query));
      const events = (await timelineEvents(filtered, claims)).slice(0, 100);
      const records = filtered.map(publicRecord);
      const editions = new Map<string, { maxClaims: bigint; totalClaims: bigint; closed: boolean }>();
      if (claims) {
        await Promise.all(latestPerShot(filtered).slice(0, 100).map(async (record) => {
          const shotID = normalizeHex32(releaseOf(record).shot_id);
          const edition = await claims.editionForDisplay(shotID);
          if (edition?.opened) editions.set(shotID, edition);
        }));
      }
      return registryHTML(records, rawQuery?.trim() ?? "", publicLaunch, events, editions);
    },
    renderShot: async (shotID) => {
      if (!HEX32.test(shotID)) return undefined;
      const record = await discoverableShot(shotID as Hex);
      if (!record) return undefined;
      const edition = await claims?.editionForDisplay(shotID as Hex);
      return shotHTML(publicRecord(record), edition);
    },
    renderBuilder: async (builder) => {
      const id = decodeURIComponent(builder);
      const indexed = IDENTIFIER.test(id)
        ? await readJSON<{ builderID: string }>(join(directories.handles, `${id}.json`))
        : undefined;
      const builderID = /^eip155:4663:0x[0-9a-f]{40}$/.test(id) ? id : indexed?.builderID;
      if (!builderID) return undefined;
      const records = (await discoverable(await allRecords(directories.releases))).filter((record) => {
        const release = releaseOf(record);
        return release.builder_id === builderID;
      });
      const profile = await profileForBuilder(directories.profiles, builderID);
      if (!records.length && !profile) return undefined;
      return builderHTML(builderID, records.sort(newestFirst).map(publicRecord),
        profile ? object(profile.envelope.profile, "profile") : undefined);
    },
    renderHumanRoute: async (pathname) => {
      const builderRoute = pathname.match(/^\/@([a-z0-9]+(?:-[a-z0-9]+)*)\/([a-z0-9]+(?:-[a-z0-9]+)*)$/);
      if (builderRoute) {
        const indexed = await readJSON<{ builderID: string }>(
          join(directories.handles, `${builderRoute[1]}.json`),
        );
        if (!indexed) return undefined;
        const profile = await profileForBuilder(directories.profiles, indexed.builderID);
        if (!profile || object(profile.envelope.profile, "profile").handle !== builderRoute[1]) {
          return undefined;
        }
        const record = (await discoverable(await allRecords(directories.releases))).filter((item) => {
          const release = releaseOf(item);
          return release.builder_id === indexed.builderID
            && object(release.display, "display").app_slug === builderRoute[2];
        }).sort(newestFirst)[0];
        if (!record) return undefined;
        return shotHTML(publicRecord(record), await claims?.editionForDisplay(
          normalizeHex32(releaseOf(record).shot_id),
        ));
      }
      const alias = pathname.match(/^\/([a-z0-9]+(?:-[a-z0-9]+)*)$/)?.[1];
      if (!alias || alias.length > 64) return undefined;
      const pointer = await readJSON<JsonObject>(join(directories.aliases, `${alias}.json`));
      if (!pointer || pointer.schema !== "tohseno.global-alias/1") return undefined;
      const target = await discoverableShot(normalizeHex32(pointer.shot_id));
      if (!target) return undefined;
      return shotHTML(publicRecord(target), await claims?.editionForDisplay(
        normalizeHex32(releaseOf(target).shot_id),
      ));
    },
    currentClaimContext: async (shotID, releaseDigest) => {
      const current = await discoverableShot(normalizeHex32(shotID));
      if (!current || current.releaseDigest !== normalizeDigest(releaseDigest)) {
        throw new HttpError(409, "Claim preparation requires the current canonical release; refresh this app first");
      }
      return claimContextOf(current);
    },
    claimReceiptContext: async (shotID, releaseDigest) => {
      const record = await readJSON<CatalogRecord>(join(directories.releases, `${normalizeDigest(releaseDigest).slice(2)}.json`));
      if (!record || releaseOf(record).shot_id !== normalizeHex32(shotID)
          || !(await discoverable([record])).length) {
        throw new HttpError(404, "The exact claimed release is not canonically available");
      }
      return claimContextOf(record);
    },
  };
}

class RobinhoodVerifier implements ChainVerifier {
  private readonly client: PublicClient;
  constructor(private readonly config: AppConfig) {
    this.client = createPublicClient({ transport: http(config.registry.rpcUrl!, { timeout: 10_000 }) });
  }

  async verify(envelope: JsonObject, transactionHash: Hex): Promise<ChainEvidence> {
    const release = releaseOf(envelope);
    const builder = normalizeBuilder(release.builder_id);
    const controller = builderAddress(builder);
    const shotID = normalizeHex32(release.shot_id);
    const head = normalizeDigest(release.public_checkpoint_digest);
    const sequence = positiveSafeInteger(release.checkpoint_sequence, "checkpoint_sequence");
    const receipt = await this.client.getTransactionReceipt({ hash: transactionHash });
    if (receipt.status !== "success" || receipt.to?.toLowerCase() !== this.config.registry.registryAddress) {
      throw new HttpError(422, "The transaction is not a successful active Registry call");
    }
    const matched = receipt.logs.find((log) => {
      if (log.address.toLowerCase() !== this.config.registry.registryAddress) return false;
      try {
        const decoded = decodeEventLog({ abi: REGISTRY_ABI, data: log.data, topics: log.topics });
        const args = decoded.args as Record<string, unknown>;
        return (decoded.eventName === "ShotRegistered" && args.shotId === shotID && args.head === head)
          || (decoded.eventName === "CheckpointAppended" && args.shotId === shotID && args.newHead === head);
      } catch { return false; }
    });
    if (!matched) throw new HttpError(422, "The transaction receipt does not contain the declared checkpoint");
    const observed = await this.client.readContract({ address: this.config.registry.registryAddress,
      abi: REGISTRY_ABI, functionName: "getShot", args: [shotID] });
    if (observed.controller.toLowerCase() !== controller || observed.head !== head || Number(observed.checkpointSequence) !== sequence) {
      throw new HttpError(409, "The current Registry head no longer matches this release");
    }
    const signer = object(envelope.signer, "signer");
    const keyID = (`0x${bytesToHex(keccak_256(concat(hexBytes(signer.x), hexBytes(signer.y))))}`) as Hex;
    const authorized = await this.client.readContract({ address: controller, abi: ACCOUNT_ABI,
      functionName: "isAuthorizedKey", args: [keyID] });
    if (!authorized) throw new HttpError(403, "The catalog signer is not an authorized Builder DeviceKey");
    const canonicalBlock = await this.client.getBlock({ blockHash: receipt.blockHash });
    if (canonicalBlock.hash !== receipt.blockHash) {
      throw new HttpError(409, "The Registry transaction block is no longer canonical");
    }
    const blockTimestamp = chainTimestamp(canonicalBlock.timestamp);
    return { transactionHash, blockNumber: receipt.blockNumber.toString(), blockHash: receipt.blockHash,
      controller, head, checkpointSequence: sequence, signerKeyID: keyID, blockTimestamp,
      transactionIndex: matched.transactionIndex, logIndex: matched.logIndex };
  }

  async verifyBuilderKey(builderID: string, keyID: Hex): Promise<void> {
    const normalized = normalizeBuilder(builderID);
    const account = builderAddress(normalized);
    const code = await this.client.getCode({ address: account });
    if (!code || code === "0x") throw new HttpError(404, "BuilderAccount is not deployed");
    const authorized = await this.client.readContract({
      address: account, abi: ACCOUNT_ABI, functionName: "isAuthorizedKey", args: [keyID],
    });
    if (!authorized) throw new HttpError(403, "Profile signer is not an authorized Builder DeviceKey");
  }

  async revalidate(record: CatalogRecord): Promise<boolean> {
    try {
      if (verifyEnvelope(record.envelope, this.config) !== record.releaseDigest) return false;
      const release = releaseOf(record.envelope);
      const builder = normalizeBuilder(release.builder_id);
      const controller = builderAddress(builder);
      const shotID = normalizeHex32(release.shot_id);
      const head = normalizeDigest(release.public_checkpoint_digest);
      const sequence = positiveSafeInteger(release.checkpoint_sequence, "checkpoint_sequence");
      if (record.chain.controller !== controller || record.chain.head !== head
          || record.chain.checkpointSequence !== sequence) return false;
      const receipt = await this.client.getTransactionReceipt({ hash: record.chain.transactionHash });
      if (receipt.status !== "success"
          || receipt.to?.toLowerCase() !== this.config.registry.registryAddress
          || receipt.blockHash !== record.chain.blockHash
          || receipt.blockNumber.toString() !== record.chain.blockNumber) return false;
      const canonicalBlock = await this.client.getBlock({ blockNumber: receipt.blockNumber });
      if (canonicalBlock.hash !== record.chain.blockHash) return false;
      if (record.chain.blockTimestamp !== undefined
          && record.chain.blockTimestamp !== chainTimestamp(canonicalBlock.timestamp)) return false;
      const matched = receipt.logs.some((log) => {
        if (log.address.toLowerCase() !== this.config.registry.registryAddress) return false;
        try {
          const decoded = decodeEventLog({ abi: REGISTRY_ABI, data: log.data, topics: log.topics });
          const args = decoded.args as Record<string, unknown>;
          return (decoded.eventName === "ShotRegistered" && args.shotId === shotID && args.head === head)
            || (decoded.eventName === "CheckpointAppended" && args.shotId === shotID && args.newHead === head);
        } catch { return false; }
      });
      if (!matched) return false;
      const observed = await this.client.readContract({ address: this.config.registry.registryAddress,
        abi: REGISTRY_ABI, functionName: "getShot", args: [shotID] });
      if (observed.controller.toLowerCase() !== controller
          || Number(observed.checkpointSequence) < sequence
          || (Number(observed.checkpointSequence) === sequence && observed.head !== head)) return false;
      const signer = object(record.envelope.signer, "signer");
      const keyID = (`0x${bytesToHex(keccak_256(concat(hexBytes(signer.x), hexBytes(signer.y))))}`) as Hex;
      if (keyID !== record.chain.signerKeyID) return false;
      const authorized = await this.client.readContract({ address: controller, abi: ACCOUNT_ABI,
        functionName: "isAuthorizedKey", args: [keyID] });
      return authorized;
    } catch {
      return false;
    }
  }
}

type PendingStatus = "prepared" | "account_ready" | "committed" | "waiting_maturity" | "submitted" | "claims_submitted";

class PublicationPending extends Error {
  constructor(readonly status: PendingStatus) { super(status); }
}

export interface ConstrainedRelayer {
  address: Hex;
  advance(job: PublicationJob, staging: StagingRecord): Promise<void>;
}

function createRelayer(config: AppConfig): ConstrainedRelayer {
  const privateKey = config.registry.relayerPrivateKey;
  if (!privateKey) throw new Error("Registry relayer enabled without its dedicated key");
  const account = privateKeyToAccount(privateKey);
  const robinhood = defineChain({
    id: config.registry.chainId,
    name: "Robinhood Chain",
    nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
    rpcUrls: { default: { http: [config.registry.rpcUrl!] } },
  });
  const publicClient = createPublicClient({ chain: robinhood, transport: http(config.registry.rpcUrl!, { timeout: 10_000 }) });
  const wallet = createWalletClient({ account, chain: robinhood, transport: http(config.registry.rpcUrl!, { timeout: 10_000 }) });

  async function confirmed(hash: Hex): Promise<Awaited<ReturnType<typeof publicClient.getTransactionReceipt>>> {
    try {
      const receipt = await publicClient.getTransactionReceipt({ hash });
      if (receipt.status !== "success") throw new HttpError(422, "A constrained relayer transaction reverted");
      return receipt;
    } catch (error) {
      if (error instanceof HttpError) throw error;
      throw new PublicationPending("submitted");
    }
  }

  return {
    address: account.address,
    async advance(job, staging) {
      verifyRegistryAuthorization(job.registry, staging.envelope, config, job.registryTransactionHash !== undefined);
      const release = releaseOf(staging.envelope);
      const signer = object(job.registry.signer, "Registry signer");
      const action = object(job.registry.action, "Registry action");
      const x = normalizeDigest(signer.x);
      const y = normalizeDigest(signer.y);
      const keyID = `0x${bytesToHex(keccak_256(concat(hexBytes(x), hexBytes(y))))}` as Hex;
      const accountSalt = sha256Hex(concat(new TextEncoder().encode("TOHSENO-BUILDER-SALT-V1\0"), hexBytes(keyID)));
      const predicted = await publicClient.readContract({
        address: config.registry.factoryAddress, abi: FACTORY_ABI, functionName: "predictAccount",
        args: [accountSalt, BigInt(x), BigInt(y)],
      });
      const builder = normalizeBuilder(release.builder_id);
      if (`eip155:4663:${predicted.toLowerCase()}` !== builder) {
        throw new HttpError(422, "BuilderID is not the active factory prediction for this DeviceKey");
      }

      const code = await publicClient.getCode({ address: predicted });
      if (!code || code === "0x") {
        if (!job.accountTransactionHash) {
          job.accountTransactionHash = await wallet.writeContract({
            address: config.registry.factoryAddress, abi: FACTORY_ABI, functionName: "createAccount",
            args: [accountSalt, BigInt(x), BigInt(y)],
          });
          throw new PublicationPending("prepared");
        }
        await confirmed(job.accountTransactionHash);
        const deployed = await publicClient.getCode({ address: predicted });
        if (!deployed || deployed === "0x") throw new HttpError(422, "BuilderAccount deployment did not produce code");
      }
      job.status = "account_ready";

      const authorization = object(job.registry.authorization, "Registry authorization");
      const signature = object(authorization.signature, "Registry signature");
      const applicationSignature = `0x01${x.slice(2)}${y.slice(2)}${normalizeDigest(signature.r).slice(2)}${normalizeDigest(signature.s).slice(2)}` as Hex;
      const shotID = normalizeHex32(action.shot_id);
      const actionType = String(action.type);
      if (actionType === "REGISTER_SHOT") {
        const controller = normalizeAddress(action.controller);
        const head = normalizeDigest(action.head);
        const salt = normalizeDigest(action.salt);
        const nonce = nonnegativeSafeInteger(action.nonce, "Registry nonce");
        const deadline = positiveSafeInteger(action.deadline, "Registry deadline");
        const commitment = registrationCommitment(controller, shotID, salt, config.registry.registryAddress, deadline);
        const commitmentState = await publicClient.readContract({
          address: config.registry.registryAddress, abi: RELAYER_REGISTRY_ABI,
          functionName: "getCommitment", args: [commitment],
        });
        if (!commitmentState.exists) {
          if (!job.commitTransactionHash) {
            job.commitTransactionHash = await wallet.writeContract({
              address: config.registry.registryAddress, abi: RELAYER_REGISTRY_ABI,
              functionName: "commitShot", args: [commitment],
            });
            throw new PublicationPending("account_ready");
          }
          const receipt = await confirmed(job.commitTransactionHash);
          const block = await publicClient.getBlock({ blockHash: receipt.blockHash });
          job.committedAt = Number(block.timestamp);
        } else {
          job.committedAt = Number(commitmentState.committedAt);
        }
        job.status = "committed";
        if (Math.floor(Date.now() / 1000) < (job.committedAt ?? 0) + 60) {
          throw new PublicationPending("waiting_maturity");
        }
        if (!job.registryTransactionHash) {
          job.registryTransactionHash = await wallet.writeContract({
            address: config.registry.registryAddress, abi: RELAYER_REGISTRY_ABI,
            functionName: "registerShot",
            args: [{ shotId: shotID, controller, head, salt, nonce: BigInt(nonce), deadline: BigInt(deadline) }, applicationSignature],
          });
          throw new PublicationPending("submitted");
        }
      } else if (actionType === "APPEND_CHECKPOINT") {
        const previousHead = normalizeDigest(action.previous_head);
        const newHead = normalizeDigest(action.new_head);
        const sequence = positiveSafeInteger(action.checkpoint_sequence, "checkpoint sequence");
        const nonce = nonnegativeSafeInteger(action.nonce, "Registry nonce");
        const deadline = positiveSafeInteger(action.deadline, "Registry deadline");
        const observed = await publicClient.readContract({
          address: config.registry.registryAddress, abi: RELAYER_REGISTRY_ABI,
          functionName: "getShot", args: [shotID],
        });
        if (observed.controller.toLowerCase() !== predicted.toLowerCase()
            || observed.head !== previousHead
            || Number(observed.checkpointSequence) + 1 !== sequence
            || Number(observed.nonce) !== nonce) {
          throw new HttpError(409, "Shot moved after publication approval; review a fresh request");
        }
        if (!job.registryTransactionHash) {
          job.registryTransactionHash = await wallet.writeContract({
            address: config.registry.registryAddress, abi: RELAYER_REGISTRY_ABI,
            functionName: "appendCheckpoint",
            args: [{ shotId: shotID, previousHead, newHead, checkpointSequence: BigInt(sequence),
              nonce: BigInt(nonce), deadline: BigInt(deadline) }, applicationSignature],
          });
          throw new PublicationPending("submitted");
        }
      } else {
        throw new HttpError(422, "The relayer does not permit this Registry call");
      }
      await confirmed(job.registryTransactionHash!);
      job.status = "submitted";
    },
  };
}

function verifyRegistryAuthorization(
  value: JsonObject,
  envelope: JsonObject,
  config: AppConfig,
  allowExpired = false,
): Hex {
  exactKeys(value, ["schema", "domain", "action", "signer", "authorization"], "Registry authorization");
  if (value.schema !== "tohseno.registry-action/2") throw new HttpError(422, "unsupported Registry authorization schema");
  const domain = object(value.domain, "Registry domain");
  exactKeys(domain, ["name", "version", "chain_id", "verifying_contract"], "Registry domain");
  if (domain.name !== "TOHSENO ShotRegistry" || domain.version !== "2"
      || domain.chain_id !== config.registry.chainId || domain.verifying_contract !== config.registry.registryAddress) {
    throw new HttpError(422, "Registry signature domain is not the active generation");
  }
  const action = object(value.action, "Registry action");
  const release = releaseOf(envelope);
  const type = String(action.type);
  if (type === "REGISTER_SHOT") {
    exactKeys(action, ["type", "shot_id", "controller", "head", "salt", "nonce", "deadline"], "RegisterShot action");
    if (release.checkpoint_sequence !== 1 || action.nonce !== 0
        || action.shot_id !== release.shot_id || action.controller !== String(release.builder_id).split(":").at(-1)
        || action.head !== release.public_checkpoint_digest) {
      throw new HttpError(422, "RegisterShot action differs from the signed catalog release");
    }
    normalizeDigest(action.salt); normalizeAddress(action.controller);
  } else if (type === "APPEND_CHECKPOINT") {
    exactKeys(action, ["type", "shot_id", "previous_head", "new_head", "checkpoint_sequence", "nonce", "deadline"], "AppendCheckpoint action");
    if (action.shot_id !== release.shot_id || action.new_head !== release.public_checkpoint_digest
        || action.checkpoint_sequence !== release.checkpoint_sequence) {
      throw new HttpError(422, "AppendCheckpoint action differs from the signed catalog release");
    }
    normalizeDigest(action.previous_head);
  } else throw new HttpError(422, "The publication relayer permits only RegisterShot or AppendCheckpoint");
  normalizeHex32(action.shot_id);
  const deadline = positiveSafeInteger(action.deadline, "Registry deadline");
  const now = Math.floor(Date.now() / 1000);
  if ((!allowExpired && deadline <= now) || deadline > now + 24 * 60 * 60) {
    throw new HttpError(422, "Registry authorization expired or exceeds the 24-hour bound");
  }
  nonnegativeSafeInteger(action.nonce, "Registry nonce");

  const signer = object(value.signer, "Registry signer");
  exactKeys(signer, ["x", "y"], "Registry signer");
  const envelopeSigner = object(envelope.signer, "catalog signer");
  if (signer.x !== envelopeSigner.x || signer.y !== envelopeSigner.y) {
    throw new HttpError(422, "Registry and catalog signatures use different Builder DeviceKeys");
  }
  const authorization = object(value.authorization, "Registry authorization");
  exactKeys(authorization, ["algorithm", "digest", "signature", "low_s"], "Registry authorization");
  if (authorization.algorithm !== "p256" || authorization.low_s !== true) throw new HttpError(422, "Registry authorization must be low-s P-256");
  const digest = canonicalRegistryActionDigest(action, config.registry.registryAddress);
  if (authorization.digest !== digest) throw new HttpError(422, "Registry action digest differs from its structured action");
  const signature = object(authorization.signature, "Registry signature");
  exactKeys(signature, ["r", "s"], "Registry signature");
  const r = hexBytes(signature.r); const s = hexBytes(signature.s);
  if (BigInt(normalizeDigest(signature.s)) > HALF_ORDER) throw new HttpError(422, "Registry signature is not low-s");
  const publicKey = concat(new Uint8Array([4]), hexBytes(signer.x), hexBytes(signer.y));
  if (!p256.verify(concat(r, s), hexBytes(digest), publicKey, { prehash: false, lowS: true })) {
    throw new HttpError(403, "Registry authorization signature is invalid");
  }
  return digest;
}

export function canonicalRegistryActionDigest(action: JsonObject, registry: Hex): Hex {
  const domainType = keccak_256(new TextEncoder().encode("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"));
  const domain = keccak_256(concat(domainType, keccak_256(new TextEncoder().encode("TOHSENO ShotRegistry")),
    keccak_256(new TextEncoder().encode("2")), uintWord(4663), addressWord(registry)));
  const type = String(action.type);
  let words: Uint8Array[];
  if (type === "REGISTER_SHOT") {
    words = [hexBytes("0xc356ba3244a346558a5821261a4eccfb38382e0f90a60dc903003a671d5e828c"),
      hexBytes(action.shot_id), addressWord(normalizeAddress(action.controller)), hexBytes(action.head),
      hexBytes(action.salt), uintWord(nonnegativeSafeInteger(action.nonce, "Registry nonce")),
      uintWord(positiveSafeInteger(action.deadline, "Registry deadline"))];
  } else if (type === "APPEND_CHECKPOINT") {
    words = [hexBytes("0x4ada9482c2ee717b1b8faa0707d2096906a4cc7d3e9ab28cf94f2b8d220e22f5"),
      hexBytes(action.shot_id), hexBytes(action.previous_head), hexBytes(action.new_head),
      uintWord(positiveSafeInteger(action.checkpoint_sequence, "checkpoint sequence")),
      uintWord(nonnegativeSafeInteger(action.nonce, "Registry nonce")),
      uintWord(positiveSafeInteger(action.deadline, "Registry deadline"))];
  } else throw new HttpError(422, "unsupported Registry action");
  const structHash = keccak_256(concat(...words));
  return `0x${bytesToHex(keccak_256(concat(new Uint8Array([0x19, 0x01]), domain, structHash)))}`;
}

function registrationCommitment(controller: Hex, shotID: Hex, salt: Hex, registry: Hex, deadline: number): Hex {
  const typeHash = hexBytes("0x916bdb07dc63f8f944e630d491d633db4e254b88c225dda462fbae8afc34e6e4");
  return `0x${bytesToHex(keccak_256(concat(typeHash, addressWord(controller), hexBytes(shotID), hexBytes(salt), addressWord(registry), uintWord(4663), uintWord(deadline))))}`;
}

function addressWord(value: Hex): Uint8Array { return concat(new Uint8Array(12), lowerHexBytes(value, 20)); }
function uintWord(value: number): Uint8Array { const result = new Uint8Array(32); let current = BigInt(value); for (let index = 31; index >= 0; index--) { result[index] = Number(current & 255n); current >>= 8n; } return result; }
function normalizeAddress(value: unknown): Hex { if (typeof value !== "string" || !ADDRESS.test(value) || value === `0x${"0".repeat(40)}`) throw new HttpError(422, "address is invalid"); return value as Hex; }
function lowerHexBytes(value: string, length: number): Uint8Array { if (!new RegExp(`^0x[0-9a-f]{${length * 2}}$`).test(value)) throw new HttpError(422, "hex value is invalid"); return Uint8Array.from(value.slice(2).match(/../g)!.map((byte) => Number.parseInt(byte, 16))); }
function nonnegativeSafeInteger(value: unknown, name: string): number { if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new HttpError(422, `${name} must be a nonnegative safe integer`); return value; }
function authorizeToken(request: Request, tokenSHA256: string): void { const token = request.headers.get("authorization")?.replace(/^Bearer /, "") ?? ""; const digest = sha256Hex(new TextEncoder().encode(token)).slice(2); if (!token || !timingSafeText(digest, tokenSHA256)) throw new HttpError(401, "Invalid publication authorization"); }
function publicPublicationJob(job: PublicationJob): JsonObject { return { schema: "tohseno.registry-publication-status/1", job_id: job.jobID, status: job.status,
  relayer_transactions: { account: job.accountTransactionHash ?? null, commitment: job.commitTransactionHash ?? null,
    registry: job.registryTransactionHash ?? null, claims: job.claimsTransactionHash ?? null },
  public_release: job.publicRecord ? publicRecordProjection(job.publicRecord) : null,
  failure: job.failure ?? null, updated_at: job.updatedAt }; }

function verifyEnvelope(envelope: JsonObject, config: AppConfig): Hex {
  exactKeys(envelope, ["schema", "release", "signer", "authorization"], "signed release");
  if (envelope.schema !== SIGNED_SCHEMA) throw new HttpError(422, `schema must be ${SIGNED_SCHEMA}`);
  const release = releaseOf(envelope);
  exactKeys(release, ["schema", "generation", "shot_id", "builder_id", "release_id", "published_at",
    "display", "source", "build", "permissions", "parent", "checkpoint_sequence", "public_checkpoint_digest"], "release");
  if (release.schema !== RELEASE_SCHEMA) throw new HttpError(422, `release.schema must be ${RELEASE_SCHEMA}`);
  const generation = object(release.generation, "release.generation");
  exactKeys(generation, ["contract_generation", "chain_id", "builder_account_factory", "shot_registry", "activation_signing_digest"], "generation");
  if (generation.contract_generation !== "0.8.0" || generation.chain_id !== config.registry.chainId
      || generation.builder_account_factory !== config.registry.factoryAddress
      || generation.shot_registry !== config.registry.registryAddress
      || generation.activation_signing_digest !== config.registry.activationSigningDigest) {
    throw new HttpError(422, "release generation does not match the client-trusted active generation");
  }
  normalizeHex32(release.shot_id); normalizeBuilder(release.builder_id); normalizeHex32(release.release_id);
  normalizeHex32(release.public_checkpoint_digest); positiveSafeInteger(release.checkpoint_sequence, "checkpoint_sequence");
  const published = typeof release.published_at === "string" ? Date.parse(release.published_at) : Number.NaN;
  if (typeof release.published_at !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(release.published_at)
      || !Number.isFinite(published) || published <= 0 || published > Date.now() + 5 * 60 * 1000) {
    throw new HttpError(422, "published_at is not valid canonical UTC");
  }
  validateDisplay(object(release.display, "release.display"));
  validateSource(object(release.source, "release.source"));
  validateBuild(object(release.build, "release.build"));
  validatePermissions(object(release.permissions, "release.permissions"));
  if (release.parent !== null) validateParent(object(release.parent, "release.parent"), release.shot_id);
  const signer = object(envelope.signer, "signer");
  exactKeys(signer, ["x", "y"], "signer");
  const x = hexBytes(signer.x); const y = hexBytes(signer.y);
  const authorization = object(envelope.authorization, "authorization");
  exactKeys(authorization, ["algorithm", "digest", "signature", "low_s"], "authorization");
  if (authorization.algorithm !== "p256" || authorization.low_s !== true) throw new HttpError(422, "authorization must be a low-s P-256 signature");
  const digest = sha256Hex(new TextEncoder().encode(canonicalCatalogJSON(release)));
  if (authorization.digest !== digest) throw new HttpError(422, "authorization digest differs from canonical release");
  const signature = object(authorization.signature, "authorization.signature");
  exactKeys(signature, ["r", "s"], "signature");
  const r = hexBytes(signature.r); const s = hexBytes(signature.s);
  if (BigInt(normalizeDigest(signature.s)) > HALF_ORDER) throw new HttpError(422, "signature is not low-s");
  const publicKey = concat(new Uint8Array([4]), x, y);
  const compact = concat(r, s);
  if (!p256.verify(compact, hexBytes(digest), publicKey, { prehash: false, lowS: true })) throw new HttpError(403, "catalog signature is invalid");
  return digest;
}

function verifySignedProfile(
  envelope: JsonObject,
  expectedBuilder: string,
): { digest: Hex; keyID: Hex } {
  const verified = verifySignedObject(envelope, "profile", SIGNED_PROFILE_SCHEMA);
  const profile = object(envelope.profile, "profile");
  exactKeys(profile, ["schema", "builder_id", "display_name", "handle", "avatar_sha256",
    "external_attestations", "updated_at", "nonce"], "profile");
  if (profile.schema !== PROFILE_SCHEMA || normalizeBuilder(profile.builder_id) !== expectedBuilder) {
    throw new HttpError(422, "Profile identity differs from its Builder route");
  }
  boundedText(profile.display_name, 1, 80, "profile.display_name");
  if (profile.handle !== null) normalizeName(profile.handle, "profile.handle", 32);
  if (profile.avatar_sha256 !== null) normalizeDigest(profile.avatar_sha256);
  positiveSafeInteger(profile.nonce, "profile.nonce");
  canonicalRecentTimestamp(profile.updated_at, "profile.updated_at", 24 * 60 * 60);
  if (!Array.isArray(profile.external_attestations) || profile.external_attestations.length > 8) {
    throw new HttpError(422, "profile.external_attestations is invalid");
  }
  if (profile.external_attestations.length > 0) {
    throw new HttpError(503, "External identity verification is not configured; unverified attestations are refused");
  }
  for (const value of profile.external_attestations) {
    const attestation = object(value, "profile attestation");
    exactKeys(attestation, ["provider", "subject", "proof_url", "verified_at"], "profile attestation");
    if (attestation.provider !== "x") throw new HttpError(422, "unsupported profile attestation provider");
    boundedText(attestation.subject, 1, 64, "profile attestation subject");
    const proof = new URL(String(attestation.proof_url));
    if (proof.protocol !== "https:" || proof.username || proof.password || proof.hash) {
      throw new HttpError(422, "profile attestation proof URL is invalid");
    }
    canonicalRecentTimestamp(attestation.verified_at, "profile attestation time", 366 * 24 * 60 * 60);
  }
  return verified;
}

function verifySignedAliasClaim(envelope: JsonObject): { digest: Hex; keyID: Hex } {
  const verified = verifySignedObject(envelope, "claim", SIGNED_ALIAS_CLAIM_SCHEMA);
  const claim = object(envelope.claim, "alias claim");
  exactKeys(claim, ["schema", "builder_id", "shot_id", "alias", "request_id", "nonce",
    "deadline", "requested_at"], "alias claim");
  if (claim.schema !== ALIAS_CLAIM_SCHEMA) throw new HttpError(422, "unsupported alias claim schema");
  normalizeBuilder(claim.builder_id); normalizeHex32(claim.shot_id); normalizeHex32(claim.request_id);
  normalizeGlobalAlias(claim.alias);
  positiveSafeInteger(claim.nonce, "alias nonce");
  canonicalRecentTimestamp(claim.requested_at, "alias requested_at", 60 * 60);
  const deadline = positiveSafeInteger(claim.deadline, "alias deadline");
  const now = Math.floor(Date.now() / 1000);
  if (deadline <= now || deadline > now + 60 * 60) throw new HttpError(422, "alias claim deadline is stale or too far ahead");
  return verified;
}

function verifyStoredAliasClaim(record: AliasClaimRecord): { digest: Hex; keyID: Hex } {
  const verified = verifySignedObject(record.envelope, "claim", SIGNED_ALIAS_CLAIM_SCHEMA);
  const claim = object(record.envelope.claim, "alias claim");
  exactKeys(claim, ["schema", "builder_id", "shot_id", "alias", "request_id", "nonce",
    "deadline", "requested_at"], "alias claim");
  if (claim.schema !== ALIAS_CLAIM_SCHEMA) throw new HttpError(422, "unsupported alias claim schema");
  normalizeBuilder(claim.builder_id); normalizeHex32(claim.shot_id); normalizeHex32(claim.request_id);
  normalizeGlobalAlias(claim.alias); positiveSafeInteger(claim.nonce, "alias nonce");
  const requestedAt = canonicalTimestampSeconds(claim.requested_at, "alias requested_at");
  const receivedAt = recordedTimestampSeconds(record.received_at, "alias received_at");
  const deadline = positiveSafeInteger(claim.deadline, "alias deadline");
  if (receivedAt < requestedAt - 5 * 60 || receivedAt > requestedAt + 65 * 60
      || receivedAt > deadline) {
    throw new HttpError(409, "Stored alias request was not accepted inside its signed window");
  }
  return verified;
}

function verifySignedObject(
  envelope: JsonObject,
  payloadKey: "profile" | "claim",
  expectedSchema: string,
): { digest: Hex; keyID: Hex } {
  exactKeys(envelope, ["schema", payloadKey, "signer", "authorization"], `signed ${payloadKey}`);
  if (envelope.schema !== expectedSchema) throw new HttpError(422, `unsupported signed ${payloadKey} schema`);
  const payload = object(envelope[payloadKey], payloadKey);
  const signer = object(envelope.signer, `${payloadKey} signer`);
  exactKeys(signer, ["x", "y"], `${payloadKey} signer`);
  const x = hexBytes(signer.x); const y = hexBytes(signer.y);
  const authorization = object(envelope.authorization, `${payloadKey} authorization`);
  exactKeys(authorization, ["algorithm", "digest", "signature", "low_s"], `${payloadKey} authorization`);
  if (authorization.algorithm !== "p256" || authorization.low_s !== true) {
    throw new HttpError(422, `${payloadKey} authorization must be low-s P-256`);
  }
  const digest = sha256Hex(new TextEncoder().encode(canonicalCatalogJSON(payload)));
  if (authorization.digest !== digest) throw new HttpError(422, `${payloadKey} digest differs from its canonical payload`);
  const signature = object(authorization.signature, `${payloadKey} signature`);
  exactKeys(signature, ["r", "s"], `${payloadKey} signature`);
  const r = hexBytes(signature.r); const s = hexBytes(signature.s);
  if (BigInt(normalizeDigest(signature.s)) > HALF_ORDER) throw new HttpError(422, `${payloadKey} signature is not low-s`);
  const publicKey = concat(new Uint8Array([4]), x, y);
  if (!p256.verify(concat(r, s), hexBytes(digest), publicKey, { prehash: false, lowS: true })) {
    throw new HttpError(403, `${payloadKey} signature is invalid`);
  }
  return { digest, keyID: `0x${bytesToHex(keccak_256(concat(x, y)))}` as Hex };
}

async function verifyCurrentBuilderKey(chain: ChainVerifier, builder: string, keyID: Hex): Promise<void> {
  if (!chain.verifyBuilderKey) throw new HttpError(503, "Current Builder authority verification is unavailable");
  await chain.verifyBuilderKey(builder, keyID);
}

function canonicalRecentTimestamp(value: unknown, name: string, maximumAgeSeconds: number): void {
  const seconds = canonicalTimestampSeconds(value, name);
  const now = Date.now() / 1000;
  if (seconds < now - maximumAgeSeconds || seconds > now + 5 * 60) {
    throw new HttpError(422, `${name} is stale or in the future`);
  }
}

function canonicalTimestampSeconds(value: unknown, name: string): number {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(value)) {
    throw new HttpError(422, `${name} is not canonical UTC`);
  }
  const seconds = Date.parse(value) / 1000;
  if (!Number.isFinite(seconds)) throw new HttpError(422, `${name} is not a real timestamp`);
  return seconds;
}

function recordedTimestampSeconds(value: unknown, name: string): number {
  if (typeof value !== "string"
      || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(value)) {
    throw new HttpError(422, `${name} is not canonical UTC`);
  }
  const seconds = Date.parse(value) / 1000;
  if (!Number.isFinite(seconds)) throw new HttpError(422, `${name} is not a real timestamp`);
  return seconds;
}

function validateDisplay(value: JsonObject): void {
  exactKeys(value, ["name", "description", "icon_sha256", "builder_handle", "app_slug"], "display");
  boundedText(value.name, 1, 160, "display.name"); boundedText(value.description, 1, 2000, "display.description");
  if (value.icon_sha256 !== null) normalizeHex32(value.icon_sha256);
  for (const [field, maximum] of [["builder_handle", 32], ["app_slug", 64]] as const) {
    const item = value[field]; if (item !== null && (typeof item !== "string" || item.length < 2 || item.length > maximum || !IDENTIFIER.test(item))) throw new HttpError(422, `display.${field} is invalid`);
  }
}

function validateSource(value: JsonObject): void {
  exactKeys(value, ["format", "sha256", "byte_length", "source_tree_sha256", "file_count", "uncompressed_byte_length"], "source");
  if (value.format !== "deterministic_tar") throw new HttpError(422, "unsupported source format");
  normalizeHex32(value.sha256); normalizeHex32(value.source_tree_sha256);
  const bytes = positiveSafeInteger(value.byte_length, "source.byte_length");
  const files = positiveSafeInteger(value.file_count, "source.file_count");
  const expanded = positiveSafeInteger(value.uncompressed_byte_length, "source.uncompressed_byte_length");
  if (bytes > MAX_SOURCE_BYTES || files > 100_000 || expanded > 2 * 1024 * 1024 * 1024) throw new HttpError(422, "source bounds are invalid");
}

function validateBuild(value: JsonObject): void {
  exactKeys(value, ["container_kind", "container_path", "scheme", "original_bundle_identifier", "minimum_ios", "device_families", "dependency_locks", "safety"], "build");
  if (value.container_kind !== "project" && value.container_kind !== "workspace") throw new HttpError(422, "build.container_kind is invalid");
  safeRelativePath(value.container_path, "build.container_path"); boundedText(value.scheme, 1, 256, "build.scheme");
  if (typeof value.original_bundle_identifier !== "string" || !/^[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+$/.test(value.original_bundle_identifier)) throw new HttpError(422, "bundle identifier is invalid");
  if (typeof value.minimum_ios !== "string" || !/^\d+(?:\.\d+)*$/.test(value.minimum_ios)) throw new HttpError(422, "minimum iOS is invalid");
  if (!Array.isArray(value.device_families) || !value.device_families.length || value.device_families.length > 8) throw new HttpError(422, "device families are invalid");
  sortedUniqueStrings(value.device_families, "device families");
  if (!Array.isArray(value.dependency_locks) || value.dependency_locks.length > 128) throw new HttpError(422, "dependency locks are invalid");
  let prior = "";
  for (const entry of value.dependency_locks) { const lock = object(entry, "dependency lock"); exactKeys(lock, ["path", "sha256"], "dependency lock"); safeRelativePath(lock.path, "dependency lock path"); normalizeHex32(lock.sha256); if ((lock.path as string) <= prior) throw new HttpError(422, "dependency locks are not sorted"); prior = lock.path as string; }
  const safety = object(value.safety, "build.safety"); exactKeys(safety, ["classification", "reasons"], "build.safety");
  if (!["green", "requires_mac_review", "unsupported"].includes(String(safety.classification))) throw new HttpError(422, "build safety is invalid");
  if (!Array.isArray(safety.reasons) || safety.reasons.length > 64) throw new HttpError(422, "build safety reasons are invalid"); sortedUniqueStrings(safety.reasons, "build safety reasons");
  if ((safety.classification === "green") !== (safety.reasons.length === 0)) throw new HttpError(422, "build safety classification and reasons disagree");
}

function validatePermissions(value: JsonObject): void {
  exactKeys(value, ["install_allowed", "fork_allowed", "distributor_rights_declared", "spdx_license"], "permissions");
  if (value.install_allowed !== true || value.distributor_rights_declared !== true || typeof value.fork_allowed !== "boolean") throw new HttpError(422, "release permissions are invalid");
  if (value.spdx_license !== null && (typeof value.spdx_license !== "string" || !/^[A-Za-z0-9.+() -]{1,96}$/.test(value.spdx_license))) throw new HttpError(422, "SPDX license is invalid");
}

function validateParent(value: JsonObject, shotID: unknown): void {
  exactKeys(value, ["parent_shot_id", "parent_release_digest"], "parent");
  if (normalizeHex32(value.parent_shot_id) === shotID) throw new HttpError(422, "a release cannot be its own parent");
  normalizeHex32(value.parent_release_digest);
}

export function canonicalCatalogJSON(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number") { if (!Number.isSafeInteger(value)) throw new HttpError(422, "canonical JSON numbers must be safe integers"); return JSON.stringify(value); }
  if (Array.isArray(value)) return `[${value.map(canonicalCatalogJSON).join(",")}]`;
  if (typeof value === "object") { const record = value as JsonObject; return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalCatalogJSON(record[key])}`).join(",")}}`; }
  throw new HttpError(422, "catalog contains a non-JSON value");
}

async function authorizedStaging(request: Request, root: string, id: string): Promise<StagingRecord> {
  const record = await readJSON<StagingRecord>(join(root, `${id}.json`));
  if (!record || record.schema !== STAGING_SCHEMA || new Date(record.expiresAt) <= new Date()) throw new HttpError(404, "Staging reservation not found");
  const token = request.headers.get("authorization")?.replace(/^Bearer /, "") ?? "";
  const digest = sha256Hex(new TextEncoder().encode(token)).slice(2);
  if (!token || !timingSafeText(digest, record.tokenSHA256)) throw new HttpError(401, "Invalid staging authorization");
  return record;
}

async function writeBoundedBody(request: Request, path: string, maximum: number): Promise<Hex> {
  const length = Number(request.headers.get("content-length") ?? "NaN");
  if (!Number.isSafeInteger(length) || length < 1 || length > maximum) throw new HttpError(413, "Blob size is missing or outside its bound");
  const file = await open(path, "wx", 0o600);
  const hasher = new Bun.CryptoHasher("sha256");
  let count = 0;
  try {
    const reader = request.body?.getReader(); if (!reader) throw new HttpError(400, "Blob body is required");
    while (true) { const { done, value } = await reader.read(); if (done) break; count += value.byteLength; if (count > maximum || count > length) throw new HttpError(413, "Blob exceeded its declared bound"); hasher.update(value); await file.write(value); }
    if (count !== length) throw new HttpError(400, "Blob length differs from Content-Length");
    await file.sync(); return `0x${hasher.digest("hex")}`;
  } catch (error) { await rm(path, { force: true }); throw error; }
  finally { await file.close(); }
}

async function updateIndexes(directories: Record<string, string>, record: CatalogRecord): Promise<void> {
  const release = releaseOf(record);
  const shotID = normalizeHex32(release.shot_id).slice(2);
  const builder = normalizeBuilder(release.builder_id).split(":").at(-1)!;
  for (const [directory, key] of [[directories.shots, shotID], [directories.builders, builder]] as const) {
    const path = join(directory, `${key}.json`);
    const existing = await readJSON<{ releases: string[] }>(path);
    const releases = [...new Set([...(existing?.releases ?? []), record.releaseDigest])];
    await atomicJSON(path, { schema: "tohseno.catalog-index/1", latest: record.releaseDigest, releases }, false);
  }
}

async function latestForShot(directories: Record<string, string>, shotID: Hex): Promise<CatalogRecord | undefined> {
  const index = await readJSON<{ latest: string }>(join(directories.shots, `${shotID.slice(2)}.json`));
  return index ? readJSON<CatalogRecord>(join(directories.releases, `${index.latest.slice(2)}.json`)) : undefined;
}

async function profileForBuilder(root: string, builder: string): Promise<SignedProfileRecord | undefined> {
  const address = normalizeBuilder(builder).split(":").at(-1)!.slice(2);
  const record = await readJSON<SignedProfileRecord>(join(root, `${address}.json`));
  return record?.schema === "tohseno.builder-profile-record/1" ? record : undefined;
}

async function allRecords(directory: string): Promise<CatalogRecord[]> {
  const names = await readdir(directory).catch(() => [] as string[]); const records: CatalogRecord[] = [];
  for (const name of names.slice(0, 10_000)) { if (!/^[0-9a-f]{64}\.json$/.test(name)) continue; const value = await readJSON<CatalogRecord>(join(directory, name)); if (value?.schema === RECORD_SCHEMA) records.push(value); }
  return records;
}

async function requireBuilderLocalSlugAvailable(directory: string, release: JsonObject): Promise<void> {
  const display = object(release.display, "release.display");
  if (display.app_slug === null) return;
  const builder = normalizeBuilder(release.builder_id);
  const shotID = normalizeHex32(release.shot_id);
  const slug = normalizeName(display.app_slug, "display.app_slug", 64);
  const collision = (await allRecords(directory)).some((record) => {
    const published = releaseOf(record);
    return published.builder_id === builder
      && published.shot_id !== shotID
      && object(published.display, "release.display").app_slug === slug;
  });
  if (collision) throw new HttpError(409, "App slug is already used by another Shot from this Builder");
}

async function requireBuilderLocalSlugAvailableInStaging(directory: string, release: JsonObject): Promise<void> {
  const display = object(release.display, "release.display");
  if (display.app_slug === null) return;
  const builder = normalizeBuilder(release.builder_id);
  const shotID = normalizeHex32(release.shot_id);
  const slug = normalizeName(display.app_slug, "display.app_slug", 64);
  const names = await readdir(directory).catch(() => [] as string[]);
  for (const name of names.slice(0, 10_000)) {
    if (!/^[0-9a-f]{32}\.json$/.test(name)) continue;
    const staged = await readJSON<StagingRecord>(join(directory, name));
    if (staged?.schema !== STAGING_SCHEMA || new Date(staged.expiresAt) <= new Date()) continue;
    const pending = releaseOf(staged.envelope);
    if (pending.builder_id === builder && pending.shot_id !== shotID
        && object(pending.display, "release.display").app_slug === slug) {
      throw new HttpError(409, "App slug is already reserved by another Shot from this Builder");
    }
  }
}

function publicChainEvidence(chain: ChainEvidence | JsonObject): JsonObject { return {
  transactionHash: chain.transactionHash,
  blockNumber: chain.blockNumber,
  blockHash: chain.blockHash,
  controller: chain.controller,
  head: chain.head,
  checkpointSequence: chain.checkpointSequence,
  signerKeyId: chain.signerKeyID,
}; }
function publicRecordProjection(record: JsonObject): JsonObject { return {
  schema: record.schema,
  release_digest: record.release_digest,
  route: record.route,
  release: record.release,
  chain: publicChainEvidence(object(record.chain, "public release chain")),
  manifest_url: record.manifest_url,
  source_url: record.source_url,
  icon_url: record.icon_url,
}; }
function publicRecord(record: CatalogRecord): JsonObject { return { schema: "tohseno.public-catalog-release/1", release_digest: record.releaseDigest,
  route: record.route, release: releaseOf(record), chain: publicChainEvidence(record.chain),
  manifest_url: `/api/registry/v1/releases/${record.releaseDigest}`,
  source_url: record.sourceURL, icon_url: record.iconURL ?? null } }
function releaseEvidence(record: CatalogRecord): JsonObject { return {
  schema: "tohseno.public-release-evidence/1",
  release_digest: record.releaseDigest,
  signed_manifest: record.envelope,
  chain: publicChainEvidence(record.chain),
  source_url: record.sourceURL,
  icon_url: record.iconURL ?? null,
} }
function releaseOf(value: JsonObject | CatalogRecord): JsonObject { const envelope = "envelope" in value ? object(value.envelope, "envelope") : value; return object(envelope.release, "release"); }
function newestFirst(a: CatalogRecord, b: CatalogRecord): number { return String(releaseOf(b).published_at).localeCompare(String(releaseOf(a).published_at)); }
function canonicalRoute(release: JsonObject): string { return `/s/${String(release.shot_id).slice(2)}`; }

function latestPerShot(records: CatalogRecord[]): CatalogRecord[] {
  const seen = new Set<string>();
  return records.filter((record) => {
    const shotID = String(releaseOf(record).shot_id);
    if (seen.has(shotID)) return false;
    seen.add(shotID);
    return true;
  });
}

async function timelineEvents(
  records: CatalogRecord[],
  claims?: ClaimsPublicationBridge,
): Promise<TimelineEvent[]> {
  const unique = new Map<string, TimelineEvent>();
  for (const record of records) {
    const release = releaseOf(record);
    const shotID = normalizeHex32(release.shot_id);
    const sequence = positiveSafeInteger(release.checkpoint_sequence, "checkpoint_sequence");
    const parentValue = release.parent;
    const parent = parentValue === null ? null : (() => {
      const value = object(parentValue, "release.parent");
      return {
        shot_id: normalizeHex32(value.parent_shot_id),
        release_digest: normalizeDigest(value.parent_release_digest),
      };
    })();
    const kind = sequence === 1 ? "shot.shipped" : "shot.updated";
    const eventID = sha256Hex(new TextEncoder().encode([
      "TOHSENO-REGISTRY-TIMELINE-V1", kind, shotID, record.releaseDigest,
      String(sequence), record.chain.blockHash,
    ].join("\0")));
    unique.set(eventID, {
      schema: "tohseno.timeline-event/1",
      event_id: eventID,
      kind,
      shot_id: shotID,
      builder_id: normalizeBuilder(release.builder_id),
      release_digest: record.releaseDigest,
      checkpoint_sequence: sequence,
      occurred_at: record.chain.blockTimestamp ?? String(release.published_at),
      canonical_block: { number: record.chain.blockNumber, hash: record.chain.blockHash,
        transaction_index: record.chain.transactionIndex ?? null,
        log_index: record.chain.logIndex ?? null },
      parent: sequence === 1 ? parent : null,
    });
    if (sequence === 1 && parent) {
      const forkEventID = sha256Hex(new TextEncoder().encode([
        "TOHSENO-REGISTRY-TIMELINE-V1", "shot.forked", shotID, record.releaseDigest,
        parent.shot_id, parent.release_digest, record.chain.blockHash,
      ].join("\0")));
      unique.set(forkEventID, { schema: "tohseno.timeline-event/1", event_id: forkEventID,
        kind: "shot.forked", shot_id: shotID, builder_id: normalizeBuilder(release.builder_id),
        release_digest: record.releaseDigest, checkpoint_sequence: sequence,
        occurred_at: record.chain.blockTimestamp ?? String(release.published_at), canonical_block: {
          number: record.chain.blockNumber, hash: record.chain.blockHash,
          transaction_index: record.chain.transactionIndex ?? null,
          log_index: record.chain.logIndex ?? null }, parent });
    }
  }
  if (claims) {
    await Promise.all(latestPerShot([...records].sort(newestFirst)).slice(0, 10_000).map(async (record) => {
      const release = releaseOf(record);
      const shotID = normalizeHex32(release.shot_id);
      const closure = await claims.closureForTimeline(shotID);
      if (!closure) return;
      const eventID = sha256Hex(new TextEncoder().encode([
        "TOHSENO-REGISTRY-TIMELINE-V1", "claim.edition_closed", shotID,
        closure.reason, closure.canonicalBlock.number, closure.canonicalBlock.hash,
        String(closure.canonicalBlock.transactionIndex), String(closure.canonicalBlock.logIndex),
      ].join("\0")));
      unique.set(eventID, { schema: "tohseno.timeline-event/1", event_id: eventID,
        kind: "claim.edition_closed", shot_id: shotID,
        builder_id: normalizeBuilder(release.builder_id), release_digest: record.releaseDigest,
        checkpoint_sequence: positiveSafeInteger(release.checkpoint_sequence, "checkpoint_sequence"),
        occurred_at: closure.occurredAt,
        canonical_block: { number: closure.canonicalBlock.number, hash: closure.canonicalBlock.hash,
          transaction_index: closure.canonicalBlock.transactionIndex,
          log_index: closure.canonicalBlock.logIndex }, parent: null,
        closure_reason: closure.reason });
    }));
  }
  return [...unique.values()].sort(compareTimelineEvents);
}

function compareTimelineEvents(left: TimelineEvent, right: TimelineEvent): number {
  const block = BigInt(right.canonical_block.number) - BigInt(left.canonical_block.number);
  if (block !== 0n) return block > 0n ? 1 : -1;
  const rightTransaction = right.canonical_block.transaction_index ?? -1;
  const leftTransaction = left.canonical_block.transaction_index ?? -1;
  if (rightTransaction !== leftTransaction) return rightTransaction - leftTransaction;
  const rightLog = right.canonical_block.log_index ?? -1;
  const leftLog = left.canonical_block.log_index ?? -1;
  return rightLog - leftLog
    || right.checkpoint_sequence - left.checkpoint_sequence
    || right.event_id.localeCompare(left.event_id);
}

function timelinePage(events: TimelineEvent[], url: URL): JsonObject {
  const limit = boundedLimit(url.searchParams.get("limit"));
  const rawCursor = url.searchParams.get("cursor");
  let offset = 0;
  if (rawCursor !== null) {
    const cursor = normalizeDigest(rawCursor);
    const index = events.findIndex((event) => event.event_id === cursor);
    if (index < 0) throw new HttpError(400, "timeline cursor is invalid or no longer canonical");
    offset = index + 1;
  }
  const page = events.slice(offset, offset + limit);
  return {
    schema: "tohseno.registry-timeline-page/1",
    events: page,
    next_cursor: offset + page.length < events.length ? page.at(-1)!.event_id : null,
  };
}

function registryHTML(
  records: JsonObject[],
  query: string,
  launched: boolean,
  events: TimelineEvent[] = [],
  editions = new Map<string, { maxClaims: bigint; totalClaims: bigint; closed: boolean }>(),
): string {
  const lead = launched
    ? "Software enters this network once, changes through Updates, and moves person to person."
    : "Registry support is online in pre-launch verification mode. No public app or write path is claimed.";
  const status = launched ? "Public Registry live" : "Registry verification";
  return page("The Registry", `
    <section class="registry-hero">
      <div class="registry-hero-copy">
        <p class="eyebrow">PERSON-TO-PERSON NATIVE SOFTWARE</p>
        <h1>Software is alive here.</h1>
        <p class="lead">${lead}</p>
        <div class="registry-modes" aria-label="Registry views">
          <strong>Discover</strong>
          <span>Following <small>private</small></span>
          <span>Updates <small>private</small></span>
        </div>
        <form class="search" action="/registry" method="get">
          <label for="registry-search">Search apps, builders, or ShotID</label>
          <div>
            <input id="registry-search" name="q" maxlength="100" value="${escapeHTML(query)}" placeholder="Find software or a person">
            <button type="submit">Search <span aria-hidden="true">→</span></button>
          </div>
        </form>
      </div>
      <div class="registry-visual">
        <img src="/landing-assets/network.png" alt="The Tohseno mascot connecting Apple devices through the network">
        <span class="registry-signal registry-signal--status"><i aria-hidden="true"></i>${status}</span>
        <span class="registry-signal registry-signal--proof">Exact releases</span>
        <span class="registry-signal registry-signal--people">People move software</span>
      </div>
    </section>
    <section class="registry-pulse" aria-label="How the Registry works">
      <p><span class="live-dot" aria-hidden="true"></span>${launched ? "Discover is public" : "Verification is active"}</p>
      <ul><li>One Ship</li><li>Permanent Updates</li><li>Verifiable source</li><li>Recipient-signed</li></ul>
    </section>
    ${timelineCards(records, events, editions, launched)}
  `);
}
function shotHTML(
  record: JsonObject,
  edition?: { opened: boolean; maxClaims: bigint; totalClaims: bigint; closesAt: bigint; closed: boolean },
): string {
  const release = object(record.release, "release");
  const display = object(release.display, "display");
  const permissions = object(release.permissions, "permissions");
  const build = object(release.build, "build");
  const source = object(release.source, "source");
  const safety = object(build.safety, "build.safety");
  const exact = `${String(release.shot_id).slice(2)}?release=${String(record.release_digest)}`;
  const editionLabel = !edition?.opened
    ? "Claim Edition unavailable"
    : edition.maxClaims === 0n
      ? `Open Edition · ${edition.totalClaims} claimed`
      : `${edition.totalClaims} / ${edition.maxClaims} claimed`;
  const claimAction = edition?.opened
    ? edition.closed
      ? `<span class="primary disabled">Claim Edition closed</span>`
      : `<a class="primary" href="tohseno://claim/${exact}">On iPhone: open in Companion</a>`
    : `<span class="primary disabled">Claim is not available yet</span>`;
  const dependencyCount = Array.isArray(build.dependency_locks) ? build.dependency_locks.length : 0;
  const builderLabel = typeof display.builder_handle === "string"
    ? `@${display.builder_handle}`
    : compactBuilder(String(release.builder_id));
  const checkpointLabel = release.checkpoint_sequence === 1
    ? "Shipped once"
    : `Update ${escapeHTML(String(release.checkpoint_sequence))}`;
  const reviewCopy = "No DeviceKey-signed human Release Attestations are published by this Registry yet.";

  return page(String(display.name), `
    <a class="back" href="/registry">← Registry</a>
    <header class="app-hero">
      <p class="eyebrow">ONE EXACT PUBLIC RELEASE</p>
      <h1>${escapeHTML(String(display.name))}</h1>
      <p class="lead">${escapeHTML(String(display.description))}</p>
      <p class="edition">${escapeHTML(editionLabel)}</p>
      <p class="exact-release">Release ${escapeHTML(compactDigest(String(record.release_digest)))} · Checkpoint ${escapeHTML(String(release.checkpoint_sequence))}</p>
      <div class="actions">
        ${claimAction}
        <a href="/download/macos">On Mac: download Tohseno</a>
        <a href="${escapeHTML(String(record.source_url))}">Download public source</a>
      </div>
    </header>

    <section class="friend-path">
      <p class="eyebrow">GET IT ON YOUR IPHONE</p>
      <h2>Four small steps.</h2>
      <ol>
        <li><strong>Open this same link on your Mac.</strong><span>Download and install Tohseno. You need macOS 14 or later and the full Xcode app; the published download route is pinned to one signed, notarized DMG and exact SHA-256.</span></li>
        <li><strong>Pair your iPhone once.</strong><span>Open Tohseno on the Mac and follow setup. Apple may require a cable for first pairing; after pairing, Tohseno also uses Xcode-supported Wi-Fi reachability when available.</span></li>
        <li><strong>Open this link on that iPhone.</strong><span>Tap “On iPhone: open in Companion,” inspect the exact Builder and release, then draw the Claim circle. Claim is public; installation details stay private.</span></li>
        <li><strong>Let your Mac prepare it.</strong><span>Your Mac verifies the exact source, builds and signs its own copy with your Apple identity, keeps the artifact if needed, and installs when your paired iPhone is reachable and unlocked.</span></li>
      </ol>
      <p class="quiet">Send the address in this browser. The Claim button is pinned to the exact release shown above, even if the Builder publishes an Update later.</p>
    </section>

    <section class="evidence-grid">
      <article class="evidence-card">
        <p class="eyebrow">BUILDER + PROVENANCE</p>
        <h2><a href="/@${escapeHTML(String(release.builder_id))}">${escapeHTML(builderLabel)}</a></h2>
        <dl>
          <div><dt>Network event</dt><dd>${checkpointLabel}</dd></div>
          <div><dt>Release</dt><dd>${escapeHTML(compactDigest(String(record.release_digest)))}</dd></div>
          <div><dt>Source</dt><dd>${escapeHTML(compactDigest(String(source.sha256)))}</dd></div>
          <div><dt>Fork</dt><dd>${permissions.fork_allowed ? "Builder permits it after Claim" : "Not permitted"}</dd></div>
        </dl>
        <p>The Registry accepted this page only after the DeviceKey-signed manifest, current Builder authority, chain checkpoint, receipt, and source bytes agreed.</p>
      </article>
      <article class="evidence-card">
        <p class="eyebrow">MACHINE-READABLE FACTS</p>
        <h2>${escapeHTML(humanBuildClassification(String(safety.classification)))}</h2>
        <dl>
          <div><dt>Minimum iOS</dt><dd>${escapeHTML(String(build.minimum_ios))}</dd></div>
          <div><dt>Dependency locks</dt><dd>${dependencyCount}</dd></div>
          <div><dt>Install</dt><dd>${permissions.install_allowed ? "Declared allowed" : "Not allowed"}</dd></div>
          <div><dt>Signing</dt><dd>Your own Apple identity</dd></div>
        </dl>
        <p>These are bounded catalog/build observations, not a claim that the app is safe.</p>
      </article>
      <article class="evidence-card">
        <p class="eyebrow">NETWORK REVIEW</p>
        <h2>Not available for this release.</h2>
        <p>${reviewCopy}</p>
        <p>Claim means you encountered this exact release. It does not mean you reviewed, endorsed, installed, or certified it.</p>
      </article>
    </section>

    <section class="requirements">
      <p class="eyebrow">BEFORE YOU START</p>
      <h2>Apple’s security still applies.</h2>
      <p>You need a Mac with full Xcode, an Apple Account visible to Xcode, Developer Mode on the iPhone, Trust/pairing, and enough Personal Team capacity. Tohseno skips App Store submission; it does not bypass Apple signing or device security.</p>
    </section>

    <section class="timeline">
      <h2>Exact history</h2>
      <p>One birth, permanent Updates, and canonical Claim evidence for this Shot.</p>
      <a href="/api/registry/v1/shots/${release.shot_id}/timeline">View exact timeline evidence</a>
    </section>
  `);
}
function builderHTML(builder: string, records: JsonObject[], profile?: JsonObject): string { const title = profile ? String(profile.display_name) : builder; const handle = profile?.handle ? `<p class="eyebrow">@${escapeHTML(String(profile.handle))}</p>` : ""; const address = builder.split(":").at(-1)!; return page("Builder", `<a class="back" href="/registry">← Registry</a><header><p class="eyebrow">BUILDER</p><h1>${escapeHTML(title)}</h1>${handle}<p class="lead">A public track record assembled from a DeviceKey-signed profile, signed releases, and current chain authority.</p><p class="eyebrow">${escapeHTML(builder)}</p><div class="actions"><a class="primary" href="tohseno://follow/${escapeHTML(address)}">Follow privately in Tohseno</a></div><p>Follow state stays on your Mac and paired Companion. There is no public follower count.</p></header>${cards(records)}`); }
function cards(records: JsonObject[], launched = true): string {
  if (!records.length) {
    const title = launched ? "The network is ready." : "Pre-launch verification.";
    const copy = launched
      ? "The first independently verified release will appear here."
      : "Publication and public launch remain disabled until the signed Mac release and acceptance gates pass.";
    return `<section class="empty">
      <div class="empty-copy">
        <p class="eyebrow">${launched ? "THE NEXT SHIP STARTS HERE" : "NOT YET PUBLIC"}</p>
        <h2>${title}</h2>
        <p>${copy}</p>
      </div>
      <div class="empty-network" aria-hidden="true">
        <span class="empty-node empty-node--one"></span>
        <span class="empty-node empty-node--two"></span>
        <span class="empty-node empty-node--three"></span>
        <i></i><i></i>
      </div>
    </section>`;
  }
  return `<section class="grid">${records.map((record) => {
    const release = object(record.release, "release");
    const display = object(release.display, "display");
    return `<a class="card" href="${escapeHTML(String(record.route))}"><p class="eyebrow">SHOT ${escapeHTML(String(release.checkpoint_sequence))}</p><h2>${escapeHTML(String(display.name))}</h2><p>${escapeHTML(String(display.description))}</p><span>Open app →</span></a>`;
  }).join("")}</section>`;
}
function timelineCards(records: JsonObject[], events: TimelineEvent[], editions: Map<string, { maxClaims: bigint; totalClaims: bigint; closed: boolean }>, launched: boolean): string { if (!events.length) return cards([], launched); const byRelease = new Map(records.map((record) => [String(record.release_digest), record])); return `<section class="timeline-feed">${events.map((event) => { const record = byRelease.get(event.release_digest); if (!record) return ""; const release = object(record.release, "release"); const display = object(release.display, "display"); const edition = editions.get(event.shot_id); const action = event.kind === "shot.shipped" ? "entered Tohseno" : event.kind === "shot.updated" ? "updated" : event.kind === "shot.forked" ? "was born as a fork" : "Claim Edition closed"; const count = edition ? edition.maxClaims === 0n ? `Open Edition · ${edition.totalClaims} claimed` : `${edition.totalClaims} / ${edition.maxClaims} claimed${edition.closed ? " · edition closed" : ""}` : "Claim Edition activating"; return `<article class="network-event"><p class="eyebrow">${escapeHTML(event.kind.toUpperCase())}</p><h2><a href="${escapeHTML(String(record.route))}">${escapeHTML(String(display.name))}</a></h2><p class="event-action">${action}</p><p>${escapeHTML(count)}</p><a class="builder-link" href="/@${escapeHTML(event.builder_id)}">by ${escapeHTML(String(object(release.display, "display").builder_handle ?? compactBuilder(event.builder_id)))}</a><time>${escapeHTML(event.occurred_at)}</time></article>`; }).join("")}</section>`; }
function compactBuilder(value: string): string { return `…${value.slice(-10)}`; }
function compactDigest(value: string): string { return value.length > 22 ? `${value.slice(0, 12)}…${value.slice(-8)}` : value; }
function humanBuildClassification(value: string): string {
  if (value === "green") return "Automatic build profile";
  if (value === "requires_mac_review") return "Requires review on your Mac";
  return "Automatic build unavailable";
}
function page(title: string, body: string): string {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <meta name="theme-color" content="#f7f4ee">
  <meta name="description" content="Discover native Apple software moving person to person on Tohseno.">
  <link rel="icon" href="/tohseno-logo.png" type="image/png">
  <link rel="preload" href="/landing-assets/network.png" as="image" type="image/png">
  <link rel="stylesheet" href="/landing.css">
  <link rel="stylesheet" href="/registry.css">
  <title>${escapeHTML(title)} — Tohseno</title>
</head>
<body class="registry-page">
  <a class="skip-link" href="#main">Skip to the main content</a>
  <header class="site-header page-shell">
    <a class="wordmark" href="/" aria-label="Tohseno home"><img src="/landing-assets/wordmark.svg" alt="Tohseno"></a>
    <nav class="site-nav" aria-label="Primary">
      <a href="/">Network</a>
      <a href="/registry" aria-current="page">Registry</a>
      <a class="nav-action" href="/buy">$TOHSENO</a>
    </nav>
  </header>
  <main class="registry-main page-shell" id="main">${body}</main>
  <footer class="site-footer page-shell">
    <img src="/landing-assets/wordmark.svg" alt="Tohseno">
    <p>Software moving person to person.</p>
    <nav aria-label="Footer"><a href="/">Network</a><a href="/privacy">Privacy</a><a href="/buy">$TOHSENO</a></nav>
  </footer>
</body>
</html>`;
}

function unavailableRouter(): RegistryRouter { return { handles: (path) => path.startsWith("/api/registry/v1/"), fetch: async () => json({ error: "The public Registry is not enabled." }, 503), renderRegistry: async () => registryHTML([], "", false), renderShot: async () => undefined, renderBuilder: async () => undefined, renderHumanRoute: async () => undefined, currentClaimContext: async () => { throw new HttpError(503, "The public Registry is not enabled"); }, claimReceiptContext: async () => { throw new HttpError(503, "The public Registry is not enabled"); } }; }

class RateLimiter {
  private windows = new Map<string, { started: number; count: number }>();

  take(key: string, maximum: number, now = Date.now()): boolean {
    if (this.windows.size > 10_000) {
      for (const [candidate, window] of this.windows) {
        if (now - window.started >= 60_000) this.windows.delete(candidate);
      }
      if (this.windows.size > 10_000 && !this.windows.has(key)) return false;
    }
    const current = this.windows.get(key);
    if (!current || now - current.started >= 60_000) {
      this.windows.set(key, { started: now, count: 1 });
      return true;
    }
    if (current.count >= maximum) return false;
    current.count += 1;
    return true;
  }
}

function sourceKey(request: Request, config: AppConfig): string {
  if (!config.trustProxy) return "direct";
  const value = request.headers.get("cf-connecting-ip")
    ?? request.headers.get("x-forwarded-for")?.split(",", 1)[0]?.trim()
    ?? "unknown";
  return /^[0-9a-f:.]{2,64}$/i.test(value) ? `source:${value}` : "source:invalid";
}

async function collectExpiredStaging(root: string, now = Date.now()): Promise<void> {
  const names = await readdir(root).catch(() => [] as string[]);
  for (const name of names.slice(0, 10_000)) {
    if (!/^[0-9a-f]{32}\.json$/.test(name)) continue;
    const id = name.slice(0, 32);
    const record = await readJSON<StagingRecord>(join(root, name));
    if (!record || Date.parse(record.expiresAt) > now) continue;
    await Promise.all([
      rm(join(root, name), { force: true }),
      rm(join(root, `${id}.source`), { force: true }),
      rm(join(root, `${id}.icon`), { force: true }),
      rm(join(root, `${id}.source.partial`), { force: true }),
      rm(join(root, `${id}.icon.partial`), { force: true }),
    ]);
  }
}

async function requireStagingCapacity(
  root: string,
  nextRelease: JsonObject,
  config: AppConfig,
): Promise<void> {
  const names = (await readdir(root).catch(() => [] as string[]))
    .filter((name) => /^[0-9a-f]{32}\.json$/.test(name));
  if (names.length >= config.registry.maxStagingRecords) {
    throw new HttpError(503, "Registry staging capacity is full");
  }
  let declaredBytes = positiveSafeInteger(
    object(nextRelease.source, "release.source").byte_length,
    "source.byte_length",
  );
  for (const name of names.slice(0, config.registry.maxStagingRecords)) {
    const record = await readJSON<StagingRecord>(join(root, name));
    if (!record) continue;
    declaredBytes += positiveSafeInteger(
      object(releaseOf(record.envelope).source, "release.source").byte_length,
      "source.byte_length",
    );
    if (!Number.isSafeInteger(declaredBytes) || declaredBytes > config.registry.maxStagingBytes) {
      throw new HttpError(503, "Registry staging byte capacity is full");
    }
  }
}

function json(value: unknown, status = 200): Response { return withSecurityHeaders(new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" } })); }
function head(response: Response, method: string): Response { return method === "HEAD" ? new Response(null, { status: response.status, headers: response.headers }) : response; }
function methodNotAllowed(): Response { const response = json({ error: "Method not allowed" }, 405); response.headers.set("allow", "GET, HEAD, POST, PUT"); return response; }
function requireJSON(request: Request): void { if (request.headers.get("content-type")?.split(";", 1)[0] !== "application/json") throw new HttpError(415, "Content-Type must be application/json"); }
async function boundedJSON(request: Request, maximum: number): Promise<JsonObject> {
  const lengthHeader = request.headers.get("content-length");
  if (lengthHeader !== null && (!/^\d+$/.test(lengthHeader) || Number(lengthHeader) > maximum)) {
    throw new HttpError(413, "Request is too large");
  }
  const text = await request.text();
  if (!text || new TextEncoder().encode(text).length > maximum) {
    throw new HttpError(413, "Request is empty or too large");
  }
  try {
    rejectDuplicateJSONMembers(text);
    return object(JSON.parse(text), "request");
  } catch (error) {
    if (error instanceof HttpError) throw error;
    throw new HttpError(400, "Request body is not valid JSON");
  }
}

// JSON.parse keeps only the last occurrence of a repeated member. Walk the
// original transport first so escaped-equivalent keys are rejected before any
// typed validation or signature canonicalization can observe a lossy object.
function rejectDuplicateJSONMembers(text: string): void {
  let offset = 0;
  const whitespace = () => { while (/[\t\n\r ]/.test(text[offset] ?? "")) offset += 1; };
  const fail = (): never => { throw new HttpError(400, "Request body is not valid duplicate-free JSON"); };
  const string = (): string => {
    if (text[offset] !== '"') fail();
    const start = offset++;
    while (offset < text.length) {
      const character = text[offset++];
      if (character === '"') {
        try { return JSON.parse(text.slice(start, offset)) as string; } catch { fail(); }
      }
      if (character === "\\") {
        const escape = text[offset++];
        if (escape === "u") {
          if (!/^[0-9a-fA-F]{4}$/.test(text.slice(offset, offset + 4))) fail();
          offset += 4;
        } else if (!'"\\/bfnrt'.includes(escape ?? "")) fail();
      } else if ((character?.charCodeAt(0) ?? 0) < 0x20) fail();
    }
    return fail();
  };
  const literal = (expected: string) => {
    if (text.slice(offset, offset + expected.length) !== expected) fail();
    offset += expected.length;
  };
  const value = (depth: number): void => {
    if (depth > 128) fail();
    whitespace();
    if (text[offset] === "{") {
      offset += 1; whitespace();
      const keys = new Set<string>();
      if (text[offset] === "}") { offset += 1; return; }
      while (true) {
        const key = string();
        if (keys.has(key)) fail();
        keys.add(key); whitespace();
        if (text[offset++] !== ":") fail();
        value(depth + 1); whitespace();
        if (text[offset] === "}") { offset += 1; return; }
        if (text[offset++] !== ",") fail();
        whitespace();
      }
    }
    if (text[offset] === "[") {
      offset += 1; whitespace();
      if (text[offset] === "]") { offset += 1; return; }
      while (true) {
        value(depth + 1); whitespace();
        if (text[offset] === "]") { offset += 1; return; }
        if (text[offset++] !== ",") fail();
      }
    }
    if (text[offset] === '"') { string(); return; }
    for (const keyword of ["true", "false", "null"]) {
      if (text.startsWith(keyword, offset)) { literal(keyword); return; }
    }
    const number = text.slice(offset).match(/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/);
    if (!number) fail();
    offset += number![0].length;
  };
  whitespace(); value(0); whitespace();
  if (offset !== text.length) fail();
}
function object(value: unknown, name: string): JsonObject { if (!value || typeof value !== "object" || Array.isArray(value)) throw new HttpError(422, `${name} must be an object`); return value as JsonObject; }
function exactKeys(value: JsonObject, keys: readonly string[], name: string): void { const observed = Object.keys(value).sort(); const expected = [...keys].sort(); if (observed.length !== expected.length || observed.some((key, index) => key !== expected[index])) throw new HttpError(422, `${name} has unknown or missing fields`); }
function normalizeDigest(value: unknown): Hex { if (typeof value !== "string" || !HEX32.test(value)) throw new HttpError(422, "digest must be 32 lowercase hex bytes"); return value as Hex; }
function normalizeHex32(value: unknown): Hex { const result = normalizeDigest(value); if (/^0x0{64}$/.test(result)) throw new HttpError(422, "identifier must not be zero"); return result; }
function normalizeBuilder(value: unknown): string { if (typeof value !== "string" || !/^eip155:4663:0x[0-9a-f]{40}$/.test(value) || value.endsWith("0".repeat(40))) throw new HttpError(422, "builder_id is invalid"); return value; }
export function builderAddress(value: unknown): Hex {
  return normalizeBuilder(value).split(":").at(-1)! as Hex;
}
function normalizeName(value: unknown, name: string, maximum: number): string { if (typeof value !== "string" || value.length < 2 || value.length > maximum || !IDENTIFIER.test(value)) throw new HttpError(422, `${name} is invalid`); return value; }
function byteRange(value: string | null, size: number): { start: number; end: number } | undefined {
  if (value === null) return undefined;
  const matched = value.match(/^bytes=(0|[1-9][0-9]*)-(0|[1-9][0-9]*)$/);
  const start = matched ? Number(matched[1]) : Number.NaN;
  const end = matched ? Number(matched[2]) : Number.NaN;
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start > end || end >= size
      || end - start + 1 > 16 * 1024 * 1024) {
    throw new HttpError(416, "Requested blob range is invalid");
  }
  return { start, end };
}
function normalizeGlobalAlias(value: unknown): string {
  // Keep the signed global route compatible with CatalogDisplay.app_slug so
  // Companion can request the exact slug the Builder already approved.
  const alias = normalizeName(value, "alias", 64);
  if (["api", "claims", "docs", "download", "healthz", "install", "privacy", "registry",
    "releases", "s"].includes(alias)) throw new HttpError(422, "alias is reserved by the Tohseno website");
  return alias;
}
function searchableRelease(release: JsonObject): string { const display = object(release.display, "display"); return [display.name, display.description, display.builder_handle, display.app_slug, release.shot_id, release.builder_id].filter((value) => typeof value === "string").join("\n").toLocaleLowerCase("en-US"); }
function normalizeStagingID(value: string): string { if (!/^[0-9a-f]{32}$/.test(value)) throw new HttpError(404, "Staging reservation not found"); return value; }
function positiveSafeInteger(value: unknown, name: string): number { if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) throw new HttpError(422, `${name} must be a positive safe integer`); return value; }
function boundedText(value: unknown, minimum: number, maximum: number, name: string): void { if (typeof value !== "string" || new TextEncoder().encode(value).length < minimum || new TextEncoder().encode(value).length > maximum || /[\u0000-\u001f\u007f]/.test(value)) throw new HttpError(422, `${name} is outside its bound`); }
function safeRelativePath(value: unknown, name: string): void { if (typeof value !== "string" || !value || value.startsWith("/") || value.includes("\\") || value.split("/").some((part) => !part || part === "." || part === "..")) throw new HttpError(422, `${name} is not a safe relative path`); }
function sortedUniqueStrings(value: unknown[], name: string): void { let prior = ""; for (const item of value) { if (typeof item !== "string" || !item || item <= prior) throw new HttpError(422, `${name} must be sorted and unique`); prior = item; } }
function boundedLimit(value: string | null): number { if (value === null) return 50; if (!/^\d+$/.test(value)) throw new HttpError(400, "limit is invalid"); return Math.max(1, Math.min(100, Number(value))); }
function randomHex(bytes: number): string { const value = new Uint8Array(bytes); crypto.getRandomValues(value); return bytesToHex(value); }
function hexBytes(value: unknown): Uint8Array { return Uint8Array.from(normalizeDigest(value).slice(2).match(/../g)!.map((byte) => Number.parseInt(byte, 16))); }
function bytesToHex(value: Uint8Array): string { return [...value].map((byte) => byte.toString(16).padStart(2, "0")).join(""); }
function concat(...values: Uint8Array[]): Uint8Array { const result = new Uint8Array(values.reduce((sum, value) => sum + value.length, 0)); let offset = 0; for (const value of values) { result.set(value, offset); offset += value.length; } return result; }
function chainTimestamp(value: bigint): string {
  if (value < 0n || value > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("canonical block timestamp is invalid");
  return new Date(Number(value) * 1000).toISOString().replace(".000Z", "Z");
}
function sha256Hex(value: Uint8Array): Hex { return `0x${new Bun.CryptoHasher("sha256").update(value).digest("hex")}`; }
function timingSafeText(a: string, b: string): boolean { if (a.length !== b.length) return false; let difference = 0; for (let index = 0; index < a.length; index++) difference |= a.charCodeAt(index) ^ b.charCodeAt(index); return difference === 0; }
function blobPath(root: string, digest: Hex): string { return join(root, digest.slice(2, 4), digest.slice(4)); }
async function promote(source: string, destination: string, expected: Hex): Promise<void> {
  await mkdir(dirname(destination), { recursive: true, mode: 0o700 });
  if (await lstat(destination).catch(() => undefined)) {
    if (await fileSHA256(destination) !== expected) {
      throw new HttpError(500, "Existing content-addressed blob failed its digest");
    }
    await rm(source, { force: true });
    return;
  }
  await rename(source, destination);
}
async function fileSHA256(path: string): Promise<Hex> {
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new HttpError(500, "Content-addressed blob path is not a regular file");
  }
  const file = await open(path, "r");
  const hasher = new Bun.CryptoHasher("sha256");
  const buffer = new Uint8Array(1024 * 1024);
  try {
    while (true) {
      const { bytesRead } = await file.read(buffer, 0, buffer.length, null);
      if (bytesRead === 0) break;
      hasher.update(buffer.subarray(0, bytesRead));
    }
  } finally {
    await file.close();
  }
  return `0x${hasher.digest("hex")}`;
}
async function atomicJSON(path: string, value: unknown, exclusive: boolean): Promise<void> { const temporary = `${path}.${crypto.randomUUID()}.partial`; await writeFile(temporary, `${canonicalCatalogJSON(value)}\n`, { mode: 0o600, flag: "wx" }); if (exclusive && await stat(path).catch(() => undefined)) { await rm(temporary, { force: true }); throw new HttpError(409, "Record already exists"); } await rename(temporary, path); }
async function readJSON<T>(path: string): Promise<T | undefined> { try { const metadata = await stat(path); if (!metadata.isFile() || metadata.size > 1024 * 1024) throw new Error("invalid record"); return JSON.parse(await readFile(path, "utf8")) as T; } catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined; throw error; } }
function escapeHTML(value: string): string { return value.replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#39;" })[character]!); }
