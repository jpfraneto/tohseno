# Identity and cryptography

## Current implementation

### Status summary

| Question | Repository answer | Status |
|---|---|---|
| When is identity created? | On `loadOrCreate`; iOS calls it during `AppRoot.onAppear`, Android normally triggers it while configuring RevenueCat in the unlocked shell | Implemented |
| Private-key source | Deterministically derived from a generated/imported 12-word mnemonic | Implemented |
| Algorithm | BIP-39 English, 128-bit entropy, BIP-44 `m/44'/60'/0'/0/0`, secp256k1 EOA, EIP-55 address | Implemented |
| Chain/domain | Base mainnet/testnet chain IDs 8453/84532; EIP-712 domain `Anky`, version `1` | Implemented |
| Raw key storage | Raw key is derived in memory; the recovery phrase is the durable secret | Implemented |
| Phrase export/import | Biometric-gated reveal/import on both platforms | Implemented |
| Recovery | iOS optional iCloud phrase/data backup; Android manual phrase/ZIP and installation-local encrypted backup | Partial and platform-dependent |
| Traditional authentication | No username/password/email/session-cookie login found | Not implemented by design |
| Server account record | No single profile row; account-address-keyed ledger, quota, subscription, idempotency, event, and painting records arise through use | Implemented, distributed |
| Cross-platform key compatibility | Same declared derivation and identity fixtures | Implemented at algorithm/fixture level |
| Cross-device restoration | iOS opt-in cloud path; Android no automatic off-device path found | Partial |
| ERC-1271/smart account | Type/spec placeholder only | Documented/planned, not implemented |
| Identity bridge among apps | No consent-based bridge abstraction found | Not implemented |
| Exportable continuity proof | Request signatures are transient and not stored as artifact proofs | Not implemented |

### Creation and derivation

The identity contract is named `anky.base.eoa.v1`.

```text
128 random bits
  → BIP-39 English mnemonic + checksum (12 words)
  → BIP-39 seed, empty passphrase
  → BIP-32/BIP-44 m/44'/60'/0'/0/0
  → secp256k1 private/public key
  → last 20 bytes of Keccak-256 public key
  → EIP-55 checksum address
  → account ID (the checksum address)
```

Evidence:

- `protocol/identity/SPEC.md:3-24` — normative identity law.
- `protocol/implementations/typescript/src/identity.ts:5-83` — TypeScript mnemonic derivation and account ID.
- `apps/ios/Anky/Core/Identity/RecoveryPhrase.swift:31-99` — entropy, word encoding, checksum validation.
- `apps/ios/Anky/Core/Identity/WriterIdentity.swift:18-108` — HD derivation, account, signing/recovery.
- `apps/android/app/src/main/java/inc/anky/android/core/identity/RecoveryPhrase.kt:7-69`.
- `apps/android/app/src/main/java/inc/anky/android/core/identity/WriterIdentity.kt:9-73`.

The public “account ID” does not include chain ID, even though signing accepts only Base mainnet or testnet configuration. The same EOA address therefore identifies the practice across environments unless deployment data is otherwise separated.

### iOS secret storage and recovery

`WriterIdentityStore.loadOrCreate` checks the local phrase. If absent, it can adopt the synchronizable backup phrase; otherwise it generates a new one. The phrase, rather than the raw private key, is stored:

- local account: Keychain with `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`;
- optional recovery account: synchronizable Keychain with `kSecAttrAccessibleAfterFirstUnlock`;
- service: `lat.memetics.anky`.

The import path validates and stages the new phrase and preserves a previous phrase for rollback. `YouViewModel` places phrase reveal/import and cloud-backup changes behind biometric authentication. The independent iCloud data backup derives an AES-GCM key from the phrase using HKDF-SHA-256 and stores an encrypted ZIP envelope in iCloud Documents.

Evidence:

- `apps/ios/Anky/Core/Identity/WriterIdentityStore.swift:18-130`.
- `apps/ios/Anky/Core/Identity/KeychainClient.swift:27-43`.
- `apps/ios/Anky/Features/You/YouViewModel.swift:135-320`.
- `apps/ios/Anky/Core/Storage/ICloudBackupStore.swift:36-207`.

What happens after uninstall is **unclear** from source because Keychain persistence is OS-controlled. What the code guarantees is narrower: if the local item is absent at next launch, an opted-in synchronizable phrase may be adopted; otherwise a new identity is generated. Archive/reflection recovery also requires the encrypted document backup.

### Android secret storage and recovery

Android generates a random AES key in Android Keystore and AES-GCM encrypts the mnemonic into:

- `filesDir/Anky/identity.enc`;
- a corresponding IV file;
- a Keystore alias owned by the application.

The manifest sets `android:allowBackup="false"`, and both backup/data-extraction rule XML files exclude all domains. `backupToDeviceSecureStorage` only re-saves the same installation-local encrypted phrase; it does not create an off-device copy. Phrase import overwrites the current encrypted phrase after validation but has no equivalent staged previous-phrase rollback.

The Android encrypted data backup uses the same general HKDF/AES-GCM envelope shape as iOS, but writes it inside `filesDir`. A user can manually export/import archives and manually enter a phrase. No Google Drive, cloud key sync, or automatic restoration is called by the live application.

Evidence:

- `apps/android/app/src/main/java/inc/anky/android/core/identity/WriterIdentityStore.kt:12-90`.
- `apps/android/app/src/main/AndroidManifest.xml:35-43`.
- `apps/android/app/src/main/res/xml/backup_rules.xml`.
- `apps/android/app/src/main/res/xml/data_extraction_rules.xml`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/AndroidEncryptedBackupStore.kt:34-245`.
- `apps/android/app/src/main/java/inc/anky/android/feature/you/YouViewModel.kt:192-279,375-557`.

After app-data removal or movement to another device, the source provides no automatic way to recover the Keystore key or installation-local ciphertext. Recovery therefore depends on an earlier user-held phrase and separately exported data archive. This is **portable cryptography but partially implemented portability**.

### What is signed

The practice key authorizes backend requests. For reflection, clients SHA-256 the exact `.anky` request bytes and sign EIP-712 typed data containing:

- identity version;
- account address;
- method;
- path;
- body hash;
- request-time epoch milliseconds;
- client (`ios`, `android`, or `other`).

The same headers/signing helper are used for level sessions/status, painting preparation and asset fetches, ceremony state, funnel/emergency events, subscription identity, and iOS account deletion.

Evidence:

- `protocol/implementations/typescript/src/identity.ts:40-50,89-170`.
- `apps/ios/Anky/Core/Mirror/AnkyPostSigner.swift:21-75`.
- `apps/ios/Anky/Core/Level/LevelSyncClient.swift:48-200,224-233`.
- `apps/android/app/src/main/java/inc/anky/android/core/level/SignedLevelRequests.kt:13-25`.
- `apps/android/app/src/main/java/inc/anky/android/core/level/LevelSyncClient.kt:68-185`.

#### Endpoint-binding limitation

Although the typed-data schema contains method and path, native `AnkyPostSigner` implementations always insert `POST` and `/anky`. `LevelSyncClient` then uses those signed headers for actual GET, POST, and DELETE routes. Server verification accepts body bytes and reconstructs the fixed `AnkyMirrorRequest`; it is not given the actual route or method.

Therefore the current signature proves:

> this account signed this body hash, timestamp, and client under the Anky domain.

It does **not** prove:

> this account authorized this specific HTTP endpoint and method.

The in-memory replay cache rejects the same timestamp/signature pair after first use within one process, and TLS limits interception, but that is not equivalent to endpoint-bound least privilege. Empty-body signed operations deserve particular care. This must not be copied unchanged into TOHSENO.

Evidence:

- `apps/ios/Anky/Core/Mirror/AnkyPostSigner.swift:63-71` — constants in the digest.
- `apps/ios/Anky/Core/Level/LevelSyncClient.swift:197-233` — fixed signer used for `DELETE /account`.
- `backend/server.ts:924-985` — verification does not receive actual method/path.
- `backend/server.ts:991-1028` — process-memory freshness/replay guard.

### Server verification and account state

`verifyAnkyBaseRequest`:

1. accepts only configured Base chain IDs;
2. checks identity/signature type and known client;
3. parses the EIP-55 account;
4. hashes exact request body bytes;
5. recovers/verifies the EOA signature;
6. checks request freshness and a short in-memory replay set at the route wrapper.

The server never receives the mnemonic or private key. No bearer refresh token or server session is issued.

SQLite nevertheless acts as a pseudonymous account database. `session_ledger`, `level_state`, `painting_meta`, `generation_log`, `subscription_state`, idempotency, quota, and some event records are keyed directly or indirectly by account address/hash. RevenueCat uses the same address as its app-user identity. There is no central human profile, but there is a durable practice-account graph.

Evidence:

- `backend/server.ts:881-1028`.
- `backend/level/db.ts:64-184`.
- `backend/subscription/routes.ts`.
- `backend/events/routes.ts`.

`DELETE /account` is implemented and authenticated server-side. iOS calls it before local deletion. Android's `deleteAccountAndDataEverywhere` clears local data and logs out RevenueCat but has no delete method in its `LevelSyncClient`, so it never invokes the route.

Evidence:

- `backend/account/routes.ts:41-78`.
- `backend/level/db.ts:517-560`.
- `apps/ios/Anky/Features/You/YouViewModel.swift:323-354`.
- `apps/android/app/src/main/java/inc/anky/android/feature/you/YouViewModel.kt:688-738`.
- `apps/android/app/src/main/java/inc/anky/android/core/level/LevelSyncClient.kt:54-64`.

### Identity portability and data ownership

The mnemonic makes the key portable across iOS, Android, and TypeScript. It does not automatically make the application state portable:

| Scenario | iOS | Android |
|---|---|---|
| Reinstall, local secret still available | OS-dependent; local Keychain path can load it | OS-dependent, but app-data/Keystore loss makes ciphertext unusable |
| Reinstall, local secret absent | Adopt opted-in sync phrase or generate new | Generate new unless user manually imports |
| New device | Optional sync phrase + encrypted iCloud data restore | Manual phrase + separately exported ZIP |
| Import phrase into existing install | New account; old phrase staged; existing archive remains | New account; existing archive remains; no old-phrase staging |
| Restore content without matching phrase | Possible through archive import | Possible through archive import |

No archive metadata binds a `.anky` to the identity that created it. Importing a different phrase while retaining existing local artifacts and reflections can therefore mix one device's content with another server/RevenueCat account. The server ledger and subscription do not migrate. No consent/migration transaction resolves that split.

### Privacy and linkability

Positive properties:

- identity exists without collecting email, phone, name, or password;
- private material stays client-side;
- raw writing is not required for level-ledger synchronization;
- biometric gates protect phrase UX;
- signed requests prevent simple address spoofing.

Risks:

- the stable EVM address links reflection, ledger, paintings, events, RevenueCat, and any future public-chain use;
- account hashes in logs remain stable pseudonyms;
- the same derivation path and address across continuity apps would create cross-app linkability if generalized naively;
- EVM public use can associate the practice address with on-chain payments/ownership;
- phrase reveal is highly portable and therefore exfiltratable once the biometric gate is passed;
- server/provider metadata still includes network and request timing;
- deletion does not remove generated painting files, and future RevenueCat webhooks may recreate subscription rows;
- request signing proves key control, not honest timing, human authorship, or device observation.

Evidence:

- `backend/server.ts:2292-2322` — hashed operational logging.
- `backend/painting/config.ts:84-110` — address-derived asset directory.
- `smart_contracts/src/ANKY_MIRRORS.sol:49-430` — optional public address/hash relationships.
- `backend/level/routes.ts:92-166` — client-asserted continuity records.

## Interpretation

The implementation is best described as an **app-scoped, deterministic, portable practice key with uneven data recovery**, rather than a device-bound key or a complete human identity. It is installation-created, app-contextual by convention, and portable through a mnemonic. The server treats its address as an account.

It correctly avoids profile-first identity. It does not yet model:

- distinct identities per continuity app;
- explicit consent-based relationships among identities;
- identity rotation with content/ledger ownership migration;
- multiple devices under one practice identity;
- scoped signing capabilities;
- stored/exportable continuity proofs;
- non-EVM signing suites.

The future ERC-1271 type in `backend/server.ts:898-907` and `protocol/identity/SPEC.md:79-81` is a placeholder; no verifier or client implementation is wired.

## TOHSENO implication

A generic package should separate four concerns that Anky currently bundles:

```ts
type PracticeIdentityId = string;

interface PracticeIdentity {
  readonly id: PracticeIdentityId;
  readonly suite: string;
  sign(statement: Uint8Array): Promise<Signature>;
}

interface IdentitySuite {
  createRecoveryMaterial(): Promise<RecoveryMaterial>;
  derive(material: RecoveryMaterial): Promise<PracticeIdentity>;
  verify(statement: Uint8Array, signature: Signature): Promise<boolean>;
}

interface SecretStore {
  load(alias: string): Promise<Uint8Array | null>;
  store(alias: string, secret: Uint8Array, policy: AccessPolicy): Promise<void>;
  remove(alias: string): Promise<void>;
}

interface RecoveryCoordinator {
  assess(): Promise<RecoveryState>;
  exportWithConsent(): Promise<RecoveryPackage>;
  importAndPlanMigration(input: RecoveryPackage): Promise<MigrationPlan>;
}
```

The public API should not expose `privateKey`, mandate a phrase, equate an address with a human, or mandate Base. An internal `base-eoa-v1` adapter can originate from Anky's current implementation while its compatibility fixtures remain unchanged.

Signed statements must include a purpose/audience, actual method/path or operation name, body digest, nonce/idempotency identifier, issued/expiry times, app/practice scope, and signature-suite version. A durable server nonce store or operation-specific idempotency should replace reliance on a process-memory replay set where security requires it.

## Recommendation

1. Keep `anky.base.eoa.v1` frozen as a compatibility adapter; do not silently rename its domain or derivation.
2. Add endpoint-bound signature fixtures and version them before changing production authorization.
3. Define identity import as a migration plan: identify old/new account, content ownership, subscription behavior, backup compatibility, and rollback.
4. Do not enable cross-app key reuse by default. A bridge should be a separate, signed, consented relation revealing only chosen identifiers.
5. Make server deletion enumerate databases, generated files, provider-side identifiers, webhook recreation behavior, and local/cloud backups.
6. Decide the framework's local-encryption threat model before promising that a “secure app context” encrypts raw content; Anky currently relies on platform sandbox encryption for live content.

