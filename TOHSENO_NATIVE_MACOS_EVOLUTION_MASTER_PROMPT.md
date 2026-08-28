# MASTER IMPLEMENTATION PROMPT — EVOLVE TOHSENO INTO A NATIVE macOS APP FACTORY

You are working inside the existing TOHSENO repository. Your mission is to evolve TOHSENO from a CLI-first product with a browser Studio into a beautiful, native, nontechnical macOS application while preserving the mature local factory, protocol, privacy, ownership, build, signing, installation, and recovery machinery already present.

This is an implementation task, not a speculative redesign. Inspect the repository, respect its authority hierarchy, write the necessary ADRs before changing governed behavior, implement the work in coherent phases, run the relevant verification matrix, and leave the repository in a truthful state. Do not claim an external deployment, notarization, Stripe activation, Bankr funding, physical-device result, or release unless the corresponding real evidence exists.

The product promise is:

> A person who has never programmed can download TOHSENO on their Mac, describe a small app they wish existed, and find that native app running on their iPhone—without opening Terminal or understanding Xcode.

The canonical product law remains:

> **App → Intent → App on your iPhone.**

## 1. Read authority before acting

Read these completely before editing:

1. `AGENTS.md`
2. `protocol/SPECIFICATION.md`
3. `protocol/CONFORMANCE.md`
4. `docs/STATE.md`
5. `docs/ARCHITECTURE.md`
6. `docs/GOLDEN_PATH.md`
7. Every accepted ADR referenced by `AGENTS.md`, especially ADRs 0015, 0016, 0019, 0020, 0021, 0022, 0023, and 0024
8. `README.md`
9. `studio/README.md`
10. `companion/apple/TohsenoCompanion/README.md`
11. `billing/README.md` and `docs/runbooks/BILLING_1_0_0.md`
12. The actual Rust application, execution, service, harness, device, signing, billing, snapshot, and presentation implementations
13. The current website billing and relay implementations
14. `BANKR_DEPLOYMENT_CHECKLIST.md`, treating obsolete file references as historical rather than current authority

`protocol/` remains normative. Do not rewrite historical protocol records or silently migrate existing Shots. The native product shell must project the existing factory; it must not create a second factory, execution engine, Shot implementation, signing path, or persistence authority.

Before implementation, add an accepted ADR that defines this native macOS product transition and explicitly supersedes the CLI/npm/browser/mandatory-Companion/qualification portions of earlier product ADRs while preserving their valid internal machinery. Update `AGENTS.md`, `docs/STATE.md`, `docs/ARCHITECTURE.md`, `README.md`, and the ADR index coherently.

## 2. Non-negotiable product decisions

Implement according to these decisions:

### 2.1 Native macOS is the primary product

- The normal product is a real native macOS application written in Swift and SwiftUI.
- Do not make the normal interface a `WKWebView` wrapper around the existing Studio.
- The app must feel like a first-party Mac application: native navigation, controls, sheets, menus, keyboard behavior, accessibility, window restoration, state restoration, drag and drop, file pickers, progress, alerts, and system materials where appropriate.
- The existing browser Studio may remain temporarily as an advanced/debug/support surface until native parity is proven, but it is no longer the public product or first-run door.
- The CLI remains supported as an advanced, automation, development, diagnostics, and recovery interface. It becomes hidden machinery, not something ordinary users must install or understand.

### 2.2 No Terminal, npm, Node, or Homebrew requirement for ordinary users

- Replace `npm i -g tohseno` as the consumer installation path with a signed and notarized macOS `.app` distributed in a `.dmg` from `tohseno.com`.
- Bundle the required TOHSENO native core/service executable and static product resources inside the application or its verified installer-owned support location.
- Do not require Node or Bun on the customer Mac for the local product.
- Preserve the dependency-free npm bootstrap only as a legacy/developer path until a later explicit deletion decision.
- Add release scripts and documentation for Developer ID signing, nested executable signing, hardened runtime, notarization, stapling, DMG creation, integrity manifests, rollback, and update verification. Never perform external notarization or publication without explicit owner authorization and credentials.

### 2.3 Local and bring-your-own intelligence remain first-class

TOHSENO must never force a person to buy managed inference if they already have useful intelligence available.

Support these execution sources:

1. Existing authenticated coding harnesses already recognized by the repository, including Codex, Claude Code, OpenCode, and other supported adapters.
2. A configurable custom executable harness that satisfies a strict declared adapter contract.
3. Local OpenAI-compatible model endpoints, including common local runtimes when available. Detection must be safe, bounded, opt-in where network permission is involved, and must never scan arbitrary ports or send private data without consent.
4. TOHSENO-managed inference through the Bankr LLM Gateway for people without a usable harness or for people who consciously choose it.

The default creation screen must not expose harnesses, model IDs, API routes, credentials, or token vocabulary. TOHSENO should automatically select the best usable configured path.

An **Advanced** disclosure must allow the owner to:

- Inspect detected harnesses and whether each is usable/authenticated.
- Select a harness for this app.
- Select an advertised model belonging to that harness.
- Configure an approved custom harness.
- Configure a local OpenAI-compatible endpoint and model.
- Select privacy mode where the provider supports it.
- See a projected cost range and a maximum authorized managed-compute amount.
- Restore automatic selection.

Persist the exact selected harness/model/route with the durable command, as the repository already does. Recovery must never silently substitute another selection.

For local inference, display managed inference cost as `$0 paid to TOHSENO`; do not claim electricity or hardware use is free. For subscription-backed harnesses, say that incremental provider cost is unknown or covered by the owner’s provider plan rather than falsely asserting zero. For API-backed and Bankr models, compute an honest estimate from current model pricing plus bounded expected input/output usage. Label estimates as estimates and show the actual recorded usage afterward.

### 2.4 The iOS Companion stays out of v0

- Preserve the Companion, its SDK, encrypted relay, pairing, capability model, and tests.
- Do not delete or weaken it.
- Remove it from mandatory macOS genesis, trial activation, app creation, and iPhone installation.
- Native macOS v0 must install generated apps directly onto the connected development iPhone without first installing the TOHSENO Companion.
- Hide Companion setup from the normal interface. It may appear only as an unavailable/future or explicitly experimental advanced feature if doing so does not confuse the normal path.
- Do not activate APNs, production relay changes, or new Companion distribution in this work.

### 2.5 Remove commercially hostile qualification

- Remove the requirement to achieve five successful days before being allowed to pay.
- Remove the behavior where an expired, unqualified person cannot purchase.
- Do not gate local/BYO harness execution behind a TOHSENO subscription or managed-compute balance.
- Existing apps, source, history, and local harness use remain owned by the user and continue to function.
- Managed inference is gated by managed creation balance, not by deleting or disabling locally funded capability.

Welcome managed-compute balance is intentionally personal during v0:

- If no usable local/BYO harness is available, explain that managed access is available.
- Offer a configurable “Message JP for welcome compute” action rather than pretending automatic free credits exist.
- The contact destination must be release configuration, not hard-coded into business logic.
- Add an authenticated operator/admin command or protected server operation that grants a small USD-denominated promotional balance to one exact installation with a required reason and append-only audit record.
- Promotional grants and paid balance must be distinguishable in the ledger.
- Never expose an unauthenticated public credit-grant endpoint.

### 2.6 Managed inference uses Bankr and may be funded by $TOHSENO launch fees

Use Bankr’s current documented LLM Gateway rather than inventing a provider protocol. At implementation time, verify the live official documentation. The known current interface is:

- Base URL: `https://llm.bankr.bot`
- OpenAI-compatible models and chat completions under `/v1`
- Anthropic-compatible messages under `/v1/messages`
- Live model discovery through `/v1/models`
- Usage and credit endpoints exposed by Bankr
- API-key authentication
- Per-request usage reporting
- Standard, ZDR, and private inference tiers where supported
- Funding through Bankr LLM credits, including configured token launch fees or wallet assets

Architect this safely:

- The operator Bankr key must exist only in the TOHSENO managed inference service’s secret manager.
- Never embed it in the Swift app, Rust binary, CLI, generated apps, repository, logs, process arguments, crash reports, or a local harness environment.
- Local managed jobs authenticate to a TOHSENO inference proxy with a short-lived, installation-bound, scope-limited credential.
- The TOHSENO proxy verifies balance, reservation, job identity, model allowlist, privacy tier, request size, rate limits, and authorized maximum before forwarding to Bankr.
- The proxy records provider request IDs, model, token usage, provider-reported cost when available, retail charge, privacy tier, and reconciliation status without storing source or prompts beyond the minimum explicitly documented retention policy.
- Do not expose a general-purpose Bankr proxy. Requests must be bound to admitted TOHSENO executions and their reserved budget.
- A model request cannot cause wallet/token transactions through this key. Use a Bankr key scoped only to the LLM Gateway with least privilege.
- A Bankr `402`, exhausted operator credit balance, rate limit, or gateway outage must become a recoverable managed-compute state, not app corruption and not a reason to retry indefinitely.
- Add an operator health surface/runbook showing Bankr credit availability, recent usage reconciliation, and whether $TOHSENO launch-fee funding is active. Do not claim the fees fund inference unless Bankr confirms that configuration in the real account.

If an existing coding harness can safely target the TOHSENO inference proxy, use a short-lived proxy credential and compatible base URL. If no suitable harness exists, create a narrow local managed harness adapter or adopt a pinned, license-compatible open-source runner after documenting its license, integrity, update, sandbox, and tool-permission model. Do not silently download and execute an unpinned coding agent.

### 2.7 Stripe becomes a real balance and checkout system

Keep the existing signed, installation-bound billing concepts where useful, but evolve billing from subscription-only entitlement into an append-only USD-denominated creation-balance ledger.

Stripe collects money. TOHSENO owns the authoritative internal balance ledger. Bankr is an upstream compute expense. Do not treat Stripe’s success redirect or the local Mac as balance authority.

## 3. Target architecture

The target must preserve one factory:

```text
Native SwiftUI Tohseno.app
        |
        | authenticated loopback API + event stream (v0)
        v
Persistent Rust Local Workspace Service
        |
        v
ShotApplicationService → Engine → build/sign/install/launch
        |
        +── local/BYO harness or local model
        |
        └── managed execution credential
                    |
                    v
            TOHSENO inference proxy
                    |
                    v
              Bankr LLM Gateway
```

For v0, SwiftUI may reuse the existing loopback JSON API and server-sent event stream. Do not rush into Swift/Rust FFI or XPC unless repository evidence demonstrates it is necessary. Strengthen native-client authentication so an unrelated local webpage or process cannot mutate the factory. Preserve loopback-only binding, anti-CSRF/origin defenses for browser Studio, and add a separate native-session mechanism with bounded tokens and explicit app/service identity.

The macOS application should be added under a clear path such as:

```text
macos/Tohseno/
  App/
  Features/Onboarding/
  Features/Library/
  Features/Creation/
  Features/AppDetail/
  Features/Billing/
  Features/Settings/
  FactoryClient/
  DesignSystem/
  Resources/
  Tests/
  UITests/
```

Reuse existing brand tokens and assets. If Swift Package Manager improves testability, keep the product feature/model layer in a package or framework and the `@main` target thin, following the successful Companion pattern.

## 4. Native macOS v0 interface

### 4.1 Product-quality requirements

- Minimum supported macOS must match what the existing core and Apple tooling can truthfully support; document the decision.
- Use SwiftUI unless a specific AppKit bridge is required for missing native behavior.
- Support keyboard navigation, VoiceOver labels, Dynamic Type where applicable, reduced motion, light/dark appearance if the brand supports both, and clear focus states.
- Do not imitate an iOS app stretched onto macOS.
- Do not expose internal terms such as Shot, Expression, Version, execution ID, harness route, lineage, capability, relay, or provisioning profile on the normal path.
- Technical truth remains available under Details and Advanced.

### 4.2 Window and navigation

Ship one primary window with:

- Sidebar or compact app rail containing generated apps and a Create App action.
- Main content showing creation, the selected app, or its current human state.
- Optional native preview region using the latest accepted first-screen capture. Do not pretend it is interactive if it is only an image.
- Settings window or native Settings scene.
- Standard About, Help, Check for Updates, Hide, Quit, and diagnostics commands.

### 4.3 First run without Companion

Replace the current mandatory Companion cable genesis with a generated-app readiness flow:

1. Welcome and explain the promise in one sentence.
2. Verify supported macOS.
3. Verify full Xcode availability.
4. If missing, open the official Mac App Store/Xcode destination and explain the large download honestly.
5. Verify required Xcode components and license state.
6. Ask the person to connect and unlock an iPhone.
7. Detect Trust status where Apple tooling permits.
8. Guide Developer Mode and wait for observable completion.
9. Detect an Apple development signing team.
10. If absent, open Xcode and show the exact native instructions for adding an Apple Account; never collect Apple credentials.
11. Build/sign/install a deterministic minimal readiness app or perform the least invasive real verification that proves the path.
12. Enter Your Apps.

Show one instruction and at most one primary action at a time. Advance from observed state, not self-attestation. Preserve useful existing `cable_genesis` probes and refactor their projection so Companion installation/pairing is no longer required.

### 4.4 Your Apps

Display:

- Real icon where available.
- User-facing name.
- One human status indicator.
- Current presentation headline.
- Create App.

Preserve existing apps and adopt existing `~/Desktop/Tohseno` and `~/.tohseno` state without rewriting identity or history. Retired apps remain recoverable through an advanced/archive view.

### 4.5 Create App

The normal surface contains:

- “What would make your life easier?”
- Large intention field.
- Optional name.
- Up to eight validated reference images using the repository’s existing limits and exact-byte preservation.
- Managed balance only when relevant.
- Honest estimated range only when the selected route has a metered incremental cost.
- Create App button.
- Collapsed Advanced disclosure.

No templates, model selector, harness selector, token display, or technical project settings appear until Advanced is opened.

### 4.6 Progress

Reuse `application/src/presentation.rs` as the authority for the six human states:

- Waiting
- Building
- Ready for phone
- Installing
- Installed
- Failed

Add truthful, privacy-safe activity such as understanding the request, creating the interface, checking the build, preparing for iPhone, and installing only when each phrase maps to a durable real phase. Do not generate theatrical progress.

Closing the window must not cancel admitted work. The persistent service continues and the app restores current state when reopened.

### 4.7 App detail and evolution

For one app show:

- Icon and display name.
- Latest accepted preview.
- Current human state.
- “What should change?” composer.
- Reference images.
- Cost estimate/cap when applicable.
- Evolve App.
- Open on iPhone when technically available.
- Open Source Folder.
- Details.
- Retire/remove with precise preservation language.

Evolve binds the exact accepted base automatically and preserves existing stale-base refusal. Do not reintroduce a separate Feedback ceremony or version picker.

### 4.8 Details and Settings

Details may reveal:

- Exact intention.
- Selected harness/model/provider route.
- Actual token usage and charge if recorded.
- Deterministic gate outcomes.
- Human-readable failure.
- Execution timing.
- Open source folder.
- Export support report.

Settings must include:

- iPhone and Apple readiness.
- App storage location.
- Local factory status.
- Advanced intelligence configuration.
- Managed balance and transactions.
- Privacy explanation.
- Updates.
- Diagnostics and safe service restart.
- Optional legacy browser Studio launch.

## 5. Harness, local-model, estimate, and cost design

Extend the existing harness abstraction rather than replacing it.

### 5.1 Discovery

- Preserve current known-harness detection.
- Explain usability: installed, authenticated where safely detectable, configured, unavailable, or needs attention.
- Never read or return raw provider credentials.
- Add explicit custom harness configuration with executable chosen through a file picker, validated non-symlink regular executable, bounded arguments, declared model list, and no shell interpolation.
- Add local OpenAI-compatible provider configuration with explicit base URL, optional Keychain credential, health check, model discovery, and consent before sending repository content.

### 5.2 Automatic route selection

Default priority should be configurable but initially favor:

1. Owner-selected per-app route.
2. Usable preferred local/BYO harness.
3. Usable local model route when the user has opted in.
4. TOHSENO-managed Bankr route when access and balance exist.
5. A clear blocked state explaining what is missing.

Do not silently move from local/BYO to paid managed compute. Require explicit consent the first time and whenever a request would exceed its approved cap.

### 5.3 Estimation

Create a versioned estimator using:

- Selected model pricing snapshot and timestamp.
- Intention size.
- Reference count and size.
- Existing app source/context size for evolution.
- Historical input/output and repair usage for similar local TOHSENO operations when available.
- One implementation invocation plus at most one repair, matching ADR 0019.

Return a range, not fake precision. Persist the accepted maximum with the durable command. Managed execution must pause safely before exceeding it.

For Bankr, fetch and cache the live model catalog/pricing through the server, validate every model against an operator allowlist, and timestamp the snapshot. Never trust a model or price submitted by the Mac.

## 6. Stripe and balance implementation

Implement Stripe properly as follows.

### 6.1 Ledger

Add an authoritative server-side append-only ledger keyed to an opaque, derived installation binding. Do not use a raw local path, Apple ID, email, wallet address, Shot ID, or workspace seed as the public billing identifier.

Minimum concepts:

```text
InstallationAccount
BalanceLedgerEntry
PromotionalGrant
CheckoutPurchase
ComputeReservation
InferenceCharge
RefundAdjustment
ProviderReconciliation
```

Every ledger entry includes:

- Stable unique ID.
- Installation binding.
- Amount in integer micro-USD or another documented exact integer denomination; never floating point.
- Currency.
- Entry type.
- Related checkout/payment/execution/reservation/provider IDs where applicable.
- Idempotency key.
- Created timestamp.
- Human-safe description.
- Private operator metadata separated from client projection.

Balance is the deterministic sum of valid ledger entries. Do not store a mutable balance without an auditable ledger authority.

### 6.2 Packs

Make packs server-configured Stripe Prices, never client-supplied amounts. Seed development fixtures for:

- $10 creation balance.
- $25 creation balance.
- $50 creation balance.

Do not promise bonus amounts until product configuration explicitly defines them. Preserve existing monthly/yearly subscription code behind configuration, but do not force subscription purchase for local/BYO use.

### 6.3 Checkout

Flow:

1. Native app requests a short-lived signed checkout claim for an allowed pack.
2. Local service signs the derived installation binding and requested server-known pack ID.
3. Website verifies the claim and creates a fresh Stripe Checkout Session server-side.
4. Session uses a predefined Stripe Price ID and carries only bounded opaque metadata.
5. Native app opens Stripe’s hosted Checkout in the default browser or a compliant authentication session.
6. Redirect returns to a universal/custom app link only to improve UX; it never proves payment.
7. Stripe webhook is authoritative.
8. Server records exactly one purchase ledger credit.
9. Native app refreshes a server-signed or authenticated balance projection.

Use Stripe idempotency keys when creating sessions. Verify webhook signatures against the raw request body and enforce timestamp tolerance. Handle at least:

- `checkout.session.completed`
- `checkout.session.async_payment_succeeded`
- `checkout.session.async_payment_failed`
- Refunds and disputes that affect credited value

Prevent duplicate credit across repeated or reordered events using Stripe event ID plus PaymentIntent/Checkout Session uniqueness. Never trust line items, amounts, customer IDs, pack IDs, or installation bindings supplied by the Mac after checkout; retrieve and verify authoritative Stripe objects where required.

### 6.4 Reservation and charging

Before managed work:

1. Create a reservation for the approved maximum.
2. Ensure spendable paid/promotional balance covers it according to documented priority rules.
3. Bind reservation to one stable TOHSENO command/execution and model allowlist.
4. Issue a short-lived managed-inference capability that cannot exceed that reservation.
5. Record provider calls against the reservation.
6. On terminal completion, charge actual retail usage and release unused reserved value.
7. On safe cancellation/failure before usage, release the reservation.
8. On ambiguous provider outcome, hold the disputed portion pending reconciliation rather than charging twice or pretending it is free.

The local factory must checkpoint and report a recoverable state if managed balance runs out. Never destroy partial source or regenerate from scratch solely because payment was interrupted.

### 6.5 Promotional grants

Add a separate authenticated operator tool, not part of the consumer CLI, for example:

```text
tohseno-admin credits grant \
  --installation <derived-id> \
  --usd 5 \
  --reason "welcome grant after direct onboarding"
```

Require operator authentication, reason, idempotency key, and an audit event. Support revocation only through a compensating ledger entry. Never edit or delete the original grant.

### 6.6 Stripe configuration and tests

Add `.env.example` entries and a precise runbook for:

- Stripe secret key.
- Webhook signing secret.
- Publishable key only if actually needed.
- Price IDs for each allowed pack.
- Success/cancel URL origins.
- Billing receipt signing key and pinned verifier.
- Test-mode versus live-mode fail-closed separation.
- Stripe CLI webhook testing.
- Refund/dispute behavior.
- Key rotation.
- Reconciliation and backup.

Secrets never enter Git, Swift resources, generated apps, logs, crash reports, command arguments, or signed public release artifacts.

Use fake/in-memory providers for deterministic tests and Stripe test mode only for explicit integration verification. A source change must not activate live billing.

## 7. Migration and compatibility

- Existing CLI installations, workspace identity, app folders, command journals, Shots, accepted Versions, Git repositories, ignored private paths, pairing state, and entitlement records must remain readable.
- The native app must detect an existing healthy service and adopt it safely.
- If it replaces installed program files or LaunchAgent configuration, use explicit installer ownership markers, verified versions, rollback, and no destructive broad deletion.
- Do not relocate `~/Desktop/Tohseno` automatically.
- Do not rewrite existing `.tohseno/` metadata.
- Do not invalidate existing Companion pairings even though Companion is hidden from v0.
- Existing CLI create/evolve commands must converge on the same application service and remain functional.
- Browser Studio remains a thin projection if retained; it must not diverge into a second product implementation.

## 8. Security and privacy

Update the threat model for:

- Native app ↔ local service authentication.
- Malicious local webpages and processes.
- Bundled nested executable replacement.
- Update-feed compromise.
- Managed inference credential theft.
- Bankr operator-key compromise.
- Stripe webhook replay and forged redirects.
- Balance double-credit/double-spend.
- Model price tampering.
- Prompt/source retention and provider privacy tiers.
- Custom harness command injection.
- Local model endpoint impersonation.
- Symlink/path attacks in app folders and attachments.

The UI must explain, in plain language, where code and prompts go:

- Local/BYO work stays under the selected harness’s own provider/configuration.
- Local-model work stays on the configured local endpoint unless that endpoint itself routes elsewhere.
- Managed work sends necessary context through TOHSENO and Bankr to the selected upstream provider under the chosen privacy tier.
- Generated source remains canonically stored on the owner’s Mac.

Do not make stronger privacy claims than the implementation and upstream policies support.

## 9. Testing and verification

Preserve every existing repository test unless an accepted ADR explicitly changes the governed behavior. Update tests that encode superseded npm-first, mandatory-Companion, qualification, or visible model-picker behavior.

Add:

### Native macOS tests

- Feature/view-model unit tests without requiring UI automation.
- Native-client/service contract tests.
- Route restoration and state restoration.
- First-run projection for every observable gate.
- Existing workspace adoption.
- Create/evolve submission exactly once.
- Window close while execution continues.
- Human presentation parity with Rust fixtures.
- Accessibility identifiers and basic UI navigation tests.

### Harness tests

- Detection and usability.
- Custom executable validation.
- No shell interpolation.
- Local endpoint consent and failure.
- Automatic selection order.
- Advanced selection persistence.
- Cost estimate provenance and timestamp.
- Managed fallback never activates silently.

### Billing and Bankr tests

- Append-only balance derivation.
- Duplicate/reordered Stripe webhooks.
- Async payment success/failure.
- Refund/dispute compensating entries.
- Reservation race and insufficient balance.
- Maximum spend enforcement.
- Provider timeout and ambiguous result.
- Bankr `401`, `402`, `429`, `5xx`, and malformed usage.
- Operator key never appears in client-visible fixtures/logs.
- Promotional grant audit and idempotency.
- Model allowlist and server-authoritative pricing.
- Fake Bankr gateway integration with streaming and non-streaming responses as actually used.

### Packaging tests

- App bundle structure.
- Nested executable signatures.
- Hardened runtime entitlements.
- No forbidden secrets.
- Update integrity and rollback.
- Clean install and existing-install migration fixtures.

Run the complete existing verification matrix in `AGENTS.md` plus all new native, billing, and managed-inference checks. Record truthful evidence. If physical iPhone, notarization, Stripe test-mode, or Bankr live-account checks are not possible in the environment, leave explicit owner runbooks and mark them unverified rather than mocking them into a release-ready claim.

## 10. Delivery phases

Implement in this order so each phase leaves a coherent, testable product:

### Phase A — Decision and native shell

- Accepted ADR and updated architecture/state docs.
- SwiftUI app target and design system.
- Bundle/connect to existing Rust service.
- Native session authentication.
- Native Your Apps, Create, App Detail, Details, Settings.
- Existing harness-backed create/evolve path works through the native app.
- Browser Studio retained only as fallback.

### Phase B — Companion-independent genesis

- Refactor cable genesis into Apple/iPhone readiness.
- Remove mandatory Companion install/pair from product and entitlement gating.
- Fresh Mac can reach generated-app installation without Companion.

### Phase C — Advanced intelligence

- Hide normal model/harness selection.
- Add Advanced harness/model configuration.
- Add custom harness and local compatible endpoint support.
- Add truthful route/cost projections.

### Phase D — Balance and Stripe

- Server-side ledger.
- Promotional grants.
- Stripe packs, Checkout, authoritative webhooks, refresh, refunds/disputes.
- Native balance and transaction UI.
- No live activation.

### Phase E — Bankr managed inference

- Bankr provider adapter and operator runbook.
- TOHSENO proxy with scoped execution credentials.
- Reservations and hard spend ceilings.
- Managed fallback consent.
- $TOHSENO launch-fee funding status recorded only from real Bankr configuration.
- No operator secret in clients.

### Phase F — Distribution candidate

- Direct-distribution app packaging.
- Developer ID/notarization/update runbooks and verification tools.
- Migration from CLI-first installation.
- Clean-Mac and physical-iPhone owner checklist.
- Release readiness remains false until real evidence and owner authorization exist.

Commit coherent phases separately if repository policy permits. Do not collapse architecture, native UI, billing, managed inference, and release changes into one impossible-to-review commit.

## 11. Explicit exclusions

Do not add or activate during this evolution:

- A public Shot marketplace.
- Smart-contract or registry deployment.
- Token deployment.
- Companion redesign or App Store submission.
- APNs activation.
- Production relay activation.
- TestFlight or generated-app App Store submission.
- Family collaboration.
- Multiple-factory scheduling.
- Cloud storage of full source by default.
- An interactive fake simulator.
- Unbounded autonomous repair loops.
- Silent fallback from local to paid inference.
- Live Stripe or Bankr credential use without explicit authorization.

## 12. Definition of done

The implementation is complete only when repository evidence supports this journey:

1. A nontechnical person downloads and opens a signed/notarizable TOHSENO Mac application.
2. They never need Terminal, npm, Node, Homebrew, a coding-agent installation, or an API key for the managed path.
3. TOHSENO guides the unavoidable Xcode, Apple Account, cable, Trust, and Developer Mode steps one at a time.
4. Companion setup is not required.
5. Existing local/BYO harnesses are automatically detected and remain free to use through TOHSENO.
6. Local-model and custom-harness owners can configure them under Advanced.
7. A person with no harness can request a personally granted welcome managed balance.
8. The normal creation screen contains an intention, optional name/images, an honest cost range where relevant, and Create App.
9. One bounded execution produces, builds, verifies, signs, installs, and launches a real native iOS app.
10. Closing the Mac window does not stop admitted work.
11. The generated app’s source is an ordinary local Git repository owned by the user.
12. An evolution preserves app identity and existing data according to the current state-transition rules.
13. Managed inference is funded through the server-side balance system, uses Bankr behind a TOHSENO proxy, and cannot exceed the approved reservation.
14. Stripe payment is credited only from verified webhooks and is idempotent.
15. No Bankr, Stripe, Apple, or signing secret ships in the application or repository.
16. Existing protocol, Shot history, CLI workflows, and hidden Companion state remain valid.
17. All available automated tests pass, and every unavailable external/physical verification remains explicitly unverified.

## 13. Required final report

At the end, report:

- The exact product behavior now implemented.
- Architecture decisions and ADRs added/superseded.
- Files/components added and materially changed.
- Existing machinery reused rather than duplicated.
- Migration behavior for existing users.
- Test commands run and exact results.
- Physical-device, Stripe, Bankr, signing, notarization, and release checks actually performed.
- Checks not performed and why.
- Secrets/configuration the owner must provide.
- External activation steps still requiring explicit owner action.
- Known risks and the smallest next milestone.

Lead with working software. Do not stop after producing a plan, mockup, or documentation. At the same time, do not fabricate external success: a source implementation is not a notarized release, a fake webhook is not a Stripe payment, a fake gateway is not Bankr-funded inference, and a Simulator build is not an app installed on a physical iPhone.
