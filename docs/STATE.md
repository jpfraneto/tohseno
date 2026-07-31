# State of this repository

Written 2026-07-30. This is the plain-language answer to "what is going on
here" for someone returning after time away. When something below stops being
true, update this file in the same change that makes it untrue.

## What ships today

The current product release is TOHSENO 0.7.1, for macOS only. A person runs
one install command (`curl -fsSL https://tohseno.com/oneshot.sh | bash`),
which verifies a pinned, immutable 0.7.1 release archive, installs the
`tohseno` command-line tool and its Apple identity helper transactionally, and
starts Studio, a localhost-only browser view over the same engine. The
installer script is pinned to an exact version and checksum; releasing means
updating that pin, not rebuilding from moving sources.

What the user then gets is a local loop that needs no phone, no account, and
no TOHSENO server: they declare one intention in plain words, their own coding
agent (Claude Code or Codex, installed and authenticated by them — TOHSENO
never holds agent credentials) materializes a native iPhone app from it, and
the tool records an immutable version of that app. They can attach private
feedback to an exact version, evolve the app into a new version without losing
its origin, and verify, export, and import the whole signed history offline.
The folder on disk is the product: the app's identity and history live in
files inside it. The full end-to-end flow was re-verified on 2026-07-30 by
`scripts/test-ontology-lifecycle.sh` against real Xcode builds.

## The contracts, and the fact that none are deployed

`contracts/src/` holds four Solidity sources. None of them — and no TOHSENO
contract of any generation — is deployed on any network. They exist so that,
if a public witness layer is ever activated, its design is already reviewed,
tested, and reproducible.

`BuilderAccount` is a non-upgradeable smart account controlled by P-256 device
keys (the kind Apple hardware produces). It keeps separate counts of active
devices and active admin devices, refuses to revoke the last device ever and
the last admin device unless a recovery authority is configured, and replaces
the entire key set only through a recovery flow with a mandatory three-day
delay that any admin key can veto. `BuilderAccountFactory` deploys such
accounts at addresses predictable before deployment. `ShotRegistry` is a
neutral public witness where a controller can register a Shot (an app's
permanent identity) using a commit-then-reveal scheme hardened against
observers replaying or resetting other people's commitments, then append
hash-chained checkpoints and transfer control. It accepts any contract that
answers the standard signature-check interface — it deliberately does not pin
one account implementation, so a future account fix does not strand the
registry. `P256Verifier` wraps the EIP-7951 P-256 precompile and fails closed:
a chain without the precompile looks identical to an invalid signature, so
deployment anywhere requires a live probe of the actual target chain proving
correct verify results and the exact 6,900-gas cost.

## The two contract generations

The v0.7 contract generation shipped inside the 0.7.0/0.7.1 release artifacts
as sources, ABIs, and predicted addresses. It was never deployed, was
superseded after security review, and will never be deployed. Main's
deployment and release-build commands for it fail closed on purpose.

Generation 0.8.0 is the remediated successor: the sources currently in
`contracts/src/`, with a reproducible build definition committed at
`contracts/generations/0.8.0/generation.json`. It is a build definition only —
inactive, undeployed, and identifying no chain state. Before it could ever be
activated, all of the following must happen, none of which has: an independent
security audit; creation of a release-authority policy (a trust root, which is
deliberately not committed to this repository); a threshold-signed activation
record binding the generation digest, the chain, observed addresses and
runtime code hashes, the deployment transactions, and a canonical activation
block; and a fresh EIP-7951 probe of the actual target RPC immediately before
broadcast. Until such a signed activation exists, new secure public identity
creation and all public signing in the engine fail closed.

## What v0.7 retirement means for someone who already installed

If an installed 0.7.x CLI shows a predicted BuilderID or account address, that
prediction belongs to the retired generation and will never exist on-chain. It
is not a public identity, not ownership evidence, and not a future deployment
coordinate; the CLI now says so whenever it prints one. Everything local
remains valid: private Shots, identities, and signed history still verify
offline against the frozen v0.7.1 release inputs, which are kept byte-for-byte
in the repository and at the v0.7.1 tag for exactly that purpose.

## Capability policy for generated apps

Generated apps are local-first by construction, and the engine scans their
source to prove it. Local notifications are now the one supported protected
Apple capability: an app that uses the UserNotifications framework gets an
exact declaration of that use in its signed record instead of being rejected.
Everything else still fails the gates — camera, microphone, location,
contacts, health, Bluetooth, StoreKit, explicit entitlement files, any network
use, cloud storage or sync, tracking and analytics, and any storage runtime
other than the small declared set (files, user defaults, keychain, Secure
Enclave, SwiftData). A source using notifications plus a forbidden capability
is rejected with a diagnostic naming only the forbidden part. One helper
function in `engine/src/protocol_lifecycle.rs` decides what is supported, and
the record creation path, the local conformance check, and the independent
verifier all derive from it, so they cannot drift apart; the derived
declaration is deterministic regardless of where in the source the capability
appears.

## Deliberately deferred

Public deployment and everything downstream of it (durable public BuilderIDs,
the public witness registry, publication receipts) wait on the audit and
activation chain described above; the project treats an unaudited deployment
as worse than none. Network-capable generated apps wait until exact endpoint
declarations can be proven rather than trusted. The other protected Apple
capabilities wait until each has a declaration and policy as tight as the one
notifications got. Device-key replacement for the frozen v0.7 identities is
closed — the successor generation's recovery design (ADR 0006) is the answer,
and no signed identity-supersession flow will be built until a real migration
needs one.

## Half-finished or worth knowing

The notice that the v0.7 generation is retired exists in the repository
(`release/V0_7_CONTRACT_GENERATION_NOTICE.md`) but still has to be added by a
release operator to the already-published external v0.7 release notes; the
repository cannot do that itself. The branch
`archive/codex-0.8-cutover-pre-remediation` (now also on origin) preserves an
abandoned pre-remediation draft of the 0.8 cutover for the record only — it
must not be merged. Engine test coverage rejects a representative forbidden
capability (camera) through the shared gate rather than testing every
forbidden capability by name; the gate itself is a single code path. The
sealed Apple Fascia artifact still labels itself candidate 0.7.0 by design —
sealed artifacts are never edited in place; its next accepted revision is
expected to add generation-scoped publication-receipt verification.
