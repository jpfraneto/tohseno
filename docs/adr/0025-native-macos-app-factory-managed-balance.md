# ADR 0025: Native macOS is the product; the existing local service remains the factory

Status: accepted

Date: 2026-08-27

Supersedes:

- ADR 0016 only where it makes browser Studio or the iOS Companion a normal
  product surface. Its App → Intent → App on your iPhone vocabulary, thin
  projection rule, six-state presentation, exact-base evolution, and deletion
  of the dashboard remain accepted.
- ADR 0020's mandatory Companion genesis, successful-day qualification,
  trial-expiry purchase refusal, subscription gate on local factory work, and
  npm consumer front door. Its observable Apple readiness probes, persistent
  service, installer integrity, preservation rules, and signed hosted-billing
  boundary remain useful machinery.
- ADR 0021's choice of global npm postinstall as the normal first-run door.
  The dependency-free bootstrap remains a supported legacy/developer path.
- ADR 0023's visible Studio creation selector. Per-command exact selection
  remains accepted, but harness and model choice moves under Advanced in the
  native product.

## Context

The local factory is mature: one application service durably admits commands,
one bounded engine transition builds and verifies source, Apple gates sign and
install it, and one persistent service survives frontend closure. The product
around it still asks an ordinary person to install Node/npm, opens a browser,
requires a separate iPhone Companion before the first generated app, exposes a
coding-harness choice too early, and denies purchase unless a five-day
qualification ceremony succeeds.

Those requirements are implementation history, not the product promise. A
person who wants one small native app should be able to open a normal Mac app,
describe it, and have the result installed directly on a connected development
iPhone. The existing factory must be projected, not replaced.

Some owners already have authenticated coding harnesses or local inference.
They must not be forced into managed compute or a TOHSENO subscription. People
without those tools need an intentionally authorized, metered managed path.
Stripe is payment collection, an append-only TOHSENO ledger is balance
authority, and Bankr is an upstream inference expense; none may be confused
with another.

## Decision

### Native Mac product and one factory

`Tohseno.app` is the primary consumer product. It is a native Swift/SwiftUI Mac
application, not a web view. It owns Mac navigation, windows, restoration,
menus, accessibility, file selection, drag and drop, progress, alerts, and
settings. The normal surface remains App → Intent → App on your iPhone and
does not expose Shot, Expression, Version, execution, harness route, relay, or
provisioning vocabulary.

The app is a client of the existing persistent Rust Local Workspace Service.
For v0 it uses the loopback JSON API and event stream. All create/evolve work
still converges on `ShotApplicationService` and the existing engine,
build/sign/install path, durable command journal, app-local execution record,
and one factory lease. There is no Swift factory, second Shot implementation,
or frontend-to-frontend invocation.

Browser Studio remains an advanced support and recovery projection until
native parity is established. The CLI remains an automation, development,
diagnostics, and recovery interface. Neither is the consumer installation or
first-run door.

Browser mutations retain exact Host/Origin/anti-CSRF enforcement. Native
mutations use a separate bounded native session with an explicit client
identity, scope, expiry, and per-service-instance binding. A browser token is
not a native token and neither token crosses into generated apps.

### Installation and distribution

The consumer artifact is a self-contained macOS `.app` distributed in a
`.dmg`. It contains the native UI, the Rust service/front-door executable, and
the static resources needed by the product. Ordinary use requires no npm,
Node, Bun, Homebrew, or separately installed TOHSENO CLI. The app may adopt and
update the existing installer-owned support layout only through explicit
ownership markers, exact integrity manifests, verified version transitions,
and rollback.

Release tooling signs nested executables before the outer app, enables the
hardened runtime, verifies entitlements and forbidden-secret absence, builds a
deterministic DMG payload, supports Apple notarization submission and
stapling, and verifies the final artifact. Source support is not notarization,
publication, or release evidence. External signing, notarization, upload, feed
activation, and publication remain explicit owner actions.

### Companion-independent Apple readiness

Native first run projects observable Mac/Xcode/iPhone state one instruction
and at most one primary action at a time: supported macOS, full Xcode and its
components/license, cable/unlock/Trust, Developer Mode, development team, and
a real minimal build/sign/install/launch readiness check. It may open the
official Xcode destination and Xcode account settings but never collects Apple
credentials.

Installing or pairing the TOHSENO Companion is not a readiness, entitlement,
create, evolve, or generated-app installation gate. The Companion, SDK,
pairing state, capability system, encrypted relay, and tests remain intact and
readable. Existing pairings are not invalidated. Companion setup is hidden
from the normal native path and no APNs, relay, or Companion distribution
activation is authorized here.

### Intelligence routes

One versioned route model extends the existing harness abstraction. It can
describe:

1. known authenticated harness adapters, including Codex, Claude Code,
   OpenCode, and other explicitly supported adapters;
2. an owner-approved custom executable selected as an absolute, regular,
   non-symlink executable, with bounded literal arguments and no shell
   interpolation;
3. an explicitly configured local OpenAI-compatible endpoint, with an
   allowlisted loopback HTTP URL, optional Keychain credential,
   bounded health/model discovery, and recorded consent before source is sent;
4. TOHSENO-managed inference admitted through the server balance boundary.

Automatic selection prefers an exact per-app choice, then a usable preferred
local/BYO harness, then an opted-in local endpoint, then a consented managed
route with sufficient balance. It never silently changes a recovered command
or silently falls from local/BYO into paid managed execution. The resolved
harness, model, route, pricing snapshot, estimate range, and maximum managed
authorization are persisted with the durable command.

The normal composer contains intention, optional name, up to eight exact-byte
validated references, and cost only when the selected route has metered
incremental cost. Advanced owns detection status, model choice, custom/local
configuration, privacy tier, estimate provenance, cap, and restoration of
automatic selection. Subscription-backed routes say provider-plan cost is
unknown or covered rather than claiming zero. Local endpoints show `$0 paid to
TOHSENO`, not that hardware or electricity is free.

The estimator is versioned and range-based. It binds a timestamped
server-authoritative pricing snapshot, intention/reference/context size,
available historical usage, and ADR 0019's one implementation plus at most one
repair. Actual recorded usage and charge replace estimates after completion.

### Creation balance, Stripe, and promotion

Local/BYO execution is never gated by TOHSENO subscription, qualification, or
managed balance. Existing apps, source, history, local harnesses, and accepted
state remain usable. Managed work alone requires creation balance and one
explicitly accepted maximum.

The server owns an append-only USD ledger keyed by an opaque derived
installation binding. Exact integer micro-USD entries represent promotional
grants, checkout purchases, reservations, inference charges, reservation
releases, refunds/disputes, and provider reconciliation. Balance is derived
from valid entries; no mutable balance field is authoritative. Paid and
promotional funds remain distinguishable and reservation priority is
documented.

Development fixtures define $10, $25, and $50 Stripe Price-backed packs.
Checkout begins from a short-lived installation-signed claim for a server-known
pack. Stripe Checkout is created server-side with idempotency. Redirects are
UX only. Raw-body signature-verified webhooks and retrieved authoritative
Stripe objects determine credits, refunds, and disputes; duplicate and
reordered events cannot double-credit or roll state back.

Before managed work, the server reserves the accepted maximum for one command,
execution, allowed model set, and privacy tier. A short-lived capability cannot
spend above that reservation. Terminal reconciliation charges actual retail
usage and releases the remainder. Ambiguous provider outcomes hold the
disputed portion. A balance interruption preserves partial source and becomes
a recoverable execution state.

Welcome balance is personal in v0. The app may show a release-configured
“Message JP for welcome compute” action but never invents automatic credit.
Only an authenticated operator command or protected operation can grant one
exact installation an integer USD amount with a reason, idempotency key, and
append-only audit event. Revocation is a compensating entry.

The earlier monthly/yearly entitlement implementation remains readable and
configuration-gated for compatibility. It no longer gates local/BYO factory
admission and is not the normal native purchase surface.

### Managed Bankr inference

The managed service is a narrow admitted-execution proxy to Bankr's official
LLM Gateway. The Bankr operator key exists only in the server secret manager
and is scoped to inference. It never appears in the Mac app, Rust binary, CLI,
generated app, repository, environment passed to a local harness, process
argument, log, receipt, support report, or crash record.

The proxy validates the installation-bound short-lived capability, reservation
and command identity, model allowlist, timestamped server pricing, privacy
tier, body/token bounds, request rate, and maximum authorized spend before
forwarding. It records provider request ID, model, usage, provider-reported
cost when available, retail charge, tier, and reconciliation status without
retaining source or prompts beyond the documented minimum. It is not a
general-purpose Bankr proxy and cannot perform wallet or token transactions.

Bankr authentication failures, exhausted operator credits (`402`), rate
limits, malformed usage, timeouts, and gateway failures are bounded,
recoverable managed-compute states. There is no indefinite retry. Operator
health shows credit availability, reconciliation, and launch-fee funding
status; launch fees are reported as active only from real account
configuration evidence.

### Migration, privacy, and release truth

The native app adopts an existing healthy service and the existing default
`~/Desktop/Tohseno` and `~/.tohseno` roots in place. It does not relocate app
folders, rewrite `.tohseno/`, fabricate identity, migrate Shots, invalidate
pairings, or change accepted lineage. Existing CLI and retained Studio calls
continue through the same application service.

The threat model covers native-session theft, malicious local origins and
processes, nested executable replacement, update-feed compromise, managed
credential theft, Stripe replay and forged redirects, balance races and
double-spend, price tampering, privacy-tier/retention truth, custom command
injection, local endpoint impersonation, and symlink/path attacks. Generated
source remains canonical on the owner's Mac. UI privacy copy distinguishes
local harness provider behavior, configured local endpoint behavior, and the
TOHSENO → Bankr → upstream-provider managed path without promises stronger
than deployed policy.

## Consequences

TOHSENO has one factory and a new primary projection. Closing its window does
not cancel admitted work; reopening restores durable state. Companion becomes
optional infrastructure rather than prerequisite product ceremony. Local
owners keep using their own intelligence without paying TOHSENO, while managed
users receive a separately consented and hard-capped path.

This decision changes no public protocol encoding, frozen vector, Shot
identity, accepted lineage, Builder authority, active contract generation, or
registry behavior. It authorizes no live Stripe key, Bankr key, launch-fee
funding claim, APNs/relay activation, Developer ID signature, notarization,
publication, update feed, physical-device result, or external release. Those
claims require their own real evidence and owner action.
