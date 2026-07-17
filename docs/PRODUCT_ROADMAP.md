# TOHSENO product roadmap

This roadmap organizes the path from the live TOHSENO product shell to an
intention-preserving continuity-app system. It does not turn proposed work into
implemented capability.

## Baseline captured on 2026-07-17

The product shell is deployed at `https://tohseno.com`, and its public health
route responded successfully when this baseline was recorded. The exact commit
running in production is not yet exposed or recorded, so deployment provenance
remains **Open**.

**Implemented:** private Markdown intake, encrypted persistence, capability
handoff, mode-specific order state, payment and email adapters, an operator
boundary, a manifest validator, an early contract harness, and an agent capsule.

**Prepared:** a single-process Railway deployment and an ejectable coding-agent
handoff. The repository does not yet implement a native continuity-app compiler, a generic
identity suite, a phone-to-browser bridge, a browser extension, an application
revision ledger, a one-line initializer, a per-application deployment cell, or
external action rails.

The [POST route authority audit](POST_AUTHORITY_AUDIT.md) records, as of this
baseline, which authority actually admits each mutating route. None is
phone-rooted today.

## North star

TOHSENO should turn an owner's intention for one repeated action into an
ejectable, local-first application whose evolution remains traceable:

```text
owner intent
    → manifest revision
    → implementation and release evidence
    → act → record → reflect → continue
```

The phrase **software seed** is the working product thesis: a small source of
intent that can grow into full-stack software without losing its purpose,
privacy boundary, ownership, or history. “The future of software” is positioning,
not an implementation claim.

## Keep the histories distinct

TOHSENO needs several append-only histories, but they are not one generic log.

| History | Meaning | Private-content rule |
|---|---|---|
| Owner intent revision | What the owner asked the application to become | Exact source is private and encrypted or owner-held |
| Manifest revision | The deterministic product-contract change accepted from that intent | Contains declared behavior, not raw private prompts |
| Release revision | The commit, build, tests, migrations, and deployment evidence that realized a manifest revision | Contains provenance, never credentials |
| Continuity event | A person's completed or interrupted core action | Local or application-encrypted by default |
| Authorization audit event | Pairing, delegation, expiry, and revocation decisions | Typed and content-free |
| Operation audit event | Attempt and outcome for one allowlisted external request | Typed and content-free; no request body |
| Order event | Commercial and operator state for a TOHSENO order | Safe metadata only |

Stable identity for each record must be separate from its content digest. A
digest identifies exact bytes; it does not authorize access, prove intention, or
replace a stable revision identifier.

## Workstreams

### 0. Record the live shell

Status: **Implemented** endpoint and health response; **Open** release identity.

Record the deployed commit or image identifier, configuration modes without
secret values, synthetic smoke evidence, rollback reference, and known
limitations. This is the TOHSENO shell's operational record. It must not be
confused with the `LIVE` state of a generated customer application.

Exit criteria:

- the public endpoint maps to an exact source revision and deployment artifact;
- synthetic intake, capability, payment boundary, and restart checks are dated;
- no private submission or credential appears in the record.

### 1. Preserve application evolution

Status: **Proposed**.

Define separate `IntentRevision`, `ManifestRevision`, and `ReleaseRevision`
contracts. Preserve the owner's exact evolution request in an owner-controlled
private location; keep only a safe index in a public repository. An agent must
produce a manifest diff, state unsupported requests explicitly, record evidence,
and treat rollback as a new revision rather than erasing history.

Exit criteria:

- schemas and fixtures cover parentage, branches, approval, application,
  verification, rollback, and deletion tombstones;
- raw prompts remain encrypted and absent from logs, public Git, payment
  metadata, and public chains;
- a release can be traced back to the accepted manifest and owner intent;
- an ejection includes the complete owner-authorized evolution history.

See [Application evolution history](proposals/APPLICATION_EVOLUTION_HISTORY.md).

### 2. Define finite operations and audit records

Status: **Proposed**.

Before pairing a phone or adding an extension, define a small operation registry.
Each operation has a stable ID, method, path template, exact body schema,
authorization policy, privacy classification, idempotency rule, failure mode,
and content-free audit outcome. Unknown operations and additional fields fail
closed.

Exit criteria:

- contract fixtures reject route, method, body, operation, origin, scope, and
  replay substitutions;
- authorization and operation audit records are distinct from continuity events;
- the browser can show a useful local history without displaying secrets or
  pretending that mutable local history is independent proof.

### 3. Pair phone and browser

Status: **Proposed**; identity suite, delegation protocol, and the
co-signing-versus-delegation decision (open question 16) are **Open**.

The accepted authorization invariant is that every application-authorized
mutation after contextual identity exists must have a verifiable authorization
chain rooted in the phone; public bootstrap/pairing routes and provider
webhooks are outside it by construction. The recommended shape uses the phone
to approve a short-lived, origin- and app-scoped delegation to an ephemeral
browser key. The phone keeps recovery and long-lived private material.
The QR payload is a single-use pairing challenge, never a seed, private key, or
bearer credential. High-risk operations require a new phone approval. Whether
the phone instead literally co-signs every request is the owner decision that
gates verification-middleware implementation.

Exit criteria:

- wrong-origin, changed-body, expired, replayed, revoked, and wrong-session
  requests fail;
- a captured QR image cannot authorize a request by itself;
- key loss, rotation, multiple devices, expiry, and revocation have explicit
  behavior;
- the core action, local record, recovery, and export still work when the bridge
  is unavailable.

See [Phone-to-browser bridge](proposals/PHONE_BROWSER_BRIDGE.md).

### 4. Exercise one reference bridge

Status: **Open**.

Replyguy is a strong candidate because “reply before you scroll” is one clear
action and the extension can exercise a real phone/browser boundary. Before it
becomes a TOHSENO proof application, its manifest must define what works
offline. A safe shape is local composition or practice first, with publication
as an optional external operation; a network-only reply action would not satisfy
the current offline-core-action contract.

Exit criteria:

- one confirmed manifest and ritual-destroyer list;
- local value before login and despite extension/network failure;
- one end-to-end delegated operation using the finite registry;
- content-free audit and explicit disclosure for anything leaving the device.

### 5. Create the local agent initializer

Status: bootstrap **Implemented** in-repo (`/oneshot.sh`, pinned two-stage
design, tested); production availability, pin bump, and the fresh-machine
rehearsal are **Prepared**/**Open**.

Offer a pinned one-line initializer that asks which supported coding agent will
build the application, installs the matching local capsule, and creates a private
source contract. An install code or capsule capability must not be placed in a
command-line argument, URL query, process list, or shell history.

A free community initializer is a new product path. It may coexist with the
current paid order modes, but current prices cannot be replaced by “FREE” copy
without an explicit state, support, and ownership contract.

Exit criteria:

- pinned and verifiable distribution, dry-run preview, dirty-tree protection,
  deterministic output, rollback, and uninstall behavior;
- private prompt stays local unless the owner chooses the encrypted intake;
- at least two agent adapters consume the same manifest and invariants;
- package publication occurs only with explicit owner approval.

See [Agent initializer](proposals/AGENT_INITIALIZER.md).

### 6. Package one application as one deployment cell

Status: **Proposed**; multi-tenancy trust tier and shared-ingress boundary are
**Open** (open question 17).

The generated backend is one package that one owner-run command takes from
source to a healthy deployment: one OCI image per application, secrets outside
the image, one persistent data volume, one Compose project per cell, with
migrations, health, backup, rollback, and ejection included. Per-cell network,
Unix identity, and resource limits bound each application. Containers isolate
one owner's applications; untrusted multi-tenant hosting requires a
hardware-virtualized boundary. The cell is the server-side unit; native mobile
binaries are built and distributed through platform toolchains, not the cell.

Exit criteria:

- one command from clean host to passing health check, with a
  content-addressed release identity;
- the image is verified free of secrets, credentials, and customer data;
- backup, restore, and rollback are each rehearsed, and rollback appears as a
  new release revision;
- cells cannot read each other's data, and one saturated cell does not take a
  neighbor down;
- ejection output runs on a second machine with no TOHSENO dependency.

See [Deployment cell](proposals/DEPLOYMENT_CELL.md).

### 7. Add the first optional external action rail

Status: **Open**.

Choose one exact application operation before choosing a provider or chain.
Base, Solana, Hyperliquid, and Robinhood are not interchangeable “networks.”
On-chain code, RPC adapters, exchange execution, and brokerage APIs have
different custody, disclosure, cost, rollback, and ownership rules.

The first rail must be optional, manifest-declared, testnet or sandbox first,
and removable without breaking local continuity. No generic trading, transfer,
or contract-deployment authority is implied.

See [External action rails](proposals/EXTERNAL_ACTION_RAILS.md).

### 8. Stabilize compiler stages

Status: **Proposed**.

Only after two materially different applications pass the same contracts should
the agent workflow become stable compiler stages. Each generated change must
trace to a manifest property and revision, with reproducible builds, migration
compatibility, ejection, and rollback evidence.

### 9. Teach from evidence

Status: **Proposed**.

Every completed revision can become a programming or vibe-coding tutorial:

```text
public problem statement
→ private intent boundary
→ manifest diff
→ contract and threat model
→ implementation
→ tests
→ live verification
→ retrospective
```

Tutorials are separately published, owner-approved derivatives. They do not make
raw prompts, private continuity content, credentials, or production data public.
Claims use **Implemented**, **Prepared**, **Proposed**, and **Open** precisely.

## Immediate milestone

The next bounded engineering milestone is the application-evolution and finite-
operation contract pack: schemas, golden fixtures, privacy inventory, and ADR
proposals. It deliberately precedes native pairing, an extension, an initializer,
or any smart contract. That gives every later tutorial and implementation an
auditable chain from intention to tested software.

Two owner decisions gate the work behind that milestone: co-signing versus
scoped delegation with step-up (open question 16) gates the request
verification middleware, and the multi-tenancy trust tier (open question 17)
gates any hosted deployment-cell offering. Once question 16 is decided,
implementation proceeds contract-first: operation definitions, delegation and
envelope contracts, typed audit events, a production verifier, durable replay
protection, and negative fixtures.
