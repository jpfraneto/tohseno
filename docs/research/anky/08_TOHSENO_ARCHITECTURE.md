# Proposed TOHSENO architecture

## Design basis

This is not a clean-room architecture. It starts from seams already visible in Anky:

- cross-language protocol/identity fixtures;
- native local stores and recovery flows;
- a pure-ish writing engine;
- exact-body signed backend requests;
- a provider router with privacy capabilities;
- a local archive as the primary record;
- adjacent Gratitude Lock code proving that the next action may not be text.

It also preserves current constraints:

- Anky remains native SwiftUI and Jetpack Compose;
- existing `.anky` data and EVM identity cannot be invalidated;
- server and clients upgrade at different times;
- platform secure storage and lifecycle behavior cannot be hidden behind a TypeScript-only fantasy;
- no external provider, payment, screen-time gate, cloud sync, or blockchain is required to receive first value.

## Current implementation

There is no TOHSENO package layer today. The reusable candidates remain inside Anky's native protocol/identity/storage code, the TypeScript protocol implementation, and the Bun backend. `apps/gratitude-lock` is a separate prototype rather than a consumer of shared continuity code. Consequently, every package below is proposed; “origin from Anky” identifies source material, not an already extracted module.

Evidence:

- `apps/ios/Package.swift:5-39`.
- `apps/android/app/build.gradle.kts:37-159`.
- `protocol/implementations/typescript/src`.
- `backend/server.ts:1272-1345`.
- `apps/gratitude-lock/v0/ios/GratitudeLockByAnky/ContentView.swift:4-54`.

## Proposed monorepo

This is an eventual structure, not a request to move current files now:

```text
tohseno/
├── AGENTS.md
├── README.md
├── apps/
│   ├── anky-reference/                 # adapters to existing native apps; move only much later
│   │   ├── apple/
│   │   ├── android/
│   │   └── server-policy/
│   └── gratitude-lock-example/         # materially different proof app
├── packages/
│   ├── continuity-domain/              # pure commands, reducer, policies, event types
│   ├── continuity-artifact/            # opaque bytes, codecs, digests, event envelope
│   ├── continuity-log/                 # repository ports, transactions, migrations
│   ├── practice-identity/               # identity suites, scoped statements, recovery ports
│   ├── reflection/                      # policy/provider/result/consent contracts
│   ├── apple-adapters/                  # Swift package: files, Keychain, lifecycle
│   ├── android-adapters/                # Kotlin library: files, Keystore, lifecycle
│   └── anky-compat/                     # .anky v0 and anky.base.eoa.v1 compatibility
├── services/
│   └── reflection-server/              # Bun reference host using the contracts
├── fixtures/
│   ├── lifecycle/
│   ├── artifacts/
│   ├── identity/
│   ├── backup/
│   └── reflection/
├── templates/
│   └── native-continuity-app/           # only after two apps prove the seam
├── skills/
│   └── continuity-app/
└── docs/
```

Why this differs from the prompt's illustrative tree:

- `core-action` becomes `continuity-domain` because outcome/event semantics and action policy must be designed together.
- `local-storage` and `secure-storage` are ports inside domain packages plus explicit Apple/Android adapters. A supposedly universal storage implementation would obscure platform guarantees.
- `continuity-proof` and `sync` are deliberately deferred; current Anky does not implement their promised semantics.
- payments are an optional app/server adapter, not a framework package.
- `anky-compat` isolates frozen current contracts instead of renaming them.
- fixtures are top-level because Swift, Kotlin, TypeScript, and future languages must consume the same corpus.

## Dependency rule

```text
                    continuity-domain
                    /        |       \
      artifact + event   identity   reflection contracts
              \             |             /
                    continuity-log ports
                             |
          +------------------+------------------+
          |                                     |
     Apple adapters                       Android adapters
          |                                     |
      Anky Swift UI                        Anky Compose UI

reflection-server depends inward on artifact/identity/reflection contracts.
No domain package depends on RevenueCat, screen-time APIs, model vendors, or Anky UI.
```

App policies may depend on framework contracts. Framework packages must never depend on app policies.

## Package specifications

### `continuity-domain`

**Responsibility**

Represent action commands, state, policy decisions, explicit completion/interruption, seal reasons, and immutable event metadata. It is a pure reducer with no clocks, filesystem, crypto, network, UI, analytics, or payment code.

**Proposed public API**

```ts
interface CoreActionPolicy<State, Progress> {
  readonly id: string;
  readonly version: string;
  initial(): State;
  reduce(state: State, command: ActionCommand<Progress>): Transition<State>;
}

type Transition<State> = {
  state: State;
  facts: ActionFact[];
};

type ActionFact =
  | Started
  | Progressed
  | CheckpointRequested
  | CompletionConditionMet
  | Sealed;
```

**Dependencies:** none beyond standard types.

**Compatibility:** language-neutral semantics with TypeScript reference types; equivalent Swift/Kotlin APIs or generated fixtures, not a JS runtime embedded in native apps.

**Origin from Anky:** state transitions inferred from `WritingSessionEngine` and native `WriteViewModel`s.

**Rewrite required:** remove `UnlockPolicy`, `.anky`, haptics, storage, reflection, level callbacks, and UI assumptions; add explicit outcomes.

**Version one:** yes.

**Publication:** internal first; public after Anky plus Gratitude Lock pass the same lifecycle fixtures.

### `continuity-artifact`

**Responsibility**

Treat action output as opaque bytes/blobs, pair it with codec/media type and digest, and define a versioned event envelope whose stable ID does not change when derived artifacts appear.

**Proposed public API**

```ts
interface ArtifactCodec<Value> {
  readonly id: string;
  readonly mediaType: string;
  encode(value: Value): Uint8Array;
  decode(bytes: Uint8Array): Value;
  validate(bytes: Uint8Array): Validation;
}

interface ArtifactRef {
  codec: string;
  digest: Digest;
  storageKey: string;
}

interface EventEnvelope {
  schemaVersion: string;
  eventId: string;
  appId: string;
  policy: { id: string; version: string };
  outcome: "completed" | "interrupted";
  artifact: ArtifactRef;
}
```

**Dependencies:** standard byte/digest facility; domain event types.

**Compatibility:** TypeScript core contract and native equivalents consuming shared JSON/binary fixtures. Supports text, JSON, audio, image, and compound manifests.

**Origin from Anky:** `protocol/SPEC.md`, hash helpers, archive naming, fixtures.

**Rewrite required:** version/event envelope, stable ID, codec registry, explicit relations and migrations. `.anky v0` stays unchanged in `anky-compat`.

**Version one:** yes.

**Publication:** internal initially because canonical hash/terminal decisions remain open.

### `continuity-log`

**Responsibility**

Make local persistence the transaction boundary: checkpoint active action; append immutable event; store artifact; rebuild projections; run migrations; expose idempotent post-persist subscriptions.

**Proposed public API**

```ts
interface ContinuityRepository {
  loadCheckpoint(actionId: string): Promise<Checkpoint | null>;
  saveCheckpoint(checkpoint: Checkpoint): Promise<void>;
  commit(input: CommitEvent): Promise<CommittedEvent>;
  list(query?: EventQuery): AsyncIterable<ContinuityEvent>;
  get(eventId: string): Promise<ContinuityEvent | null>;
  deleteLocal(scope: DeleteScope): Promise<DeleteReport>;
}
```

`commit` must define ordering/atomicity among artifact bytes, event envelope, and checkpoint clearing. Indexes/statistics are projections, never canonical records.

**Dependencies:** `continuity-domain`, `continuity-artifact`.

**Compatibility:** interface is portable; concrete storage is Apple/Android/server-specific.

**Origin from Anky:** active draft, local archive, session index rebuild, reflection repository, unreported level queue.

**Rewrite required:** explicit event log, transactions/reconciliation, schema migrations, encryption port, remove absolute URLs/previews from canonical data.

**Version one:** yes.

**Publication:** internal until crash/migration tests pass on both platforms.

### `practice-identity`

**Responsibility**

Create/load a contextual practice identity, sign scoped statements, verify them, store recovery material through a secure port, and plan rotation/recovery without equating a key with a whole person.

**Proposed public API**

```ts
interface IdentitySuite<RecoveryMaterial> {
  readonly id: string;
  generate(): Promise<RecoveryMaterial>;
  derive(material: RecoveryMaterial): Promise<PracticeIdentity>;
  verify(statement: SignedStatement): Promise<boolean>;
}

interface PracticeIdentityStore {
  loadOrCreate(scope: PracticeScope): Promise<PracticeIdentity>;
  exportRecovery(scope: PracticeScope, consent: Consent): Promise<RecoveryPackage>;
  planImport(input: RecoveryPackage): Promise<IdentityMigrationPlan>;
}

interface OperationStatement {
  version: string;
  scope: PracticeScope;
  audience: string;
  operation: string;
  bodyDigest: Digest;
  issuedAt: string;
  expiresAt: string;
  nonce: string;
}
```

**Dependencies:** crypto suite adapter, secure-secret-store port; artifact digest type only.

**Compatibility:** Base EOA, Ed25519, passkey/hardware-backed, or other versioned suites; Swift/Kotlin/TypeScript adapters.

**Origin from Anky:** BIP-39/44 logic, Keychain/Keystore stores, EIP-712 fixtures, server verification.

**Rewrite required:** generic names/scopes, endpoint-bound statement, durable nonce policy, rotation/bridge model. Preserve `anky.base.eoa.v1` exactly as an adapter.

**Version one:** yes, but only the interface and Anky compatibility suite.

**Publication:** internal until recovery and endpoint-binding decisions are settled.

### `reflection`

**Responsibility**

Define reflection policy preparation, consent/disclosure context, provider capabilities, streamed/non-streamed output, provenance, and local result repository. It must permit local, server, external-provider, or no reflection.

**Proposed public API**

```ts
interface ReflectionPolicy<Input, Prompt, Output> {
  readonly id: string;
  readonly version: string;
  prepare(input: Input, context: ReflectionContext): Prompt;
  validate(output: unknown): Output;
}

interface ReflectionProvider<Prompt, Output> {
  readonly id: string;
  readonly capabilities: ProviderCapabilities;
  reflect(prompt: Prompt, context: ProviderContext): Promise<Output>;
}

interface StoredReflection<Output> {
  reflectionId: string;
  eventId: string;
  inputDigest: Digest;
  policy: { id: string; version: string };
  provider: ProviderProvenance;
  consent: DisclosureReceipt;
  output: Output;
}
```

**Dependencies:** event/artifact identifiers; optional identity authorization adapter.

**Compatibility:** Bun server reference plus native local providers/repositories.

**Origin from Anky:** `ReflectionProvider`, privacy flags/router, pending request stores, local reflection JSON.

**Rewrite required:** remove Anky `Env`, tiers/prompts, RevenueCat, Markdown assumptions; add versioned output, provenance, consent, cached-result semantics.

**Version one:** yes.

**Publication:** provider interface can become public after the Anky server is adapted; repository/output remain internal initially.

### `apple-adapters`

**Responsibility**

Implement `Clock`, lifecycle events, atomic/encrypted private files, Keychain secret access, biometric consent, iCloud backup destination, notifications, and optional platform UI bridges.

**Dependencies:** Apple frameworks and TOHSENO ports.

**Compatibility:** iOS/macOS as explicitly supported. FamilyControls/Screen Time is a separate Anky optional adapter, not this package's default.

**Origin from Anky:** `KeychainClient`, atomic stores, `ICloudBackupStore`, notification and scene-phase patterns.

**Rewrite required:** generic aliases/paths, migrations, explicit protection-class guarantees, no Anky UI.

**Version one:** yes for files/Keychain/lifecycle; iCloud later.

**Publication:** internal until entitlement/backup behavior is tested in a sample app.

### `android-adapters`

**Responsibility**

Implement clock/lifecycle, atomic and optionally application-encrypted files, Android Keystore secret access, biometric consent, notification/work scheduling, and share/export bridges.

**Dependencies:** AndroidX/platform APIs and TOHSENO ports.

**Compatibility:** declare API range based on actual cryptographic/file APIs. UsageStats/foreground shield is a separate Anky adapter.

**Origin from Anky:** identity store, storage/export, lifecycle, notification code.

**Rewrite required:** atomic file writes, key-loss/reinstall state, migration/reconciliation, configurable backup destination.

**Version one:** yes for files/Keystore/lifecycle.

**Publication:** internal until uninstall/key-loss and crash tests pass.

### `anky-compat`

**Responsibility**

Freeze and test `.anky v0`, `anky.base.eoa.v1`, exact legacy request fixtures, reflection JSON/ZIP import, and old hash lookup. It is a compatibility boundary, not the generic domain.

**Dependencies:** artifact and identity contracts.

**Compatibility:** Swift/Kotlin/TypeScript implementations against one corpus.

**Origin from Anky:** direct, with behavior preserved.

**Rewrite required:** packaging and adapters only; no semantic cleanup without a new version.

**Version one:** yes.

**Publication:** initially internal; artifact readers may later be public.

### `services/reflection-server`

**Responsibility**

Provide a deployable Bun reference service that composes request authorization, quota/idempotency, consent-aware reflection policies, provider routing, and deletion inventory.

**Dependencies:** `practice-identity`, `reflection`, `continuity-artifact`; SQLite adapter; optional RevenueCat/model providers.

**Compatibility:** self-hosted container; local deterministic provider must work without commercial keys.

**Origin from Anky:** backend route auth, provider router, SQLite stores, Railway deployment.

**Rewrite required:** decompose `server.ts`, version result envelopes, endpoint-bound auth, configurable app policies, complete file deletion.

**Version one:** yes as reference host, not as mandatory global service.

**Publication:** deployable example; framework contracts remain usable without it.

### Deferred packages

#### `continuity-proof`

Not a version-one public package. First define whether proof means self-attestation, device attestation, server witnessing, zero-knowledge claim, or public-chain receipt. An experimental internal type may store a narrowly worded signed statement.

#### `sync`

Not version one. Anky has backup and server aggregates, not multi-device conflict-free event synchronization. Identity, encryption keys, deletion, merges, and attachment transport must be resolved first.

#### `payments`

No core package. Expose a small optional entitlement capability at the application boundary. Reference adapters may use RevenueCat; core action/local record must remain usable without it.

#### `screen-time-gate`

No generic package initially. Preserve Anky's Apple/Android implementations as reference/optional app modules. Cross-platform enforcement APIs and store policies are too product/platform-specific.

## Event and effect boundary

The most important runtime architecture is:

```text
UI/platform input
    |
    v
pure policy reducer ---- facts ----> immediate local feedback
    |
 checkpoint request
    v
ContinuityRepository
    |
 commit immutable event + artifact + clear checkpoint
    |
    +--> ReflectionSubscriber (consent/network/entitlement)
    +--> AccumulationSubscriber (local first; optional server aggregate)
    +--> ProofSubscriber (opt-in, later)
    +--> SyncSubscriber (optional, later)
    +--> InvitationSubscriber (notification/gate/UI)
```

A subscriber failure cannot invalidate the committed event. Every subscriber needs an idempotency key based on stable `eventId`, not a mutable artifact filename.

## Supporting Anky without making it stop being Anky

The migration strategy is adapter-first:

1. Keep `apps/ios`, `apps/android`, and `backend` in their current paths.
2. Introduce shared fixture contracts alongside current code.
3. Put internal interfaces behind existing `WriteViewModel`, stores, and routes.
4. Make current implementations conform without changing UI/format.
5. Add event-envelope sidecars while continuing to read/write `.anky v0`.
6. Run old and new projections in shadow/compare mode.
7. Only move/package code after behavior and migrations are proven.

Anky-specific policies remain:

- eight-minute completeness and terminal silence;
- forward-only grapheme capture;
- `.anky v0`;
- sentence/dip/full reflection;
- companion/painting/journey;
- write-before-scroll;
- RevenueCat product behavior;
- current Base EOA adapter for compatibility.

## Version-one surface

| Capability | V1 | Initial visibility |
|---|---|---|
| Domain reducer/policy | Yes | Internal → public after second app |
| Event/artifact contracts | Yes | Internal |
| Local repository ports | Yes | Internal |
| Apple/Android local adapters | Yes | Internal |
| Practice identity interface + Anky suite | Yes | Internal |
| Reflection interface + reference server | Yes | Provider API may be public |
| Anky compatibility fixtures | Yes | Public fixtures/readers later |
| Manifest and validator | Yes | Public |
| Gratitude Lock example | Yes, after kernel works | Public example |
| Proof | Experimental only | Internal |
| Sync | No | — |
| Payments | Adapter example only | App-specific |
| Screen-time gate | Anky reference only | App-specific |
| Blockchain | No core dependency | Optional example |
| Template/agent skill | After two apps | Public |

## Interpretation

The initial framework is intentionally modest. The value is not the number of packages; it is a reliable vertical contract:

> a configured action can progress offline, survive interruption, commit one immutable local event, disclose only with consent, and invite continuation.

Every proposed package is traceable to Anky code, but several require rewriting because the reusable concept is currently interleaved with product behavior.

## TOHSENO implication

TOHSENO begins as a set of internal contracts and compatibility adapters inside a preserved Anky deployment. Its first public surface should be the manifest/validator, followed only by packages demonstrated by both Anky and a materially different ritual. The architecture must remain useful when identity, server reflection, payment, gating, sync, proof, and blockchain are all absent.

## Recommendation

Do not create this tree all at once. Begin with fixtures and internal interfaces inside Anky. Publish only `continuity.manifest` plus a small, tested domain/artifact surface after the second app demonstrates that the names and boundaries work for non-text/media actions. Keep deferred packages visibly deferred so the scaffold cannot imply unimplemented security or synchronization guarantees.
