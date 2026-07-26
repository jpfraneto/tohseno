import {
  identityReferencesEqual,
  type IdentityReference,
  type VerificationMethod,
  validateIdentityReference,
  validateVerificationMethod,
  verificationMethodsEqual,
} from "../../identity/src/index.ts";
import type { SignatureVerifier } from "../../signer/src/index.ts";
import {
  canonicalJson,
  hashSignedPublicShotRecord,
  parseSignedPublicShotRecord,
  PUBLIC_SHOT_PROTOCOL_VERSION,
  type AppcoinLink,
  type AppStoreEvidence,
  type CanonicalTimestamp,
  type PublicationEvidence,
  type PublicShotLifecycle,
  type Sha256Hash,
  type ShotId,
  type SignedPublicShotRecord,
  type VerifiedSignedPublicShotRecord,
  validateAppcoinLink,
  validateAppStoreEvidence,
  validateCanonicalTimestamp,
  validatePublicationEvidence,
  validateSha256Hash,
  validateShotId,
  verifySignedPublicShotRecord,
} from "../../protocol/src/index.ts";

export interface PublicEvolutionProjection {
  evolution: number;
  title: string;
  summary: string;
  recordedAt: CanonicalTimestamp;
  recordHash: Sha256Hash;
}

export interface PublicAppcoinProjection {
  link: AppcoinLink;
  recordedAt: CanonicalTimestamp;
  recordHash: Sha256Hash;
}

export interface PublicShotProjection {
  protocolVersion: typeof PUBLIC_SHOT_PROTOCOL_VERSION;
  shotId: ShotId;
  name: string;
  summary: string;
  platform: string;
  builder: IdentityReference;
  continuity?: IdentityReference;
  authority: VerificationMethod;
  lifecycle: PublicShotLifecycle;
  evolution: number;
  createdAt: CanonicalTimestamp;
  updatedAt: CanonicalTimestamp;
  latestRecordHash: Sha256Hash;
  recordCount: number;
  evolutions: PublicEvolutionProjection[];
  publication?: PublicationEvidence;
  appStore?: AppStoreEvidence;
  appcoins: PublicAppcoinProjection[];
}

export interface RegistryAppendResult {
  status: "appended" | "existing";
  recordHash: Sha256Hash;
  projection: PublicShotProjection;
}

export interface PublicRecordRegistry {
  append(record: SignedPublicShotRecord): Promise<RegistryAppendResult>;
  getProjection(shotId: string): PublicShotProjection | undefined;
  getRecords(shotId: string): readonly SignedPublicShotRecord[];
  listProjections(): readonly PublicShotProjection[];
}

export type RegistryErrorCode =
  | "empty-history"
  | "invalid-genesis"
  | "sequence-conflict"
  | "sequence-gap"
  | "previous-hash-mismatch"
  | "shot-id-mismatch"
  | "authority-change"
  | "timestamp-conflict"
  | "evolution-conflict"
  | "lifecycle-conflict"
  | "appcoin-conflict";

export class RegistryError extends Error {
  override readonly name = "RegistryError";

  constructor(
    readonly code: RegistryErrorCode,
    message: string,
  ) {
    super(message);
  }
}

export interface RecordAnchorRequest {
  shotId: ShotId;
  sequence: number;
  recordHash: Sha256Hash;
}

export interface RecordAnchorReceipt {
  adapter: string;
  anchorId: string;
  anchoredAt: CanonicalTimestamp;
  evidenceUrl?: string;
}

/**
 * Chain-neutral seam only. Implementations and deployment actions live outside
 * the protocol and registry packages.
 */
export interface RecordAnchorAdapter {
  anchor(request: RecordAnchorRequest): Promise<RecordAnchorReceipt>;
}

function unreachable(value: never): never {
  throw new RegistryError(
    "invalid-genesis",
    `unsupported record kind ${String(value)}`,
  );
}

function cloneProjection(projection: PublicShotProjection): PublicShotProjection {
  return validatePublicShotProjection(JSON.parse(canonicalJson(projection)));
}

function appcoinIdentity(link: AppcoinLink): string {
  return canonicalJson({
    network: link.network,
    asset: link.asset,
  });
}

export function projectPublicShotProjection(
  history: readonly VerifiedSignedPublicShotRecord[],
): PublicShotProjection {
  if (history.length === 0) {
    throw new RegistryError("empty-history", "a Shot history cannot be empty");
  }
  const records = [...history];
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index];
    if (record === undefined || record.sequence !== index) {
      throw new RegistryError(
        record !== undefined && record.sequence < index
          ? "sequence-conflict"
          : "sequence-gap",
        `history must already be in canonical sequence order; expected sequence ${index}`,
      );
    }
  }
  const genesis = records[0];
  if (
    genesis === undefined ||
    genesis.kind !== "SHOT_CREATED" ||
    genesis.sequence !== 0 ||
    genesis.previousRecordHash !== null
  ) {
    throw new RegistryError(
      "invalid-genesis",
      "history must start with SHOT_CREATED at sequence 0",
    );
  }

  const genesisHash = hashSignedPublicShotRecord(genesis);
  const projection: PublicShotProjection = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    shotId: genesis.shotId,
    name: genesis.body.name,
    summary: genesis.body.summary,
    platform: genesis.body.platform,
    builder: genesis.body.builder,
    authority: genesis.authority,
    lifecycle: "EVOLVING",
    evolution: 0,
    createdAt: genesis.recordedAt,
    updatedAt: genesis.recordedAt,
    latestRecordHash: genesisHash,
    recordCount: 1,
    evolutions: [],
    appcoins: [],
  };
  if (genesis.body.continuity !== undefined) {
    projection.continuity = genesis.body.continuity;
  }

  const linkedAssets = new Set<string>();
  for (let index = 1; index < records.length; index += 1) {
    const record = records[index];
    const previous = records[index - 1];
    if (record === undefined || previous === undefined) {
      throw new RegistryError("sequence-gap", "history has a sequence gap");
    }
    if (record.sequence !== index) {
      throw new RegistryError(
        record.sequence < index ? "sequence-conflict" : "sequence-gap",
        `expected sequence ${index}`,
      );
    }
    if (record.shotId !== genesis.shotId) {
      throw new RegistryError(
        "shot-id-mismatch",
        "all records must use the genesis Shot ID",
      );
    }
    if (!verificationMethodsEqual(record.authority, genesis.authority)) {
      throw new RegistryError(
        "authority-change",
        "record authority cannot change within protocol v1",
      );
    }
    const expectedPreviousHash = hashSignedPublicShotRecord(previous);
    if (record.previousRecordHash !== expectedPreviousHash) {
      throw new RegistryError(
        "previous-hash-mismatch",
        "record does not link to the preceding signed record",
      );
    }
    if (record.recordedAt < previous.recordedAt) {
      throw new RegistryError(
        "timestamp-conflict",
        "recordedAt cannot precede the preceding signed record",
      );
    }
    const recordHash = hashSignedPublicShotRecord(record);
    switch (record.kind) {
      case "SHOT_CREATED":
        throw new RegistryError(
          "invalid-genesis",
          "SHOT_CREATED is valid only at sequence 0",
        );
      case "EVOLUTION_RECORDED": {
        if (record.body.evolution !== projection.evolution + 1) {
          throw new RegistryError(
            "evolution-conflict",
            `expected evolution ${projection.evolution + 1}`,
          );
        }
        const evolutionProjection: PublicEvolutionProjection = {
          evolution: record.body.evolution,
          title: record.body.title,
          summary: record.body.summary,
          recordedAt: record.recordedAt,
          recordHash,
        };
        projection.evolution = record.body.evolution;
        projection.summary = record.body.summary;
        projection.evolutions.push(evolutionProjection);
        break;
      }
      case "LIFECYCLE_TRANSITIONED":
        if (record.body.from !== projection.lifecycle) {
          throw new RegistryError(
            "lifecycle-conflict",
            `transition starts at ${record.body.from}, not ${projection.lifecycle}`,
          );
        }
        if (
          record.body.from === "EVOLVING" &&
          record.body.to === "PUBLISHED"
        ) {
          projection.lifecycle = "PUBLISHED";
          projection.publication = record.body.evidence;
        } else if (
          record.body.from === "PUBLISHED" &&
          record.body.to === "APP_STORE"
        ) {
          projection.lifecycle = "APP_STORE";
          projection.appStore = record.body.evidence;
        } else {
          throw new RegistryError(
            "lifecycle-conflict",
            "the lifecycle transition is not monotonic",
          );
        }
        break;
      case "APPCOIN_LINKED": {
        const identity = appcoinIdentity(record.body.link);
        if (linkedAssets.has(identity)) {
          throw new RegistryError(
            "appcoin-conflict",
            "an external asset can be linked only once per Shot",
          );
        }
        linkedAssets.add(identity);
        projection.appcoins.push({
          link: record.body.link,
          recordedAt: record.recordedAt,
          recordHash,
        });
        break;
      }
      default:
        unreachable(record);
    }
    projection.updatedAt = record.recordedAt;
    projection.latestRecordHash = recordHash;
    projection.recordCount = index + 1;
  }
  return validatePublicShotProjection(JSON.parse(canonicalJson(projection)));
}

function plainObject(value: unknown, path: string): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new RegistryError("invalid-genesis", `${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function exactFields(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
  path: string,
): void {
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) {
      throw new RegistryError(
        "invalid-genesis",
        `${path}.${key} is not allowed`,
      );
    }
  }
  for (const key of required) {
    if (!Object.hasOwn(value, key)) {
      throw new RegistryError(
        "invalid-genesis",
        `${path}.${key} is required`,
      );
    }
  }
}

function publicString(
  value: unknown,
  path: string,
  maximum: number,
): string {
  if (
    typeof value !== "string" ||
    value.length < 1 ||
    value.length > maximum ||
    value !== value.trim() ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new RegistryError("invalid-genesis", `${path} is invalid`);
  }
  canonicalJson(value);
  return value;
}

function projectionInteger(
  value: unknown,
  path: string,
  minimum: number,
): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum) {
    throw new RegistryError("invalid-genesis", `${path} is invalid`);
  }
  return value as number;
}

function evolutionProjection(
  value: unknown,
  path: string,
): PublicEvolutionProjection {
  const candidate = plainObject(value, path);
  exactFields(
    candidate,
    ["evolution", "title", "summary", "recordedAt", "recordHash"],
    [],
    path,
  );
  return {
    evolution: projectionInteger(candidate.evolution, `${path}.evolution`, 1),
    title: publicString(candidate.title, `${path}.title`, 160),
    summary: publicString(candidate.summary, `${path}.summary`, 1_000),
    recordedAt: validateCanonicalTimestamp(
      candidate.recordedAt,
      `${path}.recordedAt`,
    ),
    recordHash: validateSha256Hash(
      candidate.recordHash,
      `${path}.recordHash`,
    ),
  };
}

function appcoinProjection(
  value: unknown,
  path: string,
): PublicAppcoinProjection {
  const candidate = plainObject(value, path);
  exactFields(candidate, ["link", "recordedAt", "recordHash"], [], path);
  return {
    link: validateAppcoinLink(candidate.link, `${path}.link`),
    recordedAt: validateCanonicalTimestamp(
      candidate.recordedAt,
      `${path}.recordedAt`,
    ),
    recordHash: validateSha256Hash(
      candidate.recordHash,
      `${path}.recordHash`,
    ),
  };
}

export function validatePublicShotProjection(
  value: unknown,
): PublicShotProjection {
  const candidate = plainObject(value, "projection");
  exactFields(
    candidate,
    [
      "protocolVersion",
      "shotId",
      "name",
      "summary",
      "platform",
      "builder",
      "authority",
      "lifecycle",
      "evolution",
      "createdAt",
      "updatedAt",
      "latestRecordHash",
      "recordCount",
      "evolutions",
      "appcoins",
    ],
    ["continuity", "publication", "appStore"],
    "projection",
  );
  if (candidate.protocolVersion !== PUBLIC_SHOT_PROTOCOL_VERSION) {
    throw new RegistryError(
      "invalid-genesis",
      "projection.protocolVersion is unsupported",
    );
  }
  if (
    candidate.lifecycle !== "EVOLVING" &&
    candidate.lifecycle !== "PUBLISHED" &&
    candidate.lifecycle !== "APP_STORE"
  ) {
    throw new RegistryError(
      "lifecycle-conflict",
      "projection.lifecycle is invalid",
    );
  }
  if (!Array.isArray(candidate.evolutions) || !Array.isArray(candidate.appcoins)) {
    throw new RegistryError(
      "invalid-genesis",
      "projection histories must be arrays",
    );
  }
  const authority = validateVerificationMethod(
    candidate.authority,
    "projection.authority",
  );
  const builder = validateIdentityReference(
    candidate.builder,
    "projection.builder",
  );
  if (
    builder.role !== "BUILDER" ||
    !identityReferencesEqual(builder, authority.identity)
  ) {
    throw new RegistryError(
      "authority-change",
      "projection Builder must equal its authority identity",
    );
  }
  const evolutions = candidate.evolutions.map((entry, index) =>
    evolutionProjection(entry, `projection.evolutions[${index}]`)
  );
  const evolution = projectionInteger(
    candidate.evolution,
    "projection.evolution",
    0,
  );
  if (
    evolutions.length !== evolution ||
    evolutions.some((entry, index) => entry.evolution !== index + 1)
  ) {
    throw new RegistryError(
      "evolution-conflict",
      "projection evolution history is not contiguous",
    );
  }
  const result: PublicShotProjection = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    shotId: validateShotId(candidate.shotId, "projection.shotId"),
    name: publicString(candidate.name, "projection.name", 120),
    summary: publicString(candidate.summary, "projection.summary", 1_000),
    platform: publicString(candidate.platform, "projection.platform", 32),
    builder,
    authority,
    lifecycle: candidate.lifecycle,
    evolution,
    createdAt: validateCanonicalTimestamp(
      candidate.createdAt,
      "projection.createdAt",
    ),
    updatedAt: validateCanonicalTimestamp(
      candidate.updatedAt,
      "projection.updatedAt",
    ),
    latestRecordHash: validateSha256Hash(
      candidate.latestRecordHash,
      "projection.latestRecordHash",
    ),
    recordCount: projectionInteger(
      candidate.recordCount,
      "projection.recordCount",
      1,
    ),
    evolutions,
    appcoins: candidate.appcoins.map((entry, index) =>
      appcoinProjection(entry, `projection.appcoins[${index}]`)
    ),
  };
  if (result.updatedAt < result.createdAt) {
    throw new RegistryError(
      "timestamp-conflict",
      "projection.updatedAt cannot precede projection.createdAt",
    );
  }
  if (!/^[A-Za-z][A-Za-z0-9._-]{0,31}$/u.test(result.platform)) {
    throw new RegistryError(
      "invalid-genesis",
      "projection.platform is invalid",
    );
  }
  if (candidate.continuity !== undefined) {
    const continuity = validateIdentityReference(
      candidate.continuity,
      "projection.continuity",
    );
    if (
      continuity.role !== "CONTINUITY" ||
      (continuity.method === builder.method && continuity.id === builder.id)
    ) {
      throw new RegistryError(
        "invalid-genesis",
        "projection Continuity identity is invalid",
      );
    }
    result.continuity = continuity;
  }
  if (candidate.publication !== undefined) {
    result.publication = validatePublicationEvidence(
      candidate.publication,
      "projection.publication",
    );
  }
  if (candidate.appStore !== undefined) {
    result.appStore = validateAppStoreEvidence(
      candidate.appStore,
      "projection.appStore",
    );
  }
  if (
    (result.lifecycle === "EVOLVING" &&
      (result.publication !== undefined || result.appStore !== undefined)) ||
    (result.lifecycle === "PUBLISHED" &&
      (result.publication === undefined || result.appStore !== undefined)) ||
    (result.lifecycle === "APP_STORE" &&
      (result.publication === undefined || result.appStore === undefined))
  ) {
    throw new RegistryError(
      "lifecycle-conflict",
      "projection evidence does not match its lifecycle",
    );
  }
  const transitionCount = result.lifecycle === "EVOLVING"
    ? 0
    : result.lifecycle === "PUBLISHED"
    ? 1
    : 2;
  if (
    result.recordCount !==
      1 + result.evolutions.length + result.appcoins.length + transitionCount
  ) {
    throw new RegistryError(
      "sequence-conflict",
      "projection.recordCount does not match its projected histories",
    );
  }
  if (
    result.evolutions.length > 0 &&
    result.summary !== result.evolutions.at(-1)?.summary
  ) {
    throw new RegistryError(
      "evolution-conflict",
      "projection.summary does not match its latest evolution",
    );
  }
  const appcoinKeys = result.appcoins.map((entry) =>
    appcoinIdentity(entry.link)
  );
  if (new Set(appcoinKeys).size !== appcoinKeys.length) {
    throw new RegistryError(
      "appcoin-conflict",
      "projection contains a duplicate external asset link",
    );
  }
  return result;
}

export class InMemoryRegistry implements PublicRecordRegistry {
  readonly #records = new Map<string, string[]>();
  readonly #projections = new Map<string, string>();

  constructor(private readonly verifier: SignatureVerifier) {}

  async append(
    recordValue: SignedPublicShotRecord,
  ): Promise<RegistryAppendResult> {
    const record = await verifySignedPublicShotRecord(
      recordValue,
      this.verifier,
    );
    const recordJson = canonicalJson(record);
    const recordHash = hashSignedPublicShotRecord(record);
    const existingJson = this.#records.get(record.shotId) ?? [];
    const atSequence = existingJson[record.sequence];
    if (atSequence !== undefined) {
      if (atSequence === recordJson) {
        const existingProjection = this.getProjection(record.shotId);
        if (existingProjection === undefined) {
          throw new RegistryError(
            "invalid-genesis",
            "stored history has no projection",
          );
        }
        return {
          status: "existing",
          recordHash,
          projection: existingProjection,
        };
      }
      throw new RegistryError(
        "sequence-conflict",
        `sequence ${record.sequence} is already occupied`,
      );
    }
    if (record.sequence !== existingJson.length) {
      throw new RegistryError(
        "sequence-gap",
        `expected sequence ${existingJson.length}`,
      );
    }
    const verifiedHistory = [
      ...existingJson.map((source) =>
        parseSignedPublicShotRecord(JSON.parse(source)) as
          VerifiedSignedPublicShotRecord
      ),
      record,
    ];
    const projection = projectPublicShotProjection(verifiedHistory);
    this.#records.set(record.shotId, [...existingJson, recordJson]);
    this.#projections.set(record.shotId, canonicalJson(projection));
    return {
      status: "appended",
      recordHash,
      projection: cloneProjection(projection),
    };
  }

  getProjection(shotId: string): PublicShotProjection | undefined {
    const source = this.#projections.get(shotId);
    return source === undefined
      ? undefined
      : validatePublicShotProjection(JSON.parse(source));
  }

  getRecords(shotId: string): readonly SignedPublicShotRecord[] {
    return (this.#records.get(shotId) ?? []).map((source) =>
      parseSignedPublicShotRecord(JSON.parse(source))
    );
  }

  listProjections(): readonly PublicShotProjection[] {
    return [...this.#projections.entries()]
      .toSorted(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
      .map(([, source]) => validatePublicShotProjection(JSON.parse(source)));
  }
}
