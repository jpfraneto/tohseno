# Things that are not clear for TOHSENO yet

The asymptote of this file is empty. Every entry is an unresolved product,
security, or ownership decision in the current architecture. Delete an entry
when its decision is recorded or its behavior is implemented.

## 1. The path stops at Simulator

**Implemented:** TOHSENO creates, verifies, builds, launches, and captures an
independently owned iOS Shot locally.

**Prepared:** `owner-ladders/appstore-ios.json` describes the human Apple path,
including costs, waits, dependencies, detectors, and irreversible steps.

**Open:** load and validate that catalog through the shared factory, project
the same steps through CLI and Studio, and decide which owner-approved Apple
actions may ever be automated. TestFlight submission and production deployment
remain Proposed.

## 2. Owner-step evidence and freshness

**Open:** decide which owner steps can produce trustworthy machine evidence,
which remain explicit human confirmations, and how instructions stay current
when an external provider changes its process independently of a factory
release. Stale confident instructions are worse than an honest unknown.

## 3. Process topology

The CLI currently runs a bounded flow and exits; Studio starts the loopback
server.

**Open:** decide whether one long-running local process should own shared state
while CLI and Studio become clients, or whether the current direct execution
model is the permanent boundary. If a service is chosen, its startup,
shutdown, port allocation, second-instance behavior, authentication, and
recovery semantics must be explicit.

## 4. Capability and agent vocabulary

“Skill” currently risks referring both to a deterministic app capability under
`skills/` and to instructions or extensions for a coding agent.

**Open:** choose distinct public names before TOHSENO itself is distributed as
an agent extension. Decide whether that extension complements the managed CLI
or becomes the primary entry point without weakening deterministic
composition, ownership, or ejection.

## 5. Production Builder identity

**Implemented:** protocol records separate Builder authority from an
individual generated app's runtime identity. The included signer exercises the
versioned interface in tests; it is not production custody.

**Open:** choose the production identity and signer method, key scope,
Mac-side custody, recovery policy, and explicit public-signing consent. Define
the trust root for accepting a Builder attestation and how clients discover
and resolve competing valid genesis histories for the same Shot ID across
nodes. Protocol version 1 has no cross-node fork-resolution rule.

## 6. Optional public anchoring

**Implemented:** signed records are portable Builder attestations; a registry
deterministically projects one accepted history; `RecordAnchorAdapter` is a
deployment-neutral seam. `APPCOIN_LINKED` records only an external identifier
and evidence and performs no deployment, minting, transfer, pricing, or
endorsement.

**Open:** decide whether record anchoring is needed, what minimal claim an
adapter would publish, who operates and pays for it, how clients behave during
outages, and whether any such work should precede a complete local-to-App-Store
owner path.

## 7. Provider and commercial boundary

The current planner invokes one owner-selected coding-agent provider. Raw
intent is disclosed only to that provider under its terms.

**Open:** decide whether the planning boundary should support additional
providers; how provider selection, billing, credits, and key custody would
work; and whether any commercial layer can preserve the account-free local
factory. Composition, locks, verification, and ejection must remain
provider-independent.

## 8. One-line terminal experience

**Open:** define the raw-key interaction primitive, concise failure rendering,
and the path to full diagnostics. The intended contract is one line naming the
failure, one copy-pasteable next action, and content-free detail written to a
local path, but that interaction has not been proven across Xcode and signing
failures.

## 9. Mascot and brand ownership

**Open:** decide whether a character belongs in the product voice, who owns
the mark, and how it maps to existing machine states without adding ceremony
or decorative output. Any character must be an owned brand asset, not a rented
dependency.

## 10. Package names

**Prepared:** package manifests use the `@tohseno` scope, but registry
publication is not part of the current installer design.

**Open:** confirm control of every name before the first package-registry
publication and decide whether registry distribution is necessary at all.
