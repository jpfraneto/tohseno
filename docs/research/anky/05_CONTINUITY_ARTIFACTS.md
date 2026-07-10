# Continuity artifacts

## Current implementation

### `.anky` wire format

The format is newline-delimited UTF-8:

```text
<epoch-milliseconds> <first-glyph>
<delta-milliseconds> <next-glyph-or-SPACE>
<delta-milliseconds> <next-glyph-or-SPACE>
...
<terminal-silence-milliseconds>        # optional bare integer line
```

Rules established by `protocol/SPEC.md`:

- the first numeric field is an absolute start time;
- later numeric fields are elapsed deltas;
- `SPACE` reconstructs to U+0020;
- a terminal bare integer marks ended inactivity;
- completion is active duration at least 480,000 ms;
- exact UTF-8 bytes are SHA-256 hashed;
- complete and fragment fixtures are committed.

The format has no magic bytes or version, content type, app ID, action-policy ID, explicit outcome, event ID, creator public key, signature, end timestamp, timezone, reflection link, or encryption envelope.

Evidence:

- `protocol/SPEC.md:1-57`.
- `protocol/fixtures/valid-complete.anky`.
- `protocol/fixtures/valid-fragment.anky`.
- `protocol/expected/valid-complete.json`.
- `protocol/expected/valid-fragment.json`.

### Parsers, serializers, and validators

There are three independent implementations:

| Implementation | Location | Role |
|---|---|---|
| TypeScript | `protocol/implementations/typescript/src` | Backend/reference parser, reconstruction, duration, hash, identity fixtures |
| Swift | `apps/ios/Anky/Core/Protocol` | iOS runtime writer/parser/validator/reconstructor/hasher |
| Kotlin | `apps/android/app/src/main/java/inc/anky/android/core/protocol` | Android runtime equivalent |

All recognize the first timestamp, timed glyph lines, `SPACE`, reconstruction, duration, and SHA-256. Cross-platform compatibility is intended and partially fixture-tested, but important differences exist:

- protocol/TypeScript terminal duration: 1,000–8,000 ms;
- Swift parser: any positive bare integer can be a terminal marker;
- Kotlin parser: only exactly `8000` is terminal;
- Swift and Kotlin suffix-rewrite timing behavior differs;
- iOS can append the selected terminal marker before reflection; Android sends an unterminated archived fragment/complete artifact as-is.

Evidence:

- `protocol/implementations/typescript/src/parse.ts`.
- `apps/ios/Anky/Core/Protocol/AnkyDuration.swift:3-36`.
- `apps/ios/Anky/Core/Protocol/AnkyParser.swift`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyParser.kt:3-35`.
- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:431-458`.

### Naming, persistence, and indexing

The canonical local filename is `<sha256-of-exact-bytes>.anky`.

| Data | iOS | Android |
|---|---|---|
| Active draft | `Documents/ActiveDrafts/dotAnky.anky` | `filesDir/ActiveDrafts/dotAnky.anky` |
| Archive | `Documents/Ankys/<hash>.anky` | `filesDir/Ankys/<hash>.anky` |
| Reflection | `Application Support/Anky/reflections/<hash>.json` | `filesDir/Anky/reflections/<hash>.json` |
| Session index | `Application Support/Anky/session-index.json` | app-private Anky index JSON |
| Encrypted private backup | iCloud Documents envelope, opt-in | `filesDir` envelope, manually exportable |

iOS archive/draft writes use atomic replacement. Android uses direct file writes. Both indexes are derived convenience data and can be rebuilt from archive/reflection stores. The iOS index contains previews and absolute local URLs, so it is neither a portable artifact nor the source of truth.

Evidence:

- `apps/ios/Anky/Core/Storage/ActiveDraftStore.swift:13-60`.
- `apps/ios/Anky/Core/Storage/LocalAnkyArchive.swift:32-102`.
- `apps/ios/Anky/Core/Storage/SessionIndexStore.swift:187-235`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/ActiveDraftStore.kt:7-40`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/LocalAnkyArchive.kt:12-29`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/SessionIndexStore.kt`.

### Hashing and tamper detection

SHA-256 provides a deterministic digest of exact bytes. It supports:

- content-addressed filenames;
- duplicate recognition;
- reflection lookup;
- level-ledger idempotency/reference;
- integrity checks when a caller recomputes and compares.

It does not by itself authenticate the creator or prove time/duration. The archive `load(hash:)` paths select the requested filename and parse its content, but no explicit recomputed-hash equality check was found at that boundary. A file altered in place can therefore be returned as parsed content with an internal/new digest that differs from the requested filename, depending on the caller.

There is also no Merkle log, signed manifest, monotonic counter, trusted timestamp, device attestation, or server receipt stored beside a local artifact.

Evidence:

- `apps/ios/Anky/Core/Protocol/AnkyHasher.swift`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyHasher.kt`.
- `apps/ios/Anky/Core/Storage/LocalAnkyArchive.swift:88-102`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/LocalAnkyArchive.kt:20-29`.

### Reflection changes artifact identity on iOS

The most consequential compatibility behavior is in `RevealViewModel`: before reflection, iOS may append the configured terminal silence, save the resulting bytes as a new hash, remove the prior archive, and update the index. The reflection then links to the new hash.

Android's post-seal path sends the already archived bytes. This means:

- an iOS artifact's canonical identity can depend on whether reflection was requested;
- equivalent writing can have different hashes across platforms;
- a server ledger entry created at seal can refer to a pre-terminal hash while reflection refers to a post-terminal hash;
- external links/backups created before reflection can become stale;
- hash is not a stable event identifier.

Evidence:

- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:431-458`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:684-735`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/PostSessionSealingScreen.kt:90-94`.

### Import and export

#### Individual artifacts

iOS `LocalAnkyArchive.importArtifact` and Android `SingleAnkyImporter` normalize common pasted/fenced inputs, parse, validate, and generally require an eight-minute complete artifact for the individual import flow. Export/share paths can expose the raw `.anky`.

Normalization can alter exact bytes and therefore the hash. This is acceptable only if the import is explicitly a new canonicalization operation; it is incompatible with treating an externally supplied hash/signature as preserved proof.

Evidence:

- `apps/ios/Anky/Core/Storage/LocalAnkyArchive.swift:71-81,191-330`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/SingleAnkyImporter.kt`.

#### Backup ZIP

Both platforms export a version-one ZIP containing a manifest, `.anky` files, and reflection representations. Importers parse artifacts and re-save them through local stores. They can accept fragments in backup paths even where standalone import requires completeness.

Observed limitations:

- no signed backup manifest;
- no strong manifest-version/count enforcement found;
- CRLF/spacing normalization can change bytes/hash;
- filename-to-byte hash is not consistently checked before re-save;
- standalone reflection JSON can be imported without proving a matching artifact, creating orphan/spoofable derived metadata;
- platform interoperability is intended by similar envelope/layout code but no cross-platform golden encrypted-backup test was found.

The iOS encrypted backup uses phrase-derived HKDF-SHA-256 and AES-GCM with an `anky.private.icloud.backup.v1` context. Android uses the corresponding algorithm/context but stores the encrypted result locally. Cryptographic shape similarity is evidence of intent, not proof that every ZIP/JSON/date/nonce detail interoperates.

Evidence:

- `apps/ios/Anky/Core/Storage/BackupImporter.swift`.
- `apps/ios/Anky/Core/Storage/BackupImporter.swift:227-305` — `BackupExporter`.
- `apps/ios/Anky/Core/Storage/ICloudBackupStore.swift:36-207`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/BackupImporter.kt`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/Exporter.kt`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/AndroidEncryptedBackupStore.kt:34-245`.

### Tests

Existing coverage includes:

- valid complete and fragment fixtures;
- invalid empty/malformed fixtures;
- parse/reconstruct/duration/hash behavior;
- native writer and storage tests;
- backup/export/import tests in native suites;
- signing fixture tests linked to body hashes.

The missing compatibility suite is more important than raw test count:

- one corpus run unmodified in TypeScript, Swift, and Kotlin;
- terminal values 1,000 through 8,000;
- complex Unicode grapheme clusters and normalization;
- frozen resume plus suffix replacement;
- pre/post-reflection hash identity;
- ZIP encrypted-envelope cross-platform round trip;
- tampered filename/content rejection;
- reflection/archive referential integrity.

Evidence:

- `protocol/implementations/typescript/test/protocol.test.ts`.
- `apps/ios/Anky/Tests`.
- `apps/android/app/src/test/java/inc/anky/android/protocol/ProtocolFixtureTest.kt`.
- `apps/android/app/src/test/java/inc/anky/android/storage/StorageTest.kt`.

### Privacy characteristics

`.anky` is compact but not privacy-preserving:

- it contains the complete raw writing;
- it contains a precise epoch start and per-input cadence;
- cadence can be behavioral metadata even if the words are redacted;
- it is plaintext in live local stores;
- a share/export discloses both text and timing;
- the digest is stable and linkable anywhere it is reused;
- adding a signature would authenticate/link it but would not hide it.

Only the opt-in encrypted backup envelope encrypts the collection at application level. Normal iOS/Android platform data protection and sandboxing may encrypt storage at the OS/device layer, but that is not a TOHSENO-defined encrypted context.

## Four distinct concepts

### `ContinuityEvent`

An immutable local domain fact that an action ended:

```ts
interface ContinuityEvent {
  eventId: string;
  appId: string;
  manifestVersion: string;
  actionPolicyId: string;
  startedAt: string;
  endedAt: string;
  activeDurationMs?: number;
  outcome: "completed" | "interrupted";
  sealReason: string;
  artifact: ArtifactRef;
  createdBy?: PracticeIdentityRef;
}
```

It answers *what lifecycle transition happened*. A `.anky` only partially supplies start/duration and inferred completeness. It lacks stable event ID, policy, explicit outcome/end/seal reason, and identity.

### `ContinuityArtifact`

Opaque private payload produced by the action:

```ts
interface ContinuityArtifact {
  mediaType: string;
  codec: string;
  bytes: Uint8Array;
  digest: { algorithm: "sha256"; value: string };
}
```

`.anky` maps strongly here: it is an Anky-specific codec and payload with exact digest semantics. TOHSENO should not require all artifacts to be text; Gratitude Lock may produce structured JSON plus audio/photo objects.

### `ContinuityProof`

An opt-in, minimal, verifiable statement:

```ts
interface ContinuityProof {
  proofVersion: string;
  subject: PracticeIdentityRef;
  statement: {
    eventId: string;
    appId: string;
    policyId: string;
    outcome: "completed";
    artifactDigest?: string;
  };
  signature: Signature;
  witness?: ServerReceipt;
}
```

Current `.anky` has no proof. SHA-256 is a digest, not authentication. The EIP-712 HTTP signature is transient authorization, not stored/exportable proof. The level route stores self-reported seconds/hash after verifying the key; it is at most a client attestation, not independent evidence of human action or trustworthy duration.

### `ContinuityReflection`

Derived feedback with provenance and a link to stable input:

```ts
interface ContinuityReflection<Output = unknown> {
  reflectionId: string;
  eventId: string;
  inputArtifactDigest: string;
  provider: string;
  policyVersion: string;
  createdAt: string;
  output: Output;
}
```

Anky already stores reflection separately, which is the correct direction. Current JSON has artifact hash, title/body/tags/timestamp but lacks provider, prompt/policy version, model, reflection ID, consent/disclosure record, and robust referential validation.

### Mapping summary

| Existing object | Event | Artifact | Proof | Reflection |
|---|---|---|---|---|
| `.anky` | Partial inferred facts | **Strong match** | No | No |
| SHA-256 filename | Possible artifact reference | Digest | No creator/auth proof | Lookup key |
| Signed `/anky` request | One network action | Binds body digest | Ephemeral key-control attestation only | Requests derivation |
| Level ledger row | Server continuity claim | Stores hash, not bytes | Client-asserted/server-accepted receipt-like data | No |
| Reflection JSON | Links by hash | No | No | **Partial match** |
| Backup ZIP | Collection/container | Contains artifacts | Unsigned | Contains reflections |

## Interpretation

The current storage model uses one hash to serve too many roles: filename, content identity, reflection foreign key, idempotency key, ledger reference, and user-facing continuity identity. The iOS terminalization rewrite proves those roles cannot safely remain identical.

The `.anky` codec is valuable and should be preserved. It should become an artifact codec under an immutable event envelope, not be enlarged into a universal continuity file containing identity, proofs, reflection, and synchronization metadata.

## TOHSENO implication

Version one needs:

1. opaque artifact bytes with a codec/media type and digest;
2. a separately versioned event envelope;
3. stable event IDs independent of later artifact normalization or derivation;
4. explicit outcome and policy identifiers;
5. referential checks for reflections;
6. proof as an optional export, not automatic publication;
7. migration/read support for existing `.anky` bytes.

At-rest encryption belongs in the repository/storage adapter, not inside every codec. Large media should use blob references rather than forcing event records to embed bytes.

## Recommendation

1. Freeze `.anky v0` exactly and document the three parser interpretations.
2. Decide canonical terminal-marker and hash behavior before writing a v1 codec.
3. Never mutate an archived artifact in place or replace its identity to request a reflection; derive a new artifact with an explicit relation if normalization is needed.
4. Add filename/content digest checks and atomic writes on Android.
5. Validate backup manifest version/count/digests and reflection references; preserve bytes when importing signed/digested artifacts.
6. Add an event envelope through a sidecar/local database first, leaving existing `.anky` filenames readable.
7. Do not call a digest or EOA request signature a trustworthy proof without defining the exact claim and adversary model.
