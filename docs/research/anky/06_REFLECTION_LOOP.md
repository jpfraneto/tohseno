# The reflection loop

## Current implementation

### End-to-end data flow

```text
local .anky bytes
  → optional iOS terminalization/new hash
  → SHA-256 + EIP-712 request signature
  → POST /anky (text/plain; optional SSE)
  → freshness/replay/body-size/account verification
  → protocol validation + session tier
  → quota + idempotency + RevenueCat entitlement
  → reconstruct full raw writing in memory
  → append writing to tier-specific prompt
  → route to a ZDR-declared model provider
  → parse title/tags/Markdown
  → stream/return text
  → save reflection JSON locally under artifact hash
  → show result and later index it
```

Evidence:

- `apps/ios/Anky/Core/Mirror/MirrorClient.swift:125-136`.
- `apps/android/app/src/main/java/inc/anky/android/core/mirror/MirrorClient.kt:26-42`.
- `backend/server.ts:2002-2324`.
- `backend/reflection.ts:107-204`.

### Trigger and eligibility

iOS and Android currently have different immediate loops:

- iOS prepares a local sealed-session state, then asks the person to choose. `beginReflectionRequest` runs only after tapping “Read.” A free/non-entitled person is routed to the paywall.
- Android's `PostSessionSealingScreen` invokes `beginSealedSessionReflection` on appearance. An entitled person therefore sends the session automatically; a non-entitled person sees the paywall/veil.

The older Reveal screen has separate eligibility assumptions. Android's Reveal path generally expects a complete artifact, while the immediate post-seal path can ask for a reflection on a fragment. Backend tiering explicitly supports fragments, including a one-sentence response below 88 seconds.

Evidence:

- `apps/ios/Anky/AppRoot.swift:1778-2025`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/PostSessionSealingScreen.kt:90-94`.
- `apps/android/app/src/main/java/inc/anky/android/feature/reveal/RevealViewModel.kt`.
- `protocol/implementations/typescript/src/session.ts:4-35`.

This is both a product divergence and a privacy divergence: one platform requires consent at the disclosure boundary; the other treats entitlement plus screen appearance as consent.

### Request shape and authentication

The request body is exact `.anky` UTF-8 with `Content-Type: text/plain`. Headers carry identity version, EIP-55 account, signature type, EIP-712 signature, request time, client, optional app version, request intent, and streaming preference.

The server:

1. enforces a 1 MiB body limit;
2. requires a fresh timestamp within five minutes;
3. rejects an already-seen timestamp/signature in process memory;
4. hashes exact bytes;
5. verifies the Base EOA signature;
6. applies per-account burst and durable daily quota (24 for the route call shown);
7. validates `.anky`;
8. computes the tier;
9. acquires account+artifact idempotency;
10. requires current RevenueCat entitlement unless retrying a previously succeeded reflection.

The signature binds body/account/time/client under fixed `POST /anky` typed data, but, as documented in `03_IDENTITY_AND_CRYPTOGRAPHY.md`, the shared authorization helper does not bind actual non-reflection routes.

Evidence:

- `backend/server.ts:241-258,924-1028,2002-2205`.
- `backend/security.ts:9-49`.
- `backend/subscription/store.ts`.

`INCOMPLETE_RITUAL` exists in the backend error vocabulary, but the reflection route does not enforce eight-minute completion. Instead, all valid durations receive `sentence`, `dip`, or `full` treatment. This is an implemented tier system, not an incomplete-session prohibition.

Evidence:

- `backend/server.ts:614-646,2129-2137`.
- `protocol/implementations/typescript/src/session.ts:4-35`.

### Prompt construction

The runtime prompt is code, not the Markdown file under `backend/prompts`:

- under 88 seconds: exactly one responsive sentence;
- 88 seconds to under 480 seconds: one short paragraph;
- at least 480 seconds: a longer personal mirror in Markdown.

`buildReflectPrompt` appends a separator and the entire reconstructed writing. A separate nudge intent builds a progress-sensitive nudge prompt from the current fragment. `backend/prompts/reflect-current.md` participates in evaluation/documentation workflows but is not the primary route's loaded runtime template.

Evidence:

- `backend/reflection.ts:107-204`.
- `backend/server.ts:2208-2240`.
- `backend/prompts/reflect-current.md`.
- `backend/test/promptEval.test.ts`.

Anky's voice, duration tiers, same-language rules, therapy boundaries, heading requirements, and nudge behavior are product policy. The reusable seam is prompt preparation/provenance plus provider invocation, not these prompt strings.

### Provider routing and dependencies

The server already defines a useful internal abstraction:

```ts
type ReflectionProvider = {
  name: string;
  privacy: ProviderPrivacy;
  reflect(input: {
    env: Env;
    prompt: string;
    tier?: SessionTier;
    fetchImpl?: ProviderFetch;
    onChunk?: AnkyReflectionChunkSink;
  }): Promise<ReflectionProviderResult>;
};
```

`routeReflection` walks configured providers, skips any that fail the required zero-data-retention capability, and returns the first success. Production order is OpenRouter, Bankr, Poiesis, then a local default fallback. OpenRouter requests deny data collection and request ZDR. Bankr/Poiesis are disabled unless environment configuration confirms their policies.

Default model configuration at the study snapshot:

| Tier | Provider model configuration | Limit |
|---|---|---|
| sentence | `google/gemini-2.5-flash-lite` through OpenRouter | 60 tokens |
| dip | `google/gemini-2.5-flash-lite` through OpenRouter | 250 tokens |
| full | `anthropic/claude-sonnet-4.6` through OpenRouter | provider/default |

These names are current deployment constants and external dependencies, not proposed TOHSENO defaults.

Evidence:

- `backend/server.ts:225-258`.
- `backend/server.ts:1272-1403`.
- `backend/server.ts:1426-1539`.

The provider privacy flags are application assertions/configuration gates. They are useful enforcement metadata but not cryptographic guarantees about an external service's actual retention. Contracts, provider settings, and operational verification remain part of the privacy boundary.

### Response schema and parsing

Provider output is normalized to:

- `title`;
- `reflection` Markdown;
- `tags`;
- internal `provider` and `chargeable` fields.

The HTTP route returns plain text Markdown with `X-Anky-Hash`, intent, and serialized tag headers, or streams progress/chunks over SSE. It does not return a versioned JSON reflection envelope. `parseMirrorResponse` accepts optional tag JSON on the first line and derives/normalizes title/body.

Evidence:

- `backend/server.ts:1278-1281,1709-1740,2253-2272`.
- `backend/test/parseMirrorResponse.test.ts`.

The default fallback returns HTTP 200 with a “mirror unavailable” Markdown message and `chargeable: false`. iOS recognizes this fallback and refuses to persist it. Android lacks the equivalent guard, so it can store a service-unavailable message as the session's reflection.

Evidence:

- `backend/server.ts:1709-1725`.
- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:336-356`.
- `apps/android/app/src/main/java/inc/anky/android/feature/reveal/RevealViewModel.kt`.

### Retries, idempotency, and failures

Clients persist pending reflection requests and retry temporary failures at roughly three-second intervals for up to about 120 seconds. They surface authentication, entitlement, duplicate-in-progress, rate, invalid artifact, body-size, provider, and transport failures.

Android's SSE parser maps a streamed error through a generic exception and loses the structured code in one path. Some entitlement behavior is consequently inferred from status/message rather than a stable machine response.

Server idempotency stores status, not the successful response payload. A retry of a succeeded artifact is allowed to call the model again and bypass a now-missing entitlement. This avoids charging a person twice for a lost response but means:

- provider output may differ on retry;
- external disclosure happens again;
- the stored local reflection is not reproducible;
- “idempotent” means billing/workflow admission, not result identity.

Evidence:

- `apps/ios/Anky/Core/Storage/ReflectionRequestStore.swift`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/ReflectionRequestStore.kt`.
- `apps/android/app/src/main/java/inc/anky/android/core/mirror/MirrorClient.kt:125-127,192-219`.
- `backend/server.ts:2147-2198,2262-2279`.

### Local persistence

The locally stored reflection is JSON keyed by artifact hash:

- iOS fields: hash, title, body/reflection, tags, creation time;
- Android equivalent includes a legacy `creditsRemaining` field.

Files are plaintext in the app sandbox. They are included in explicit backup/export flows. Deleting a local reflection removes it from the device/index but cannot retract text already processed by a model provider; the backend says it does not retain raw input/output.

Evidence:

- `apps/ios/Anky/Core/Storage/ReflectionStore.swift:44-114`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/ReflectionStore.kt:62-79`.
- `apps/ios/Anky/Core/Storage/BackupImporter.swift`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/BackupImporter.kt`.

### Usage limits and purchase checks

The app can write and archive without a subscription. The backend reflection route requires an entitled account, subject to:

- RevenueCat `pro` entitlement or a promotional grant;
- IP/account burst rate limits;
- durable daily quotas;
- artifact/account idempotency.

The clients also consult local/cached entitlement state to decide whether to send or display a paywall. Backend enforcement remains authoritative. The old credit-oriented models and some comments coexist with subscription-oriented code; the live route checks `AccountEntitlement`.

Evidence:

- `backend/server.ts:206-258,2112-2198`.
- `backend/subscription/revenuecat.ts`.
- `backend/subscription/store.ts`.
- `apps/ios/Anky/Purchases`.
- `apps/android/app/src/main/java/inc/anky/android/core/subscription`.

### Privacy boundary

The backend covenant in `server.ts:599-612` is implemented structurally in the reflection route:

- raw `.anky`, reconstructed writing, prompt, and provider output stay in request memory;
- logging records request ID, stable hashed address, artifact hash, duration, tier, provider, failure, status, version, and latency—not writing;
- SQLite idempotency/quota/ledger records use address/hash metadata;
- reflection bodies remain client-side after response.

Limitations:

- full writing is disclosed to the Anky backend and an external provider;
- metadata is stable and linkable;
- the server cannot cryptographically prove provider deletion;
- successful retry repeats disclosure;
- local reflection/artifact files are not application-level encrypted;
- account deletion does not delete external-provider history (the design relies on ZDR) or generated painting files;
- Android auto-request reduces the clarity of consent.

Evidence:

- `backend/server.ts:599-612,2292-2322`.
- `backend/level/db.ts:64-184`.
- `backend/account/routes.ts:41-78`.

### The painting loop is a second reflection pipeline

The journey/painting subsystem also reflects accumulated writing:

1. native clients reconstruct archive text since a level boundary;
2. signed `POST /level/prepare` sends it as JSON;
3. levels 1–8 use static/default assets and do not need generation text;
4. later levels require entitlement and recent ledger evidence;
5. a provider distills raw writing into a symbolic scene/title/palette;
6. image providers generate assets;
7. SQLite stores metadata and files persist below the account directory.

Raw writing is intended to exist only during the request, but distilled scenes and image files are durable. Account-row deletion does not remove the directory. This path must be included in privacy/deletion claims even though it is not called “reflection” in the UI.

Evidence:

- `apps/ios/Anky/Core/Level/LevelPaintingCoordinator.swift:176-200`.
- `backend/painting/routes.ts:191-240`.
- `backend/painting/distill.ts`.
- `backend/painting/pipeline.ts:142-177`.
- `backend/painting/config.ts:84-110`.

## Interpretation

Anky has two valuable separations already:

- raw artifacts remain local until a specific network feature is requested;
- provider routing has an explicit privacy capability and injectable interface.

The boundaries are still blurred:

- route orchestration, auth, quota, entitlement, prompt, model selection, parsing, logging, and transport live in a very large `server.ts`;
- provider input is a constructed prompt rather than typed private content plus a policy;
- response has no schema/provenance version;
- reflection is linked to a mutable artifact hash;
- client consent/trigger differs by platform;
- paintings are a second private-data derivation path with different persistence.

## TOHSENO implication

The repository suggests a slightly richer split than only `ReflectionProvider<Input, Output>`:

```ts
interface ReflectionPolicy<Input, Prompt> {
  readonly id: string;
  readonly version: string;
  prepare(input: Input, context: ReflectionContext): Prompt;
}

interface ReflectionProvider<Prompt, Output> {
  readonly id: string;
  readonly capabilities: {
    streaming: boolean;
    zeroDataRetention: "verified" | "declared" | "none";
    contentLogging: "disabled" | "unknown";
    trainingUse: "disabled" | "unknown";
  };
  reflect(prompt: Prompt, context: ProviderContext): Promise<Output>;
}

interface ReflectionRepository<Output> {
  get(eventId: string, policyVersion: string): Promise<StoredReflection<Output> | null>;
  save(reflection: StoredReflection<Output>): Promise<void>;
}
```

Authentication, entitlement, quota, consent receipt, transport, and provider selection should be composable middleware/capabilities around those interfaces. The stored result should include event ID, input digest, policy/prompt version, provider ID/model disclosure, time, and output schema version.

TOHSENO must support:

- local deterministic reflection with no network;
- server-hosted first-party logic;
- external provider with explicit disclosure;
- no reflection at all;
- opt-in versus automatic only when the manifest and privacy model allow it.

## Recommendation

1. Adopt iOS's explicit post-seal consent as the safe manifest default; record Android's current auto-request as a conflict to resolve deliberately.
2. Cache a successful response envelope if retries are intended to be idempotent; otherwise name the operation “retryable” and record derivation IDs.
3. Replace plain Markdown transport with a versioned result envelope while preserving legacy clients during migration.
4. Persist provider/policy/model/privacy-capability provenance locally without logging raw content.
5. Move Anky prompts and duration tiers into the Anky reference policy, not the generic provider package.
6. Treat painting distillation as a separate derived-artifact provider governed by the same consent, retention, and deletion inventory.
7. Add end-to-end deletion tests for SQLite rows, per-account files, backups, and webhook recreation.
8. Never promise “private reflection” without naming that raw content crosses to the monorepo server and configured model provider.
