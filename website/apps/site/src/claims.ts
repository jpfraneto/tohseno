import { lstat, mkdir, open, readFile, readdir, rename } from "node:fs/promises";
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
import { canonicalCatalogJSON } from "./registry.ts";
import { HttpError, withSecurityHeaders } from "./security.ts";

const CLAIMS_ACTIVATION_DOMAIN = new TextEncoder().encode("TOHSENO-CLAIMS-ACTIVATION-V1\0");
const CLAIM_MARK_DOMAIN = new TextEncoder().encode("TOHSENO-CLAIM-MARK-V1\0");
const ACCESSIBILITY_MARK_COMMITMENT = "0xa5a1280b3a8f8445b24ec2680524374720c25eff24f18cd2267fbf0f79d23bd4";
const ADDRESS = /^0x[0-9a-f]{40}$/;
const HEX32 = /^0x[0-9a-f]{64}$/;
const BUILDER_ID = /^eip155:4663:(0x[0-9a-f]{40})$/;
const HALF_ORDER = BigInt("0x7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8");
const CLAIMS_ABI = parseAbi([
  "function shotRegistry() view returns (address)",
  "function claimEdition(bytes32 shotId) view returns ((bool opened,uint64 maxClaims,uint64 totalClaims,uint64 openedAt,uint64 closesAt))",
  "function editionIsClosed(bytes32 shotId) view returns (bool)",
  "function claimTokenOf(bytes32 shotId,address claimant) view returns (uint256)",
  "function claimRecord(uint256 tokenId) view returns ((bytes32 shotId,uint64 claimNumber,address claimant,bytes32 releaseDigest,bytes32 checkpointDigest,bytes32 gestureCommitment))",
  "function ownerOf(uint256 tokenId) view returns (address)",
  "function tokenURI(uint256 tokenId) view returns (string)",
  "function editionNonces(address controller) view returns (uint64)",
  "function claimNonces(address claimant) view returns (uint64)",
  "function openClaimEdition((address shotRegistry,bytes32 shotId,uint64 maxClaims,uint64 closesAt,address controller,uint64 nonce,uint64 deadline) action,bytes signature)",
  "function claimSoftware((address shotRegistry,bytes32 shotId,address claimant,bytes32 releaseDigest,bytes32 checkpointDigest,bytes32 gestureCommitment,uint64 nonce,uint64 deadline) action,bytes signature) returns (uint256 tokenId)",
  "event ClaimEditionOpened(bytes32 indexed shotId,address indexed controller,uint64 maxClaims,uint64 opensAt,uint64 closesAt)",
  "event SoftwareClaimed(bytes32 indexed shotId,address indexed claimant,uint256 indexed tokenId,uint64 claimNumber,bytes32 releaseDigest,bytes32 checkpointDigest,bytes32 gestureCommitment)",
]);
const FACTORY_ABI = parseAbi([
  "function createAccount(bytes32 salt,uint256 initialX,uint256 initialY) returns (address account)",
  "function predictAccount(bytes32 salt,uint256 initialX,uint256 initialY) view returns (address predicted)",
]);
const DOMAIN_TYPE = "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const OPEN_EDITION_TYPE = "OpenClaimEdition(address shotRegistry,bytes32 shotId,uint64 maxClaims,uint64 closesAt,address controller,uint64 nonce,uint64 deadline)";
const CLAIM_SOFTWARE_TYPE = "ClaimSoftware(address shotRegistry,bytes32 shotId,address claimant,bytes32 releaseDigest,bytes32 checkpointDigest,bytes32 gestureCommitment,uint64 nonce,uint64 deadline)";

type JsonObject = Record<string, unknown>;

export type ClaimsAction = {
  type: "OPEN_CLAIM_EDITION";
  shot_registry: Hex; shot_id: Hex; max_claims: number; closes_at: number;
  controller: Hex; nonce: number; deadline: number;
} | {
  type: "CLAIM_SOFTWARE";
  shot_registry: Hex; shot_id: Hex; claimant: Hex; release_digest: Hex;
  checkpoint_digest: Hex; gesture_commitment: Hex; nonce: number; deadline: number;
};

export function canonicalClaimsActionDigest(action: ClaimsAction, claimsContract: Hex): Hex {
  const domain = keccak_256(concatBytes(
    keccak_256(new TextEncoder().encode(DOMAIN_TYPE)),
    keccak_256(new TextEncoder().encode("TOHSENO Claims")),
    keccak_256(new TextEncoder().encode("1")),
    uintWord(4663),
    addressWord(claimsContract),
  ));
  let words: Uint8Array[];
  if (action.type === "OPEN_CLAIM_EDITION") {
    words = [keccak_256(new TextEncoder().encode(OPEN_EDITION_TYPE)),
      addressWord(action.shot_registry), hexBytes(action.shot_id),
      uintWord(action.max_claims), uintWord(action.closes_at), addressWord(action.controller),
      uintWord(action.nonce), uintWord(action.deadline)];
  } else {
    words = [keccak_256(new TextEncoder().encode(CLAIM_SOFTWARE_TYPE)),
      addressWord(action.shot_registry), hexBytes(action.shot_id), addressWord(action.claimant),
      hexBytes(action.release_digest), hexBytes(action.checkpoint_digest),
      hexBytes(action.gesture_commitment), uintWord(action.nonce), uintWord(action.deadline)];
  }
  const structHash = keccak_256(concatBytes(...words));
  return `0x${bytesToHex(keccak_256(concatBytes(new Uint8Array([0x19, 0x01]), domain, structHash)))}`;
}

export interface ClaimsRouter {
  handles(pathname: string): boolean;
  fetch(request: Request): Promise<Response>;
  verifyOpenEdition(value: JsonObject, envelope: JsonObject): Promise<void>;
  advanceOpenEdition(value: JsonObject, envelope: JsonObject, transactionHash?: Hex): Promise<{
    transactionHash: Hex;
    confirmed: boolean;
  }>;
  renderReceipt(tokenID: string): Promise<string | undefined>;
  editionForDisplay(shotID: Hex): Promise<ClaimEditionSnapshot | undefined>;
  closureForTimeline(shotID: Hex): Promise<ClaimEditionClosure | undefined>;
}

export type ClaimsPublicationBridge = Pick<ClaimsRouter,
  "verifyOpenEdition" | "advanceOpenEdition" | "editionForDisplay" | "closureForTimeline">;

export interface ClaimEditionClosure {
  reason: "supply_filled" | "time_elapsed";
  occurredAt: string;
  canonicalBlock: { number: string; hash: Hex; transactionIndex: number | null; logIndex: number | null };
}

export interface ClaimCatalogContext {
  shotID: Hex;
  builderID: string;
  releaseDigest: Hex;
  checkpointDigest: Hex;
  checkpointSequence: number;
  appName: string;
  appDescription: string;
  sourceURL: string;
  canonicalBlock: { number: string; hash: Hex };
}

export interface ClaimCatalogResolver {
  currentClaimContext(shotID: Hex, releaseDigest: Hex): Promise<ClaimCatalogContext>;
  claimReceiptContext(shotID: Hex, releaseDigest: Hex): Promise<ClaimCatalogContext>;
}

export interface ClaimsLiveStatus {
  runtimeCodeKeccak256: Hex;
  shotRegistry: Hex;
  relayerAddress?: Hex;
  relayerBalance?: bigint;
}

export interface ClaimEditionSnapshot {
  opened: boolean;
  maxClaims: bigint;
  totalClaims: bigint;
  openedAt: bigint;
  closesAt: bigint;
  closed: boolean;
}

export interface SoftwareClaimSnapshot {
  tokenID: bigint;
  shotID: Hex;
  claimNumber: bigint;
  claimant: Hex;
  releaseDigest: Hex;
  checkpointDigest: Hex;
  gestureCommitment: Hex;
  transactionHash?: Hex;
  blockNumber?: bigint;
  blockHash?: Hex;
  claimedAt?: string;
  transactionIndex?: number;
  logIndex?: number;
}

export interface ClaimsReader {
  liveStatus(): Promise<ClaimsLiveStatus>;
  edition(shotID: Hex): Promise<ClaimEditionSnapshot>;
  tokenFor(shotID: Hex, claimant: Hex): Promise<bigint>;
  claim(tokenID: bigint): Promise<SoftwareClaimSnapshot>;
  claimsForShot(shotID: Hex): Promise<SoftwareClaimSnapshot[]>;
  canonicalBlockAtOrAfter?(timestamp: bigint): Promise<{ number: bigint; hash: Hex; timestamp: string } | undefined>;
}

export interface ClaimsWriter {
  editionNonce(controller: Hex): Promise<bigint>;
  submitOpenEdition(action: Extract<ClaimsAction, { type: "OPEN_CLAIM_EDITION" }>, signature: Hex): Promise<Hex>;
  claimNonce(claimant: Hex): Promise<bigint>;
  accountState(x: Hex, y: Hex): Promise<{ address: Hex; deployed: boolean }>;
  submitAccountBootstrap(x: Hex, y: Hex): Promise<Hex>;
  submitClaim(action: Extract<ClaimsAction, { type: "CLAIM_SOFTWARE" }>, signature: Hex): Promise<Hex>;
  transactionConfirmed(transactionHash: Hex, expectedTarget: Hex): Promise<boolean>;
}

export interface ValidatedClaimMark {
  kind: "drawn" | "accessibility_hold";
  canonicalBytes: Uint8Array;
  gestureCommitment: Hex;
  points: ReadonlyArray<{ x: number; y: number }>;
}

export function validateCanonicalClaimMark(
  canonicalHex: string,
  expectedCommitment?: string,
): ValidatedClaimMark {
  if (!/^0x[0-9a-f]+$/.test(canonicalHex) || canonicalHex.length % 2 !== 0) {
    throw new Error("Claim mark canonical bytes are invalid");
  }
  const bytes = hexBytes(canonicalHex as Hex);
  const expectedLength = CLAIM_MARK_DOMAIN.length + 3 + 64 * 4;
  if (bytes.length !== expectedLength
      || CLAIM_MARK_DOMAIN.some((byte, index) => bytes[index] !== byte)) {
    throw new Error("Claim mark canonical domain or length is invalid");
  }
  const kindByte = bytes[CLAIM_MARK_DOMAIN.length];
  const count = (bytes[CLAIM_MARK_DOMAIN.length + 1]! << 8)
    | bytes[CLAIM_MARK_DOMAIN.length + 2]!;
  if ((kindByte !== 0 && kindByte !== 1) || count !== 64) {
    throw new Error("Claim mark kind or point count is invalid");
  }
  const points: Array<{ x: number; y: number }> = [];
  let offset = CLAIM_MARK_DOMAIN.length + 3;
  for (let index = 0; index < 64; index += 1) {
    const x = (bytes[offset]! << 8) | bytes[offset + 1]!;
    const y = (bytes[offset + 2]! << 8) | bytes[offset + 3]!;
    points.push({ x: x / 65_535, y: y / 65_535 });
    offset += 4;
  }
  const gestureCommitment = sha256Bytes(bytes);
  if (expectedCommitment !== undefined && gestureCommitment !== hex32(expectedCommitment, "gesture commitment")) {
    throw new Error("Claim mark differs from its signed gesture commitment");
  }
  if (kindByte === 1) {
    if (gestureCommitment !== ACCESSIBILITY_MARK_COMMITMENT) {
      throw new Error("accessibility Claim mark is not the one fixed representation");
    }
  } else {
    const arc = points.slice(1).reduce((total, point, index) =>
      total + Math.hypot(point.x - points[index]!.x, point.y - points[index]!.y), 0);
    if (arc < 0.70) throw new Error("Claim mark is too short");
    if (Math.hypot(points[0]!.x - points.at(-1)!.x, points[0]!.y - points.at(-1)!.y) > 0.22) {
      throw new Error("Claim mark is open");
    }
    if (!substantiallyEnclosesCenter(points)) throw new Error("Claim mark does not enclose the app");
  }
  return { kind: kindByte === 0 ? "drawn" : "accessibility_hold",
    canonicalBytes: bytes, gestureCommitment, points };
}

export interface VerifiedActivation {
  signingDigest: Hex;
  claimsContract: Hex;
  shotRegistry: Hex;
  runtimeCodeKeccak256: Hex;
  deploymentBlock: bigint;
  deploymentTransaction: Hex;
  sourceCommit: string;
}

interface ClaimJob {
  schema: "tohseno.software-claim-job/1";
  jobID: string;
  tokenSHA256: Hex;
  context: ClaimCatalogContext;
  claimant: Hex;
  deviceKey: JsonObject;
  canonicalMark: string;
  gestureCommitment: Hex;
  nonce: number;
  deadline: number;
  edition: { maxClaims: number; closesAt: number };
  authorization?: JsonObject;
  status: "prepared" | "authorized" | "account_pending" | "claim_submitted" | "complete" | "failed";
  accountTransactionHash?: Hex;
  claimTransactionHash?: Hex;
  receipt?: JsonObject;
  failure?: string;
  createdAt: string;
  updatedAt: string;
}

interface ClaimsIndexState {
  schema: "tohseno.claims-index/1";
  claimsContract: Hex;
  deploymentBlock: string;
  indexedThrough: { number: string; hash: Hex; timestamp: string };
  claims: SoftwareClaimSnapshot[];
  rebuiltAt: string;
}

export async function createClaimsRouter(
  config: AppConfig,
  injectedReader?: ClaimsReader,
  injectedWriter?: ClaimsWriter,
  catalog?: ClaimCatalogResolver,
): Promise<ClaimsRouter> {
  let activation: VerifiedActivation | undefined;
  let activationFailure: string | undefined;
  if (config.claims.configured) {
    try { activation = await verifyConfiguredActivation(config); }
    catch (error) {
      activationFailure = error instanceof Error ? error.message.slice(0, 240) : "Claims activation rejected";
    }
  }
  const reader = activation
    ? injectedReader ?? new RobinhoodClaimsReader(config, activation)
    : undefined;
  const writer = activation && config.claims.relayerEnabled
    ? injectedWriter ?? new RobinhoodClaimsWriter(config, activation)
    : undefined;
  const claimRoot = config.registry.root ? join(config.registry.root, "claims-v1") : undefined;
  const claimDirectories = claimRoot ? { jobs: join(claimRoot, "jobs"), marks: join(claimRoot, "marks") } : undefined;
  if (claimDirectories) {
    await Promise.all(Object.values(claimDirectories).map((path) => mkdir(path, { recursive: true, mode: 0o700 })));
    for (const path of [claimRoot!, ...Object.values(claimDirectories)]) {
      const metadata = await lstat(path);
      if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
        throw new Error("Claims storage paths must be real private directories");
      }
    }
  }
  let liveCache: { checkedAt: number; value?: ClaimsLiveStatus; failure?: string } | undefined;
  let mutation = Promise.resolve();
  const claimLimiter = new ClaimsRateLimiter();
  const serialized = async <T>(operation: () => Promise<T>): Promise<T> => {
    const prior = mutation;
    let release!: () => void;
    mutation = new Promise<void>((resolve) => { release = resolve; });
    await prior;
    try { return await operation(); } finally { release(); }
  };

  const live = async () => {
    if (!activation || !reader) return { active: false, failure: activationFailure } as const;
    if (liveCache && Date.now() - liveCache.checkedAt < 15_000) {
      return liveCache.value
        ? { active: true, value: liveCache.value } as const
        : { active: false, failure: liveCache.failure } as const;
    }
    try {
      const value = await reader.liveStatus();
      if (value.runtimeCodeKeccak256 !== activation.runtimeCodeKeccak256
          || value.shotRegistry !== activation.shotRegistry) {
        throw new Error("live Claims code or immutable ShotRegistry differs from signed activation");
      }
      liveCache = { checkedAt: Date.now(), value };
      return { active: true, value } as const;
    } catch (error) {
      const failure = error instanceof Error ? error.message.slice(0, 240) : "live Claims verification failed";
      liveCache = { checkedAt: Date.now(), failure };
      return { active: false, failure } as const;
    }
  };

  const requireWriteReady = async (): Promise<{
    activation: VerifiedActivation;
    reader: ClaimsReader;
    writer: ClaimsWriter;
  }> => {
    const observed = await live();
    if (!observed.active || !activation || !reader || !writer
        || !config.claims.indexerEnabled || !config.claims.relayerEnabled) {
      throw new HttpError(503, "Claims write support requires signed activation, live code, canonical indexing, and the constrained relayer");
    }
    if ((observed.value.relayerBalance ?? 0n) <= 0n) {
      throw new HttpError(503, "The constrained Claims relayer is not funded");
    }
    return { activation, reader, writer };
  };

  const verifyOpenEdition = async (value: JsonObject, envelope: JsonObject): Promise<void> => {
    const ready = await requireWriteReady();
    const validated = validateOpenEditionAuthorization(value, envelope, ready.activation);
    const [edition, nonce] = await Promise.all([
      ready.reader.edition(validated.action.shot_id),
      ready.writer.editionNonce(validated.action.controller),
    ]);
    if (edition.opened) throw new HttpError(409, "This Shot already has its immutable Claim Edition");
    if (nonce !== BigInt(validated.action.nonce)) {
      throw new HttpError(409, "Claim Edition nonce moved after Companion approval; review a fresh Ship request");
    }
  };

  const advanceOpenEdition = async (
    value: JsonObject,
    envelope: JsonObject,
    transactionHash?: Hex,
  ): Promise<{ transactionHash: Hex; confirmed: boolean }> => {
    const ready = await requireWriteReady();
    const validated = validateOpenEditionAuthorization(value, envelope, ready.activation, transactionHash !== undefined);
    if (!transactionHash) {
      const next = await ready.writer.submitOpenEdition(validated.action, validated.applicationSignature);
      return { transactionHash: next, confirmed: false };
    }
    if (!await ready.writer.transactionConfirmed(transactionHash, ready.activation.claimsContract)) {
      return { transactionHash, confirmed: false };
    }
    const edition = await ready.reader.edition(validated.action.shot_id);
    if (!edition.opened || edition.maxClaims !== BigInt(validated.action.max_claims)
        || edition.closesAt !== BigInt(validated.action.closes_at)) {
      throw new HttpError(503, "Confirmed Claim Edition transaction does not match canonical contract state");
    }
    return { transactionHash, confirmed: true };
  };

  const requireClaimOrchestration = async () => {
    const ready = await requireWriteReady();
    if (!catalog || !claimDirectories) {
      throw new HttpError(503, "Claim orchestration requires the canonical catalog and durable private job storage");
    }
    return { ...ready, catalog, directories: claimDirectories };
  };

  const prepareSoftwareClaim = async (request: Request, shotID: Hex): Promise<Response> => {
    requireJSON(request);
    const source = claimsSourceKey(request, config);
    if (!claimLimiter.take(source, Math.min(config.registry.sourceRequestsPerMinute, 12))
        || !claimLimiter.take("global", Math.min(config.registry.globalRequestsPerMinute, 300))) {
      throw new HttpError(429, "Too many Claim preparations; wait one minute and try again");
    }
    const ready = await requireClaimOrchestration();
    const jobCount = (await readdir(ready.directories.jobs)).filter((name) => /^[0-9a-f]{32}\.json$/.test(name)).length;
    if (jobCount >= 100_000) throw new HttpError(503, "Claim job capacity is full");
    const body = await boundedJSON(request, 64 * 1024);
    exactKeys(body, ["release_digest", "claimant", "claim_mark", "builder_device"], "Claim preparation");
    const releaseDigest = hex32(body.release_digest, "Claim release digest");
    const claimant = address(body.claimant, "Claimant account");
    const deviceKey = object(body.builder_device, "Claimant DeviceKey");
    const coordinates = validateDeviceKey(deviceKey);
    const account = await ready.writer.accountState(coordinates.x, coordinates.y);
    if (account.address !== claimant) {
      throw new HttpError(422, "Claimant account is not the deterministic account for this DeviceKey");
    }
    const mark = validateCanonicalClaimMark(String(body.claim_mark));
    const context = await ready.catalog.currentClaimContext(shotID, releaseDigest);
    const [edition, priorToken, nonce] = await Promise.all([
      ready.reader.edition(shotID),
      ready.reader.tokenFor(shotID, claimant),
      ready.writer.claimNonce(claimant),
    ]);
    if (!edition.opened || edition.closed) throw new HttpError(409, "This Claim Edition is closed");
    if (priorToken !== 0n) throw new HttpError(409, "This Tohseno address already claimed this Shot");
    const maxClaims = safeChainInteger(edition.maxClaims, "Claim Edition maximum");
    const closesAt = safeChainInteger(edition.closesAt, "Claim Edition close time");
    const nonceValue = safeChainInteger(nonce, "Claim nonce");
    const nowSeconds = Math.floor(Date.now() / 1000);
    const deadline = Math.min(nowSeconds + 20 * 60, closesAt === 0 ? Number.MAX_SAFE_INTEGER : closesAt);
    if (deadline <= nowSeconds) throw new HttpError(409, "This Claim Edition closed before preparation");
    const jobID = randomHex(16).slice(2);
    const token = randomHex(32).slice(2);
    const now = new Date().toISOString();
    const job: ClaimJob = { schema: "tohseno.software-claim-job/1", jobID,
      tokenSHA256: sha256Text(token), context, claimant, deviceKey,
      canonicalMark: String(body.claim_mark), gestureCommitment: mark.gestureCommitment,
      nonce: nonceValue, deadline, edition: { maxClaims, closesAt }, status: "prepared",
      createdAt: now, updatedAt: now };
    await atomicJSON(join(ready.directories.jobs, `${jobID}.json`), job, true);
    return json({ schema: "tohseno.software-claim-preparation/1", job_id: jobID,
      job_token: token, chain_id: 4663, claims_contract: ready.activation.claimsContract,
      claims_activation_signing_digest: ready.activation.signingDigest,
      shot_registry: ready.activation.shotRegistry, shot_id: context.shotID,
      builder_id: context.builderID, release_digest: context.releaseDigest,
      checkpoint_digest: context.checkpointDigest, checkpoint_sequence: context.checkpointSequence,
      claimant, edition: { max_claims: maxClaims, closes_at: closesAt,
        total_claims: safeChainInteger(edition.totalClaims, "Claim count") },
      gesture_commitment: mark.gestureCommitment, nonce: nonceValue, deadline,
      account: { address: account.address, deployed: account.deployed },
      canonical_release_block: context.canonicalBlock }, 201);
  };

  const claimJobResponse = (job: ClaimJob): JsonObject => ({
    schema: "tohseno.software-claim-status/1", job_id: job.jobID, status: job.status,
    shot_id: job.context.shotID, release_digest: job.context.releaseDigest,
    gesture_commitment: job.gestureCommitment,
    transactions: { account: job.accountTransactionHash ?? null, claim: job.claimTransactionHash ?? null },
    claim: job.receipt ?? null, failure: job.failure ?? null, updated_at: job.updatedAt,
  });

  const authorizeClaimJob = async (request: Request, jobID: string): Promise<Response> => {
    const ready = await requireClaimOrchestration();
    requireJSON(request);
    return serialized(async () => {
      const path = claimJobPath(ready.directories.jobs, jobID);
      const job = await readClaimJob(path);
      authorizeClaimJobToken(request, job);
      const body = await boundedJSON(request, 128 * 1024);
      const validated = validateSoftwareClaimAuthorization(body, job, ready.activation);
      if (job.authorization) {
        if (canonicalCatalogJSON(job.authorization) !== canonicalCatalogJSON(body)) {
          throw new HttpError(409, "Claim job already contains another authorization");
        }
      } else {
        job.authorization = body;
        job.status = "authorized";
        job.updatedAt = new Date().toISOString();
        await atomicJSON(path, job, false);
      }
      if (validated.action.deadline <= Math.floor(Date.now() / 1000)) {
        throw new HttpError(409, "Claim authorization expired before relay");
      }
      return json(claimJobResponse(job), 202);
    });
  };

  const advanceClaimJob = async (request: Request, jobID: string): Promise<Response> => {
    const ready = await requireClaimOrchestration();
    return serialized(async () => {
      const path = claimJobPath(ready.directories.jobs, jobID);
      const job = await readClaimJob(path);
      authorizeClaimJobToken(request, job);
      if (!["complete", "failed"].includes(job.status)) {
        try {
          if (!job.authorization) throw new HttpError(409, "Claim job has not been authorized by Companion");
          const validated = validateSoftwareClaimAuthorization(
            job.authorization, job, ready.activation, job.claimTransactionHash !== undefined,
          );
          const priorToken = await ready.reader.tokenFor(job.context.shotID, job.claimant);
          if (priorToken !== 0n) {
            const receipt = await ready.reader.claim(priorToken);
            verifyCanonicalClaim(receipt, validated.action);
            job.receipt = publicClaim(receipt);
            job.status = "complete";
          } else {
            const account = await ready.writer.accountState(validated.x, validated.y);
            if (account.address !== job.claimant) throw new HttpError(422, "Claimant account prediction changed");
            if (!account.deployed) {
              if (!job.accountTransactionHash) {
                job.accountTransactionHash = await ready.writer.submitAccountBootstrap(validated.x, validated.y);
                job.status = "account_pending";
              } else if (!await ready.writer.transactionConfirmed(
                job.accountTransactionHash, config.registry.factoryAddress,
              )) {
                job.status = "account_pending";
              } else {
                const deployed = await ready.writer.accountState(validated.x, validated.y);
                if (!deployed.deployed || deployed.address !== job.claimant) {
                  throw new HttpError(503, "Account bootstrap confirmed without the deterministic account code");
                }
              }
            }
            if ((await ready.writer.accountState(validated.x, validated.y)).deployed) {
              if (!job.claimTransactionHash) {
                const nonce = await ready.writer.claimNonce(job.claimant);
                if (nonce !== BigInt(job.nonce)) {
                  throw new HttpError(409, "Claim nonce moved after Companion authorization");
                }
                job.claimTransactionHash = await ready.writer.submitClaim(
                  validated.action, validated.applicationSignature,
                );
                job.status = "claim_submitted";
              } else if (await ready.writer.transactionConfirmed(
                job.claimTransactionHash, ready.activation.claimsContract,
              )) {
                const tokenID = await ready.reader.tokenFor(job.context.shotID, job.claimant);
                if (tokenID === 0n) throw new HttpError(503, "Confirmed Claim transaction produced no canonical token");
                const receipt = await ready.reader.claim(tokenID);
                verifyCanonicalClaim(receipt, validated.action);
                job.receipt = publicClaim(receipt);
                job.status = "complete";
              } else {
                job.status = "claim_submitted";
              }
            }
          }
          if (job.status === "complete") {
            await atomicText(join(ready.directories.marks, `${job.gestureCommitment.slice(2)}.hex`),
              `${job.canonicalMark}\n`, true).catch(async (error) => {
                const existing = await readFile(join(ready.directories.marks, `${job.gestureCommitment.slice(2)}.hex`), "utf8");
                if (existing !== `${job.canonicalMark}\n`) throw error;
              });
          }
        } catch (error) {
          if (error instanceof HttpError && error.status === 503) throw error;
          job.status = "failed";
          job.failure = error instanceof Error ? error.message.slice(0, 300) : "Claim failed";
        }
        job.updatedAt = new Date().toISOString();
        await atomicJSON(path, job, false);
      }
      return json(claimJobResponse(job), job.status === "complete" ? 200 : job.status === "failed" ? 422 : 202);
    });
  };

  const loadReceiptMark = async (commitment: Hex): Promise<ValidatedClaimMark | undefined> => {
    if (!claimDirectories) return undefined;
    try {
      const canonicalHex = (await readFile(join(claimDirectories.marks, `${commitment.slice(2)}.hex`), "utf8")).trim();
      return validateCanonicalClaimMark(canonicalHex, commitment);
    } catch { return undefined; }
  };

  const renderReceipt = async (token: string): Promise<string | undefined> => {
    const observed = await live();
    if (!observed.active || !reader || !catalog || !activation || !/^\d+$/.test(token)) return undefined;
    try {
      const receipt = await reader.claim(BigInt(token));
      const [context, edition, mark] = await Promise.all([
        catalog.claimReceiptContext(receipt.shotID, receipt.releaseDigest),
        reader.edition(receipt.shotID),
        loadReceiptMark(receipt.gestureCommitment),
      ]);
      return claimReceiptHTML(context, edition, receipt, mark);
    } catch { return undefined; }
  };

  async function fetchRoute(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const method = request.method.toUpperCase();
    const parts = url.pathname.split("/").filter(Boolean);
    if (url.pathname === "/api/registry/v1/claims/status" && (method === "GET" || method === "HEAD")) {
      const observed = await live();
      const body = {
        schema: "tohseno.claims-status/1",
        configured: config.claims.configured,
        activation_verified: activation !== undefined,
        contract_code_verified: observed.active,
        indexer_enabled: observed.active && config.claims.indexerEnabled,
        relayer: {
          enabled: observed.active && config.claims.relayerEnabled,
          funded: observed.active && (observed.value.relayerBalance ?? 0n) > 0n,
          address: observed.active ? observed.value.relayerAddress ?? null : null,
        },
        chain_id: 4663,
        claims_contract: activation?.claimsContract ?? null,
        shot_registry: activation?.shotRegistry ?? null,
        activation_signing_digest: activation?.signingDigest ?? null,
        failure: observed.active ? null : observed.failure ?? "Claims are not activated",
      };
      return head(json(body, observed.active ? 200 : 503), method);
    }
    const observed = await live();
    if (!observed.active || !activation || !reader) {
      throw new HttpError(503, "Claims are unavailable until signed activation and live code agree");
    }
    if (parts.length === 7 && parts.slice(0, 4).join("/") === "api/registry/v1/shots"
        && parts[5] === "claims" && parts[6] === "prepare" && method === "POST") {
      return prepareSoftwareClaim(request, hex32(parts[4], "ShotID"));
    }
    if (parts.length === 7 && parts.slice(0, 5).join("/") === "api/registry/v1/claims/jobs"
        && parts[6] === "submit" && method === "POST") {
      return authorizeClaimJob(request, parts[5]!);
    }
    if (parts.length === 6 && parts.slice(0, 5).join("/") === "api/registry/v1/claims/jobs"
        && (method === "GET" || method === "POST")) {
      return advanceClaimJob(request, parts[5]!);
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/shots"
        && parts[5] === "claim-edition" && (method === "GET" || method === "HEAD")) {
      const shotID = hex32(parts[4], "ShotID");
      const edition = await reader.edition(shotID);
      return head(json(publicEdition(shotID, edition)), method);
    }
    if (parts.length === 6 && parts.slice(0, 4).join("/") === "api/registry/v1/shots"
        && parts[5] === "claims" && (method === "GET" || method === "HEAD")) {
      const shotID = hex32(parts[4], "ShotID");
      const claims = (await reader.claimsForShot(shotID)).sort((left, right) =>
        left.claimNumber < right.claimNumber ? -1 : left.claimNumber > right.claimNumber ? 1 : 0);
      const cursor = url.searchParams.get("cursor");
      const after = cursor === null ? 0n : positiveBigInt(cursor, "claim cursor");
      const limit = boundedLimit(url.searchParams.get("limit"));
      const page = claims.filter((claim) => claim.claimNumber > after).slice(0, limit);
      return head(json({ schema: "tohseno.software-claim-page/1", shot_id: shotID,
        claims: page.map(publicClaim), next_cursor: page.length === limit
          && claims.some((claim) => claim.claimNumber > page.at(-1)!.claimNumber)
          ? page.at(-1)!.claimNumber.toString() : null }), method);
    }
    if (parts.length === 7 && parts.slice(0, 4).join("/") === "api/registry/v1/shots"
        && parts[5] === "claims" && (method === "GET" || method === "HEAD")) {
      const shotID = hex32(parts[4], "ShotID");
      const claimant = accountAddress(parts[6]);
      const tokenID = await reader.tokenFor(shotID, claimant);
      if (tokenID === 0n) return head(json({ schema: "tohseno.software-claim-state/1",
        shot_id: shotID, claimant, claimed: false, claim: null }), method);
      const claim = await reader.claim(tokenID);
      if (claim.shotID !== shotID || claim.claimant !== claimant) {
        throw new HttpError(503, "canonical Claim state is internally inconsistent");
      }
      return head(json({ schema: "tohseno.software-claim-state/1",
        shot_id: shotID, claimant, claimed: true, claim: publicClaim(claim) }), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/registry/v1/claims"
        && (method === "GET" || method === "HEAD")) {
      const tokenID = positiveBigInt(parts[4], "Claim token ID");
      return head(json({ schema: "tohseno.software-claim-receipt/1",
        claim: publicClaim(await reader.claim(tokenID)), transferable: false }), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/claims/v1/token"
        && (method === "GET" || method === "HEAD")) {
      const tokenID = positiveBigInt(parts[4], "Claim token ID");
      const claim = await reader.claim(tokenID);
      if (!catalog) throw new HttpError(503, "Claim metadata requires the canonical release catalog");
      const [context, edition] = await Promise.all([
        catalog.claimReceiptContext(claim.shotID, claim.releaseDigest), reader.edition(claim.shotID),
      ]);
      return head(json(tokenMetadata(claim, context, edition)), method);
    }
    if (parts.length === 5 && parts.slice(0, 4).join("/") === "api/claims/v1/mark"
        && (method === "GET" || method === "HEAD")) {
      const match = parts[4]!.match(/^(0x[0-9a-f]{64})\.svg$/);
      if (!match) throw new HttpError(404, "Claim mark not found");
      const mark = await loadReceiptMark(match[1] as Hex);
      if (!mark) throw new HttpError(404, "Claim mark not found");
      const response = withSecurityHeaders(new Response(claimMarkSVG(mark), { headers: {
        "content-type": "image/svg+xml; charset=utf-8", "cache-control": "public, max-age=31536000, immutable",
      } }));
      return head(response, method);
    }
    if (!["GET", "HEAD"].includes(method)) return methodNotAllowed();
    throw new HttpError(404, "Not found");
  }

  return {
    handles: (pathname) => pathname.startsWith("/api/registry/v1/claims")
      || /^\/api\/registry\/v1\/shots\/[^/]+\/claim/.test(pathname)
      || pathname.startsWith("/api/claims/v1/"),
    fetch: fetchRoute,
    verifyOpenEdition,
    advanceOpenEdition,
    renderReceipt,
    editionForDisplay: async (shotID) => {
      const observed = await live();
      return observed.active && reader ? reader.edition(shotID) : undefined;
    },
    closureForTimeline: async (shotID) => {
      const observed = await live();
      if (!observed.active || !reader) return undefined;
      const edition = await reader.edition(shotID);
      if (!edition.opened || !edition.closed) return undefined;
      if (edition.maxClaims > 0n && edition.totalClaims >= edition.maxClaims) {
        const closingClaim = (await reader.claimsForShot(shotID))
          .find((claim) => claim.claimNumber === edition.maxClaims);
        if (!closingClaim?.blockNumber || !closingClaim.blockHash || !closingClaim.claimedAt) return undefined;
        return { reason: "supply_filled", occurredAt: exactSecond(closingClaim.claimedAt),
          canonicalBlock: { number: closingClaim.blockNumber.toString(), hash: closingClaim.blockHash,
            transactionIndex: closingClaim.transactionIndex ?? null, logIndex: closingClaim.logIndex ?? null } };
      }
      if (edition.closesAt > 0n && reader.canonicalBlockAtOrAfter) {
        const block = await reader.canonicalBlockAtOrAfter(edition.closesAt);
        if (block) return { reason: "time_elapsed", occurredAt: exactSecond(block.timestamp),
          canonicalBlock: { number: block.number.toString(), hash: block.hash,
            transactionIndex: null, logIndex: null } };
      }
      return undefined;
    },
  };
}

export class RobinhoodClaimsReader implements ClaimsReader {
  private readonly client: PublicClient;
  private readonly address: Hex;
  private readonly deploymentBlock: bigint;
  private readonly relayerAddress?: Hex;
  private readonly indexPath?: string;
  private indexing?: Promise<SoftwareClaimSnapshot[]>;

  constructor(config: AppConfig, activation: VerifiedActivation, injectedClient?: PublicClient) {
    if (!config.registry.rpcUrl) throw new Error("Claims read support requires Robinhood RPC");
    const chain = defineChain({ id: 4663, name: "Robinhood Chain", nativeCurrency: {
      name: "Robinhood Token", symbol: "ETH", decimals: 18,
    }, rpcUrls: { default: { http: [config.registry.rpcUrl] } } });
    this.client = injectedClient ?? createPublicClient({ chain, transport: http(config.registry.rpcUrl) });
    this.address = activation.claimsContract;
    this.deploymentBlock = activation.deploymentBlock;
    this.relayerAddress = config.claims.relayerPrivateKey
      ? privateKeyToAccount(config.claims.relayerPrivateKey).address.toLowerCase() as Hex
      : undefined;
    this.indexPath = config.claims.indexerEnabled && config.registry.root
      ? join(config.registry.root, "claims-v1", "index.json") : undefined;
  }

  async liveStatus(): Promise<ClaimsLiveStatus> {
    const bytecode = await this.client.getCode({ address: this.address });
    if (!bytecode || bytecode === "0x") throw new Error("signed Claims address has no live code");
    const shotRegistry = await this.client.readContract({ address: this.address,
      abi: CLAIMS_ABI, functionName: "shotRegistry" }) as Hex;
    const relayerBalance = this.relayerAddress
      ? await this.client.getBalance({ address: this.relayerAddress }) : undefined;
    return { runtimeCodeKeccak256: `0x${bytesToHex(keccak_256(hexBytes(bytecode)))}`,
      shotRegistry: shotRegistry.toLowerCase() as Hex,
      ...(this.relayerAddress ? { relayerAddress: this.relayerAddress, relayerBalance } : {}) };
  }

  async edition(shotID: Hex): Promise<ClaimEditionSnapshot> {
    const value = await this.client.readContract({ address: this.address, abi: CLAIMS_ABI,
      functionName: "claimEdition", args: [shotID] }) as {
        opened: boolean; maxClaims: bigint; totalClaims: bigint; openedAt: bigint; closesAt: bigint;
      };
    const closed = await this.client.readContract({ address: this.address, abi: CLAIMS_ABI,
      functionName: "editionIsClosed", args: [shotID] }) as boolean;
    return { opened: value.opened, maxClaims: value.maxClaims, totalClaims: value.totalClaims,
      openedAt: value.openedAt, closesAt: value.closesAt, closed };
  }

  async tokenFor(shotID: Hex, claimant: Hex): Promise<bigint> {
    return await this.client.readContract({ address: this.address, abi: CLAIMS_ABI,
      functionName: "claimTokenOf", args: [shotID, claimant] }) as bigint;
  }

  async claim(tokenID: bigint): Promise<SoftwareClaimSnapshot> {
    const value = await this.client.readContract({ address: this.address, abi: CLAIMS_ABI,
      functionName: "claimRecord", args: [tokenID] }) as {
        shotId: Hex; claimNumber: bigint; claimant: Hex; releaseDigest: Hex;
        checkpointDigest: Hex; gestureCommitment: Hex;
      };
    const receipt: SoftwareClaimSnapshot = { tokenID, shotID: value.shotId.toLowerCase() as Hex, claimNumber: value.claimNumber,
      claimant: value.claimant.toLowerCase() as Hex,
      releaseDigest: value.releaseDigest.toLowerCase() as Hex,
      checkpointDigest: value.checkpointDigest.toLowerCase() as Hex,
      gestureCommitment: value.gestureCommitment.toLowerCase() as Hex };
    const indexed = (await this.syncIndex()).find((claim) => claim.tokenID === tokenID);
    if (indexed && indexed.shotID === receipt.shotID && indexed.claimant === receipt.claimant
        && indexed.releaseDigest === receipt.releaseDigest
        && indexed.checkpointDigest === receipt.checkpointDigest
        && indexed.gestureCommitment === receipt.gestureCommitment) return indexed;
    throw new Error("canonical SoftwareClaimed event is missing for this token");
  }

  async claimsForShot(shotID: Hex): Promise<SoftwareClaimSnapshot[]> {
    return (await this.syncIndex()).filter((claim) => claim.shotID === shotID);
  }

  async canonicalBlockAtOrAfter(
    timestamp: bigint,
  ): Promise<{ number: bigint; hash: Hex; timestamp: string } | undefined> {
    const head = await this.client.getBlock({ blockTag: "latest" });
    if (head.number === null || head.timestamp < timestamp) return undefined;
    let lower = this.deploymentBlock;
    let upper = head.number;
    while (lower < upper) {
      const middle = lower + (upper - lower) / 2n;
      const block = await this.client.getBlock({ blockNumber: middle });
      if (block.timestamp >= timestamp) upper = middle;
      else lower = middle + 1n;
    }
    const block = await this.client.getBlock({ blockNumber: lower });
    if (block.hash === null || block.timestamp < timestamp) return undefined;
    const observed = await this.client.getBlock({ blockNumber: lower });
    if (observed.hash !== block.hash) throw new Error("Robinhood reorganized while deriving edition closure");
    return { number: lower, hash: block.hash,
      timestamp: new Date(Number(block.timestamp) * 1000).toISOString() };
  }

  private async syncIndex(): Promise<SoftwareClaimSnapshot[]> {
    if (this.indexing) return this.indexing;
    this.indexing = this.rebuildIndex();
    try { return await this.indexing; } finally { this.indexing = undefined; }
  }

  private async rebuildIndex(): Promise<SoftwareClaimSnapshot[]> {
    const head = await this.client.getBlock({ blockTag: "latest" });
    if (head.number === null) throw new Error("Robinhood latest block has no number");
    const logs = await this.client.getLogs({ address: this.address,
      fromBlock: this.deploymentBlock, toBlock: head.number });
    const sameHead = await this.client.getBlock({ blockNumber: head.number });
    if (sameHead.hash !== head.hash) throw new Error("Robinhood reorganized while Claims were indexing; retry");
    const timestamps = new Map<bigint, string>();
    const claims: SoftwareClaimSnapshot[] = [];
    for (const log of logs) {
      try {
        const decoded = decodeEventLog({ abi: CLAIMS_ABI, data: log.data, topics: log.topics });
        if (decoded.eventName !== "SoftwareClaimed") continue;
        const args = decoded.args as unknown as { shotId: Hex; claimant: Hex; tokenId: bigint;
          claimNumber: bigint; releaseDigest: Hex; checkpointDigest: Hex; gestureCommitment: Hex };
        let timestamp = timestamps.get(log.blockNumber);
        if (!timestamp) {
          const block = await this.client.getBlock({ blockNumber: log.blockNumber });
          if (block.hash !== log.blockHash) throw new Error("Claims log block is no longer canonical");
          timestamp = new Date(Number(block.timestamp) * 1000).toISOString();
          timestamps.set(log.blockNumber, timestamp);
        }
        claims.push({ tokenID: args.tokenId, shotID: args.shotId.toLowerCase() as Hex,
          claimNumber: args.claimNumber, claimant: args.claimant.toLowerCase() as Hex,
          releaseDigest: args.releaseDigest.toLowerCase() as Hex,
          checkpointDigest: args.checkpointDigest.toLowerCase() as Hex,
          gestureCommitment: args.gestureCommitment.toLowerCase() as Hex,
          transactionHash: log.transactionHash, blockNumber: log.blockNumber, blockHash: log.blockHash,
          claimedAt: timestamp, transactionIndex: log.transactionIndex, logIndex: log.logIndex });
      } catch (error) {
        if (error instanceof Error && /no longer canonical/.test(error.message)) throw error;
      }
    }
    claims.sort((left, right) => left.blockNumber! < right.blockNumber! ? -1
      : left.blockNumber! > right.blockNumber! ? 1
      : (left.transactionIndex ?? 0) - (right.transactionIndex ?? 0)
        || (left.logIndex ?? 0) - (right.logIndex ?? 0));
    if (this.indexPath) {
      const state: ClaimsIndexState = { schema: "tohseno.claims-index/1", claimsContract: this.address,
        deploymentBlock: this.deploymentBlock.toString(), indexedThrough: { number: head.number.toString(),
          hash: head.hash, timestamp: new Date(Number(head.timestamp) * 1000).toISOString() },
        claims, rebuiltAt: new Date().toISOString() };
      const exists = await readFile(this.indexPath, "utf8").then(() => true).catch(() => false);
      await atomicJSON(this.indexPath, state, !exists);
    }
    return claims;
  }
}

class RobinhoodClaimsWriter implements ClaimsWriter {
  private readonly publicClient: PublicClient;
  private readonly wallet: ReturnType<typeof createWalletClient>;
  private readonly address: Hex;
  private readonly factory: Hex;

  constructor(config: AppConfig, activation: VerifiedActivation) {
    if (!config.registry.rpcUrl || !config.claims.relayerPrivateKey) {
      throw new Error("Claims write support requires one dedicated relayer and Robinhood RPC");
    }
    const chain = defineChain({ id: 4663, name: "Robinhood Chain", nativeCurrency: {
      name: "Robinhood Token", symbol: "ETH", decimals: 18,
    }, rpcUrls: { default: { http: [config.registry.rpcUrl] } } });
    const transport = http(config.registry.rpcUrl, { timeout: 10_000 });
    this.publicClient = createPublicClient({ chain, transport });
    this.wallet = createWalletClient({ account: privateKeyToAccount(config.claims.relayerPrivateKey),
      chain, transport });
    this.address = activation.claimsContract;
    this.factory = config.registry.factoryAddress;
  }

  async editionNonce(controller: Hex): Promise<bigint> {
    return await this.publicClient.readContract({ address: this.address, abi: CLAIMS_ABI,
      functionName: "editionNonces", args: [controller] }) as bigint;
  }

  async claimNonce(claimant: Hex): Promise<bigint> {
    return await this.publicClient.readContract({ address: this.address, abi: CLAIMS_ABI,
      functionName: "claimNonces", args: [claimant] }) as bigint;
  }

  async accountState(x: Hex, y: Hex): Promise<{ address: Hex; deployed: boolean }> {
    const salt = builderAccountSalt(x, y);
    const predicted = await this.publicClient.readContract({ address: this.factory, abi: FACTORY_ABI,
      functionName: "predictAccount", args: [salt, BigInt(x), BigInt(y)] }) as Hex;
    const address = predicted.toLowerCase() as Hex;
    const code = await this.publicClient.getCode({ address });
    return { address, deployed: code !== undefined && code !== "0x" };
  }

  async submitAccountBootstrap(x: Hex, y: Hex): Promise<Hex> {
    return await this.wallet.writeContract({ account: this.wallet.account!, chain: this.wallet.chain,
      address: this.factory, abi: FACTORY_ABI, functionName: "createAccount",
      args: [builderAccountSalt(x, y), BigInt(x), BigInt(y)] });
  }

  async submitOpenEdition(
    action: Extract<ClaimsAction, { type: "OPEN_CLAIM_EDITION" }>,
    signature: Hex,
  ): Promise<Hex> {
    return await this.wallet.writeContract({ account: this.wallet.account!, chain: this.wallet.chain,
      address: this.address, abi: CLAIMS_ABI,
      functionName: "openClaimEdition", args: [{ shotRegistry: action.shot_registry,
        shotId: action.shot_id, maxClaims: BigInt(action.max_claims),
        closesAt: BigInt(action.closes_at), controller: action.controller,
        nonce: BigInt(action.nonce), deadline: BigInt(action.deadline) }, signature] });
  }

  async submitClaim(
    action: Extract<ClaimsAction, { type: "CLAIM_SOFTWARE" }>,
    signature: Hex,
  ): Promise<Hex> {
    return await this.wallet.writeContract({ account: this.wallet.account!, chain: this.wallet.chain,
      address: this.address, abi: CLAIMS_ABI, functionName: "claimSoftware", args: [{
        shotRegistry: action.shot_registry, shotId: action.shot_id, claimant: action.claimant,
        releaseDigest: action.release_digest, checkpointDigest: action.checkpoint_digest,
        gestureCommitment: action.gesture_commitment, nonce: BigInt(action.nonce),
        deadline: BigInt(action.deadline),
      }, signature] });
  }

  async transactionConfirmed(transactionHash: Hex, expectedTarget: Hex): Promise<boolean> {
    try {
      const receipt = await this.publicClient.getTransactionReceipt({ hash: transactionHash });
      if (receipt.status !== "success" || receipt.to?.toLowerCase() !== expectedTarget) {
        throw new HttpError(422, "The constrained Claim Edition transaction reverted or targeted another contract");
      }
      const block = await this.publicClient.getBlock({ blockNumber: receipt.blockNumber });
      if (block.hash !== receipt.blockHash) {
        throw new HttpError(503, "Claim Edition receipt is no longer in the canonical chain");
      }
      return true;
    } catch (error) {
      if (error instanceof HttpError) throw error;
      const name = error instanceof Error ? error.name : "";
      const message = error instanceof Error ? error.message : "";
      if (/not.?found|could not be found|not found/i.test(`${name} ${message}`)) return false;
      throw error;
    }
  }
}

function validateOpenEditionAuthorization(
  value: JsonObject,
  envelope: JsonObject,
  activation: VerifiedActivation,
  allowExpired = false,
): { action: Extract<ClaimsAction, { type: "OPEN_CLAIM_EDITION" }>; applicationSignature: Hex } {
  exactKeys(value, ["policy", "action", "digest", "signature"], "Claim Edition approval");
  const policy = object(value.policy, "Claim Edition policy");
  exactKeys(policy, ["kind", "max_claims", "closes_at"], "Claim Edition policy");
  const maxClaims = safeInteger(policy.max_claims, "Claim Edition maximum");
  const closesAt = safeInteger(policy.closes_at, "Claim Edition close time");
  const kind = String(policy.kind);
  if (!((kind === "open" && maxClaims === 0 && closesAt === 0)
      || (kind === "limited" && maxClaims > 0 && closesAt === 0)
      || (kind === "timed" && maxClaims === 0 && closesAt > 0)
      || (kind === "limited_timed" && maxClaims > 0 && closesAt > 0))) {
    throw new HttpError(422, "Claim Edition policy shape is invalid");
  }
  const rawAction = object(value.action, "OpenClaimEdition action");
  exactKeys(rawAction, ["shot_registry", "shot_id", "max_claims", "closes_at", "controller", "nonce", "deadline"], "OpenClaimEdition action");
  const action: Extract<ClaimsAction, { type: "OPEN_CLAIM_EDITION" }> = {
    type: "OPEN_CLAIM_EDITION",
    shot_registry: address(rawAction.shot_registry, "Claim Edition Registry"),
    shot_id: hex32(rawAction.shot_id, "Claim Edition ShotID"),
    max_claims: safeInteger(rawAction.max_claims, "Claim Edition maximum"),
    closes_at: safeInteger(rawAction.closes_at, "Claim Edition close time"),
    controller: address(rawAction.controller, "Claim Edition controller"),
    nonce: safeInteger(rawAction.nonce, "Claim Edition nonce"),
    deadline: safeInteger(rawAction.deadline, "Claim Edition deadline"),
  };
  const release = object(envelope.release, "catalog release");
  const signer = object(envelope.signer, "catalog signer");
  exactKeys(signer, ["x", "y"], "catalog signer");
  const builder = String(release.builder_id).match(BUILDER_ID)?.[1];
  const now = Math.floor(Date.now() / 1000);
  if (release.checkpoint_sequence !== 1 || action.shot_id !== release.shot_id
      || action.controller !== builder || action.shot_registry !== activation.shotRegistry
      || action.max_claims !== maxClaims || action.closes_at !== closesAt
      || (!allowExpired && action.deadline <= now) || action.deadline > now + 24 * 60 * 60
      || (!allowExpired && action.closes_at !== 0 && action.closes_at <= now)) {
    throw new HttpError(422, "Claim Edition approval differs from this exact first Ship");
  }
  const digest = hex32(value.digest, "Claim Edition digest");
  if (digest !== canonicalClaimsActionDigest(action, activation.claimsContract)) {
    throw new HttpError(422, "Claim Edition digest differs from its exact EIP-712 action");
  }
  const signature = object(value.signature, "Claim Edition signature");
  exactKeys(signature, ["schema", "signer", "algorithm", "digest", "r", "s", "low_s"], "Claim Edition signature");
  const signatureSigner = object(signature.signer, "Claim Edition signer");
  exactKeys(signatureSigner, ["schema", "key_id", "x", "y", "security_level", "test_only"], "Claim Edition signer");
  const x = hex32(signatureSigner.x, "Claim Edition signer x");
  const y = hex32(signatureSigner.y, "Claim Edition signer y");
  const r = hex32(signature.r, "Claim Edition signature r");
  const s = hex32(signature.s, "Claim Edition signature s");
  if (signature.schema !== "tohseno.builder-device-signature/1" || signature.algorithm !== "p256"
      || signature.digest !== digest || signature.low_s !== true || BigInt(s) > HALF_ORDER
      || signer.x !== x || signer.y !== y) {
    throw new HttpError(422, "Claim Edition signature metadata is invalid");
  }
  const compact = concatBytes(hexBytes(r), hexBytes(s));
  const publicKey = concatBytes(new Uint8Array([4]), hexBytes(x), hexBytes(y));
  if (!p256.verify(compact, hexBytes(digest), publicKey, { prehash: false, lowS: true })) {
    throw new HttpError(403, "Claim Edition DeviceKey signature is invalid");
  }
  const applicationSignature = `0x01${x.slice(2)}${y.slice(2)}${r.slice(2)}${s.slice(2)}` as Hex;
  return { action, applicationSignature };
}

function validateDeviceKey(value: JsonObject): { x: Hex; y: Hex } {
  exactKeys(value, ["schema", "key_id", "x", "y", "security_level", "test_only"], "Claimant DeviceKey");
  const x = hex32(value.x, "Claimant DeviceKey x");
  const y = hex32(value.y, "Claimant DeviceKey y");
  const keyID = hex32(value.key_id, "Claimant DeviceKey ID");
  const expected = `0x${bytesToHex(keccak_256(concatBytes(hexBytes(x), hexBytes(y))))}` as Hex;
  if (value.schema !== "tohseno.builder-device-announcement/1" || keyID !== expected
      || !["secure_enclave", "software_test"].includes(String(value.security_level))
      || value.test_only !== (value.security_level === "software_test")) {
    throw new HttpError(422, "Claimant DeviceKey announcement is invalid");
  }
  return { x, y };
}

function validateSoftwareClaimAuthorization(
  value: JsonObject,
  job: ClaimJob,
  activation: VerifiedActivation,
  allowExpired = false,
): { action: Extract<ClaimsAction, { type: "CLAIM_SOFTWARE" }>; applicationSignature: Hex; x: Hex; y: Hex } {
  exactKeys(value, ["action", "digest", "signature"], "Software Claim authorization");
  const raw = object(value.action, "ClaimSoftware action");
  exactKeys(raw, ["shot_registry", "shot_id", "claimant", "release_digest", "checkpoint_digest",
    "gesture_commitment", "nonce", "deadline"], "ClaimSoftware action");
  const action: Extract<ClaimsAction, { type: "CLAIM_SOFTWARE" }> = {
    type: "CLAIM_SOFTWARE", shot_registry: address(raw.shot_registry, "Claim Registry"),
    shot_id: hex32(raw.shot_id, "Claim ShotID"), claimant: address(raw.claimant, "Claimant"),
    release_digest: hex32(raw.release_digest, "Claim release"),
    checkpoint_digest: hex32(raw.checkpoint_digest, "Claim checkpoint"),
    gesture_commitment: hex32(raw.gesture_commitment, "Claim mark commitment"),
    nonce: safeInteger(raw.nonce, "Claim nonce"), deadline: safeInteger(raw.deadline, "Claim deadline"),
  };
  const now = Math.floor(Date.now() / 1000);
  if (action.shot_registry !== activation.shotRegistry || action.shot_id !== job.context.shotID
      || action.claimant !== job.claimant || action.release_digest !== job.context.releaseDigest
      || action.checkpoint_digest !== job.context.checkpointDigest
      || action.gesture_commitment !== job.gestureCommitment || action.nonce !== job.nonce
      || action.deadline !== job.deadline || (!allowExpired && action.deadline <= now)) {
    throw new HttpError(422, "Software Claim authorization differs from the exact prepared encounter");
  }
  const digest = hex32(value.digest, "Software Claim digest");
  if (digest !== canonicalClaimsActionDigest(action, activation.claimsContract)) {
    throw new HttpError(422, "Software Claim digest differs from its exact EIP-712 action");
  }
  const signature = object(value.signature, "Software Claim signature");
  exactKeys(signature, ["schema", "signer", "algorithm", "digest", "r", "s", "low_s"], "Software Claim signature");
  const signer = object(signature.signer, "Software Claim signer");
  const coordinates = validateDeviceKey(signer);
  if (canonicalCatalogJSON(signer) !== canonicalCatalogJSON(job.deviceKey)) {
    throw new HttpError(422, "Software Claim signature uses another DeviceKey");
  }
  const r = hex32(signature.r, "Software Claim signature r");
  const s = hex32(signature.s, "Software Claim signature s");
  if (signature.schema !== "tohseno.builder-device-signature/1" || signature.algorithm !== "p256"
      || signature.digest !== digest || signature.low_s !== true || BigInt(s) > HALF_ORDER) {
    throw new HttpError(422, "Software Claim signature metadata is invalid");
  }
  const compact = concatBytes(hexBytes(r), hexBytes(s));
  const publicKey = concatBytes(new Uint8Array([4]), hexBytes(coordinates.x), hexBytes(coordinates.y));
  if (!p256.verify(compact, hexBytes(digest), publicKey, { prehash: false, lowS: true })) {
    throw new HttpError(403, "Software Claim DeviceKey signature is invalid");
  }
  return { action, x: coordinates.x, y: coordinates.y,
    applicationSignature: `0x01${coordinates.x.slice(2)}${coordinates.y.slice(2)}${r.slice(2)}${s.slice(2)}` as Hex };
}

function verifyCanonicalClaim(
  receipt: SoftwareClaimSnapshot,
  action: Extract<ClaimsAction, { type: "CLAIM_SOFTWARE" }>,
): void {
  if (receipt.shotID !== action.shot_id || receipt.claimant !== action.claimant
      || receipt.releaseDigest !== action.release_digest
      || receipt.checkpointDigest !== action.checkpoint_digest
      || receipt.gestureCommitment !== action.gesture_commitment || receipt.tokenID <= 0n
      || receipt.claimNumber <= 0n || !receipt.transactionHash || receipt.blockNumber === undefined
      || !receipt.blockHash) {
    throw new HttpError(503, "Canonical Claim receipt differs from its authorized encounter");
  }
}

function safeChainInteger(value: bigint, name: string): number {
  if (value < 0n || value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new HttpError(503, `${name} exceeds the exact service integer bound`);
  }
  return Number(value);
}

function randomHex(bytes: number): Hex {
  const value = new Uint8Array(bytes);
  crypto.getRandomValues(value);
  return `0x${bytesToHex(value)}`;
}

function claimJobPath(root: string, value: string): string {
  if (!/^[0-9a-f]{32}$/.test(value)) throw new HttpError(404, "Claim job not found");
  return join(root, `${value}.json`);
}

async function readClaimJob(path: string): Promise<ClaimJob> {
  let raw: string;
  try { raw = await readFile(path, "utf8"); } catch { throw new HttpError(404, "Claim job not found"); }
  if (raw.length > 512 * 1024) throw new HttpError(503, "Claim job exceeds its durable bound");
  const value = JSON.parse(raw) as ClaimJob;
  if (value.schema !== "tohseno.software-claim-job/1") throw new HttpError(503, "Claim job schema is invalid");
  return value;
}

function authorizeClaimJobToken(request: Request, job: ClaimJob): void {
  const token = request.headers.get("authorization")?.replace(/^Bearer /, "") ?? "";
  const observed = sha256Text(token);
  let difference = observed.length ^ job.tokenSHA256.length;
  for (let index = 0; index < Math.max(observed.length, job.tokenSHA256.length); index += 1) {
    difference |= (observed.charCodeAt(index) || 0) ^ (job.tokenSHA256.charCodeAt(index) || 0);
  }
  if (!token || difference !== 0) throw new HttpError(401, "Invalid Claim job authorization");
}

async function atomicJSON(path: string, value: unknown, create: boolean): Promise<void> {
  await atomicText(path, `${JSON.stringify(value, (_key, item) => typeof item === "bigint" ? item.toString() : item)}\n`, create);
}

async function atomicText(path: string, value: string, create: boolean): Promise<void> {
  await mkdir(dirname(path), { recursive: true, mode: 0o700 });
  if (create) {
    const handle = await open(path, "wx", 0o600);
    try { await handle.writeFile(value); await handle.sync(); } finally { await handle.close(); }
    return;
  }
  const temporary = `${path}.${randomHex(8).slice(2)}.tmp`;
  const handle = await open(temporary, "wx", 0o600);
  try { await handle.writeFile(value); await handle.sync(); } finally { await handle.close(); }
  await rename(temporary, path);
}

function requireJSON(request: Request): void {
  if (request.headers.get("content-type")?.split(";", 1)[0] !== "application/json") {
    throw new HttpError(415, "Content-Type must be application/json");
  }
}

async function boundedJSON(request: Request, maximum: number): Promise<JsonObject> {
  const length = request.headers.get("content-length");
  if (length !== null && (!/^\d+$/.test(length) || Number(length) > maximum)) {
    throw new HttpError(413, "Request is too large");
  }
  const text = await request.text();
  if (!text || new TextEncoder().encode(text).length > maximum) throw new HttpError(413, "Request is empty or too large");
  try { rejectDuplicateJSONMembers(text); return object(JSON.parse(text), "request"); }
  catch (error) { if (error instanceof HttpError) throw error; throw new HttpError(400, "Request body is invalid JSON"); }
}

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

class ClaimsRateLimiter {
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

function claimsSourceKey(request: Request, config: AppConfig): string {
  if (!config.trustProxy) return "direct";
  const value = request.headers.get("cf-connecting-ip")
    ?? request.headers.get("x-forwarded-for")?.split(",", 1)[0]?.trim()
    ?? "unknown";
  return /^[0-9a-f:.]{2,64}$/i.test(value) ? `source:${value}` : "source:invalid";
}

async function verifyConfiguredActivation(config: AppConfig): Promise<VerifiedActivation> {
  const claims = config.claims;
  if (!claims.contractAddress || !claims.activationSigningDigest || !claims.activationEvidencePath
      || !claims.authorityPolicyPath || claims.deploymentBlock === undefined) {
    throw new Error("Claims activation configuration is incomplete");
  }
  const [signed, policy] = await Promise.all([
    strictCanonicalFile(claims.activationEvidencePath),
    strictCanonicalFile(claims.authorityPolicyPath),
  ]);
  exactKeys(signed, ["schema", "activation", "approvals"], "signed Claims activation");
  if (signed.schema !== "tohseno.signed-claims-activation/1") throw new Error("signed Claims activation schema is invalid");
  const activation = object(signed.activation, "Claims activation");
  exactKeys(activation, ["schema", "protocol", "component", "contract_version", "activation_sequence",
    "previous_activation", "authority_policy_sha256", "chain_id", "claims_contract", "shot_registry",
    "creation_code_keccak256", "runtime_code_keccak256", "source_commit", "source_tree_sha256",
    "deployment", "issued_at"], "Claims activation");
  if (activation.schema !== "tohseno.claims-activation/1" || activation.protocol !== "tohseno"
      || activation.component !== "TohsenoClaimsV1" || activation.contract_version !== 1
      || activation.activation_sequence !== 1 || activation.previous_activation !== null
      || activation.chain_id !== 4663 || activation.claims_contract !== claims.contractAddress
      || activation.shot_registry !== config.registry.registryAddress) {
    throw new Error("Claims activation identity or active Registry binding is invalid");
  }
  const deployment = object(activation.deployment, "Claims deployment");
  exactKeys(deployment, ["transaction_hash", "block_number", "block_hash"], "Claims deployment");
  if (BigInt(safeInteger(deployment.block_number, "Claims deployment block")) !== claims.deploymentBlock) {
    throw new Error("Claims deployment block differs from configured activation");
  }
  const policyDigest = sha256Text(canonicalCatalogJSON(policy));
  if (activation.authority_policy_sha256 !== policyDigest) throw new Error("Claims activation policy digest differs");
  const canonicalActivation = new TextEncoder().encode(canonicalCatalogJSON(activation));
  const preimage = new Uint8Array(CLAIMS_ACTIVATION_DOMAIN.length + canonicalActivation.length);
  preimage.set(CLAIMS_ACTIVATION_DOMAIN); preimage.set(canonicalActivation, CLAIMS_ACTIVATION_DOMAIN.length);
  const signingDigest = sha256Bytes(preimage);
  if (signingDigest !== claims.activationSigningDigest) throw new Error("Claims activation digest differs from the deployment pin");
  verifyThreshold(policy, signed.approvals, signingDigest);
  const sourceCommit = String(activation.source_commit);
  if (!/^[0-9a-f]{40}$/.test(sourceCommit)) throw new Error("Claims activation source commit is invalid");
  return { signingDigest, claimsContract: address(activation.claims_contract, "Claims contract"),
    shotRegistry: address(activation.shot_registry, "ShotRegistry"),
    runtimeCodeKeccak256: hex32(activation.runtime_code_keccak256, "Claims runtime hash"),
    deploymentBlock: claims.deploymentBlock,
    deploymentTransaction: hex32(deployment.transaction_hash, "Claims deployment transaction"),
    sourceCommit };
}

function verifyThreshold(policy: JsonObject, rawApprovals: unknown, digest: Hex): void {
  exactKeys(policy, ["schema", "protocol", "protocol_major", "purpose", "threshold", "authorities", "issued_at"], "authority policy");
  if (policy.schema !== "tohseno.release-authority-policy/1" || policy.protocol !== "tohseno"
      || policy.protocol_major !== 2 || policy.purpose !== "contract_generation_activation") {
    throw new Error("Claims activation authority policy is invalid");
  }
  const threshold = safeInteger(policy.threshold, "authority threshold");
  const authorities = array(policy.authorities, "authorities").map((entry) => object(entry, "authority"));
  const approvals = array(rawApprovals, "Claims approvals").map((entry) => object(entry, "Claims approval"));
  if (threshold < 1 || approvals.length < threshold || approvals.length > authorities.length) {
    throw new Error("Claims activation authority threshold is not satisfied");
  }
  const keys = new Map(authorities.map((authority) => {
    exactKeys(authority, ["key_id", "public_key"], "authority");
    const key = object(authority.public_key, "authority public key");
    exactKeys(key, ["x", "y"], "authority public key");
    return [hex32(authority.key_id, "authority key ID"), { x: hex32(key.x, "authority x"), y: hex32(key.y, "authority y") }] as const;
  }));
  let prior = "";
  for (const approval of approvals) {
    exactKeys(approval, ["key_id", "authorization"], "Claims approval");
    const keyID = hex32(approval.key_id, "approval key ID");
    if (keyID <= prior) throw new Error("Claims activation approvals are not strictly ordered");
    prior = keyID;
    const key = keys.get(keyID);
    if (!key) throw new Error("Claims activation approval uses an unknown authority");
    const authorization = object(approval.authorization, "Claims authorization");
    exactKeys(authorization, ["algorithm", "digest", "signature", "low_s"], "Claims authorization");
    const signature = object(authorization.signature, "Claims signature");
    exactKeys(signature, ["r", "s"], "Claims signature");
    const r = hex32(signature.r, "Claims signature r");
    const s = hex32(signature.s, "Claims signature s");
    if (authorization.algorithm !== "p256" || authorization.digest !== digest
        || authorization.low_s !== true || BigInt(s) > HALF_ORDER) {
      throw new Error("Claims activation authorization is invalid");
    }
    const compact = new Uint8Array([...hexBytes(r), ...hexBytes(s)]);
    const publicKey = new Uint8Array([4, ...hexBytes(key.x), ...hexBytes(key.y)]);
    if (!p256.verify(compact, hexBytes(digest), publicKey, { prehash: false, lowS: true })) {
      throw new Error("Claims activation signature is invalid");
    }
  }
}

function publicEdition(shotID: Hex, edition: ClaimEditionSnapshot): JsonObject {
  const kind = edition.maxClaims === 0n
    ? edition.closesAt === 0n ? "open" : "timed"
    : edition.closesAt === 0n ? "limited" : "limited_timed";
  return { schema: "tohseno.claim-edition/1", shot_id: shotID, opened: edition.opened,
    policy: edition.opened ? { kind, max_claims: edition.maxClaims === 0n ? null : edition.maxClaims.toString(),
      closes_at: edition.closesAt === 0n ? null : edition.closesAt.toString() } : null,
    total_claims: edition.totalClaims.toString(), opened_at: edition.openedAt === 0n ? null : edition.openedAt.toString(),
    closed: edition.closed };
}

function publicClaim(claim: SoftwareClaimSnapshot): JsonObject { return {
  token_id: claim.tokenID.toString(), shot_id: claim.shotID, claim_number: claim.claimNumber.toString(),
  claimant: claim.claimant, release_digest: claim.releaseDigest,
  checkpoint_digest: claim.checkpointDigest, gesture_commitment: claim.gestureCommitment,
  transaction_hash: claim.transactionHash ?? null,
  canonical_block: claim.blockNumber !== undefined && claim.blockHash
    ? { number: claim.blockNumber.toString(), hash: claim.blockHash,
      transaction_index: claim.transactionIndex ?? null, log_index: claim.logIndex ?? null,
      timestamp: claim.claimedAt ?? null } : null,
}; }

function tokenMetadata(
  claim: SoftwareClaimSnapshot,
  context: ClaimCatalogContext,
  edition: ClaimEditionSnapshot,
): JsonObject { return {
  name: `${context.appName} · Claim #${claim.claimNumber}`,
  description: `A non-transferable receipt for encountering ${context.appName} at one exact Tohseno release.`,
  image: `https://tohseno.com/api/claims/v1/mark/${claim.gestureCommitment}.svg`,
  external_url: `https://tohseno.com/claims/${claim.tokenID}`,
  attributes: [
    { trait_type: "App", value: context.appName },
    { trait_type: "Builder", value: context.builderID },
    { trait_type: "ShotID", value: claim.shotID },
    { trait_type: "Claim number", value: claim.claimNumber.toString() },
    { trait_type: "Edition maximum", value: edition.maxClaims === 0n ? "Open" : edition.maxClaims.toString() },
    { trait_type: "Claimed at", value: claim.claimedAt ?? "Canonical block unavailable" },
    { trait_type: "Release", value: claim.releaseDigest },
    { trait_type: "Public checkpoint", value: claim.checkpointDigest },
    { trait_type: "Gesture commitment", value: claim.gestureCommitment },
    { trait_type: "Transferable", value: "No" },
  ],
}; }

function claimMarkSVG(mark: ValidatedClaimMark): string {
  const points = mark.points.map((point) => `${(point.x * 1000).toFixed(2)},${(point.y * 1000).toFixed(2)}`).join(" ");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000" role="img" aria-label="Normalized Tohseno Claim mark"><rect width="1000" height="1000" rx="120" fill="#11110f"/><rect x="390" y="390" width="220" height="220" rx="54" fill="#f4f0e6"/><polyline points="${points}" fill="none" stroke="#ff641e" stroke-width="24" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
}

function claimReceiptHTML(
  context: ClaimCatalogContext,
  edition: ClaimEditionSnapshot,
  claim: SoftwareClaimSnapshot,
  mark?: ValidatedClaimMark,
): string {
  const editionLabel = edition.maxClaims === 0n ? "Open Edition" : `Claim #${claim.claimNumber} of ${edition.maxClaims}`;
  const markHTML = mark ? claimMarkSVG(mark) : `<div class="mark-missing">Mark renderer unavailable; commitment remains canonical.</div>`;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${escapeHTML(context.appName)} · Claim #${claim.claimNumber} — Tohseno</title><meta name="description" content="A non-transferable receipt for encountering ${escapeHTML(context.appName)}."><link rel="stylesheet" href="/landing.css"><style>body{max-width:760px;margin:auto;padding:32px}.receipt{padding:8vh 0;text-align:center}.mark{max-width:430px;margin:36px auto}.mark svg{width:100%;height:auto}.facts{display:grid;gap:12px;text-align:left;padding:24px;border:1px solid #393632;background:#171615}.facts div{display:grid;gap:4px}.facts span{font:700 .7rem monospace;color:#ff7a1a;letter-spacing:.1em}.facts code{overflow-wrap:anywhere}.mark-missing{padding:80px 20px;border:1px solid #393632}</style></head><body><nav><a href="/">TOHSENO</a> · <a href="/registry">REGISTRY</a></nav><main class="receipt"><p class="eyebrow">SOFTWARE ENCOUNTER</p><h1>${escapeHTML(context.appName)}</h1><p>${escapeHTML(editionLabel)}</p><div class="mark">${markHTML}</div><section class="facts"><div><span>CLAIMED</span><strong>${escapeHTML(claim.claimedAt ?? `block ${claim.blockNumber?.toString() ?? "unknown"}`)}</strong></div><div><span>BUILDER</span><code>${escapeHTML(context.builderID)}</code></div><div><span>EXACT RELEASE</span><code>${claim.releaseDigest}</code></div><div><span>CHECKPOINT</span><code>${claim.checkpointDigest}</code></div><div><span>TOHSENO ADDRESS</span><code>${claim.claimant}</code></div><div><span>TRANSACTION</span><code>${claim.transactionHash ?? "unavailable"}</code></div></section><p>This receipt is non-transferable. The software remains the thing.</p></main></body></html>`;
}

function escapeHTML(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;").replaceAll("'", "&#39;");
}

function substantiallyEnclosesCenter(points: ReadonlyArray<{ x: number; y: number }>): boolean {
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const minX = Math.min(...xs); const maxX = Math.max(...xs);
  const minY = Math.min(...ys); const maxY = Math.max(...ys);
  // Resampling can shave a little from an original stroke whose exact span
  // was 0.24, so canonical-byte validation uses the frozen quantization
  // tolerance while raw capture keeps the stricter Rust/Swift threshold.
  if (maxX - minX < 0.235 || maxY - minY < 0.235
      || minX > 0.425 || maxX < 0.575 || minY > 0.425 || maxY < 0.575) return false;
  let inside = false;
  let prior = points.at(-1)!;
  for (const point of points) {
    const crosses = (point.y > 0.5) !== (prior.y > 0.5)
      && 0.5 < (prior.x - point.x) * (0.5 - point.y) / (prior.y - point.y) + point.x;
    if (crosses) inside = !inside;
    prior = point;
  }
  return inside;
}

async function strictCanonicalFile(path: string): Promise<JsonObject> {
  const text = await readFile(path, "utf8");
  if (text.length > 2 * 1024 * 1024) throw new Error("Claims activation file exceeds its bound");
  const value = JSON.parse(text) as unknown;
  const result = object(value, "Claims activation file");
  if (`${canonicalCatalogJSON(result)}\n` !== text) throw new Error("Claims activation file is not exact canonical JSON");
  return result;
}

function json(value: unknown, status = 200): Response { return withSecurityHeaders(new Response(JSON.stringify(value, (_, item) => typeof item === "bigint" ? item.toString() : item), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" } })); }
function head(response: Response, method: string): Response { return method === "HEAD" ? new Response(null, { status: response.status, headers: response.headers }) : response; }
function methodNotAllowed(): Response { return json({ error: "Method not allowed" }, 405); }
function object(value: unknown, name: string): JsonObject { if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${name} must be an object`); return value as JsonObject; }
function array(value: unknown, name: string): unknown[] { if (!Array.isArray(value)) throw new Error(`${name} must be an array`); return value; }
function exactKeys(value: JsonObject, keys: readonly string[], name: string): void { const observed = Object.keys(value).sort(); const expected = [...keys].sort(); if (observed.length !== expected.length || observed.some((key, index) => key !== expected[index])) throw new Error(`${name} has unknown or missing fields`); }
function address(value: unknown, name: string): Hex { if (typeof value !== "string" || !ADDRESS.test(value)) throw new Error(`${name} is invalid`); return value as Hex; }
function hex32(value: unknown, name: string): Hex { if (typeof value !== "string" || !HEX32.test(value)) throw new HttpError(422, `${name} is invalid`); return value as Hex; }
function accountAddress(value: string): Hex { const builder = value.match(BUILDER_ID)?.[1]; return address(builder ?? value, "Tohseno account"); }
function safeInteger(value: unknown, name: string): number { if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new Error(`${name} is invalid`); return value; }
function exactSecond(value: string): string {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) throw new Error("canonical Claim block time is invalid");
  return date.toISOString().replace(".000Z", "Z");
}
function positiveBigInt(value: string, name: string): bigint { if (!/^[1-9]\d*$/.test(value)) throw new HttpError(422, `${name} is invalid`); return BigInt(value); }
function boundedLimit(value: string | null): number { if (value === null) return 50; if (!/^\d+$/.test(value)) throw new HttpError(400, "limit is invalid"); return Math.max(1, Math.min(100, Number(value))); }
function sha256Text(value: string): Hex { return sha256Bytes(new TextEncoder().encode(value)); }
function sha256Bytes(value: Uint8Array): Hex { return `0x${new Bun.CryptoHasher("sha256").update(value).digest("hex")}`; }
function builderAccountSalt(x: Hex, y: Hex): Hex {
  const keyID = keccak_256(concatBytes(hexBytes(x), hexBytes(y)));
  return sha256Bytes(concatBytes(new TextEncoder().encode("TOHSENO-BUILDER-SALT-V1\0"), keyID));
}
function hexBytes(value: Hex): Uint8Array { return Uint8Array.from(value.slice(2).match(/../g)!.map((byte) => Number.parseInt(byte, 16))); }
function bytesToHex(value: Uint8Array): string { return [...value].map((byte) => byte.toString(16).padStart(2, "0")).join(""); }
function concatBytes(...values: Uint8Array[]): Uint8Array { const result = new Uint8Array(values.reduce((sum, value) => sum + value.length, 0)); let offset = 0; for (const value of values) { result.set(value, offset); offset += value.length; } return result; }
function uintWord(value: number): Uint8Array { if (!Number.isSafeInteger(value) || value < 0) throw new Error("Claims action integer is invalid"); return hexBytes(`0x${BigInt(value).toString(16).padStart(64, "0")}` as Hex); }
function addressWord(value: Hex): Uint8Array { const normalized = address(value, "Claims action address"); return hexBytes(`0x${normalized.slice(2).padStart(64, "0")}` as Hex); }
