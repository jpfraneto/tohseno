# Protocol privacy boundary

TOHSENO is local and private by default. Publication is an explicit,
Builder-authorized transition; a private/local Shot does not enter the public
Builder graph merely because it exists.

The primary native Mac app talks only to the loopback Local Workspace Service
through a short-lived, scoped native session. It does not upload source merely
to list apps, show readiness, or install a deterministic build. Reference
images are copied as exact validated bytes into private command input before
execution; picker paths and symlinks do not become authority.

## Intelligence routes and managed balance

Privacy depends on the explicitly selected route:

- a known local/BYO coding harness receives source under that provider tool's
  own local configuration and provider policy;
- a configured local endpoint must be loopback HTTP, requires recorded consent
  before source is sent, and may use a Keychain credential reference;
- a custom executable runs directly with bounded literal arguments and no
  shell, but is still owner-selected code with the local account's access;
- managed execution sends the admitted prompt/source through TOHSENO's HTTPS
  proxy to Bankr and the selected upstream model provider, only after model,
  privacy tier, estimate, and maximum are shown and accepted.

Managed source and prompts are bounded in transit and memory and are not
written to the TOHSENO server ledger or semantic access logs. The ledger keeps
an opaque installation binding, integer balance movements, command/execution
and reservation identifiers, model, privacy tier, provider request/usage/cost
evidence when available, retail charge, and reconciliation state. Private
operator grant reasons and identities are stripped from the client projection.
Stripe receives the opaque binding and configured pack metadata, not app
source, intention, filenames, Apple identity, or local paths. Stripe and Bankr
retain data according to their actual deployed policies; TOHSENO must not turn
a selected `zdr` or `private` field into a stronger promise than those policies.

The optional web-to-local handoff is a transport boundary, not publication.
Before **TAKE A SHOT**, the Browser Draft remains in browser memory and
IndexedDB. After that explicit action, the browser sends only AES-256-GCM
ciphertext to the temporary relay. The relay never receives the key or
decryptable prompt/reference bytes, and it never creates a Shot. The Mac
decrypts and validates the package into durable local pending state before the
relay deletes its ciphertext. The canonical Shot remains local and is created
only by the engine after local approval.

The operator may unavoidably observe request IP, timing, ciphertext byte size,
chunk count, expiry, and relay state. Incomplete uploads expire after one hour;
ready ciphertext expires after at most seven days and is deleted after durable
import. The copied command contains a high-entropy, single-use bearer token;
anyone possessing it before claim can import the package, and the original
pasted command may remain in shell history after expiry. A downloaded
`.tohseno-intent` package is readable private material, not encrypted. There
are no site accounts, analytics, or model-inference calls during this handoff.

The private Companion channel is a separate persistent transport. The iPhone
and Local Workspace Service exchange recipient-specific signed ciphertext
through opaque relay mailboxes. The relay may observe timing, approximate
ciphertext size, expiry, and minimum routing metadata, but it receives no
plaintext prompt, feedback, marketing note, Shot name, icon, source, path,
recovery phrase, or harness output. It cannot authorize an engine action. The
Mac verifies the current device capability and remains the workspace authority.

Companion snapshots are allowlisted summaries: stable Shot and exact accepted
Version identity, real bounded icons when available, supported actions, and
privacy-safe execution phases. They never contain source code, harness
credentials, transcripts, model output, or local source filenames. Icons and
other private blobs are encrypted before relay upload. APNs payloads contain
only a request to reconcile the authenticated mailbox.

Companion recovery words stay in the iPhone Keychain and are never placed in
QR codes, relay records, snapshots, logs, analytics, UserDefaults, or crash
reports. Restoring the 12 words restores only the device identity, not a
workspace capability or Builder authority. Private companion commands,
capability grants, envelopes, snapshots, marketing notes, and provenance never
enter the Public Node or ordinary canonical lineage.

## Deliberate public source release

Shipping is a separate, explicit disclosure. Before Companion approval the Mac
shows that buildable source will become public. The deterministic snapshot
excludes VCS internals, build products, DerivedData, user data, caches,
environment files, private `.tohseno` state, pairing/relay state, logs, Apple
provisioning/signing material, and known secret paths. High-confidence secret
findings fail closed and report paths without contents. `.gitignore` is not the
privacy boundary.

The public signed catalog release may contain display name/description, ShotID,
BuilderID, release ID/time, source artifact and tree digests/sizes, Xcode
container/scheme/original bundle/minimum iOS/dependency-lock/build-safety facts,
install/fork declarations, optional immutable fork parent, generation, and
public checkpoint binding. It never contains private intentions, references,
prompts, private lineage, absolute paths, local environment, pairing or relay
capabilities, Apple credentials/profiles/certificates/keys, recipient identity,
device ID, Apple team, or install activity.

The operator necessarily observes publisher/reader IP, timing, requested public
Shot/release/blob, and transfer size. Staged bytes are capability-protected and
not publicly readable; they expire before authorization. Published source is
intentionally public and immutable by digest. Install and Fork activity remains
local and is not posted on-chain or written to the public catalog.

Builder profiles are explicitly public and linkable. The profile contains only
the signed display name, granted handle, optional avatar digest, verified
external attestations, update time, nonce, and BuilderID. An X attestation maps
an external account to BuilderID but never replaces DeviceKey authority. Global
alias requests are auditable policy records and canonical Shot links work
without them.

## Private entitlement and billing

This section describes the retained legacy subscription compatibility store.
ADR 0025 removed it from normal native admission and from local/BYO execution;
prepaid managed balance is the separate boundary above.

Genesis progress, the trial anchor, successful-day evidence, subscription
plan/paid-through state, monotonic provider revision, and verified receipt
digest are private machine state. They are never Shot, Version, public-node,
registry, token, or contract input.
Evidence keeps only bounded command/execution/Version links needed for
idempotency; it contains no intention, source, app name, device name, Apple
identifier, recovery material, or payment data.

The Companion receives only an encrypted phase, successful-day count, and
admission booleans. Hosted checkout receives a derived installation binding,
plan, short expiry, workspace public signing key, and signature. It does not
receive apps, intentions, source, device names, Apple identifiers, recovery
words, or local paths. Complete claims, provider payloads, receipts, and
customer/payment data must never enter access logs.

## Never on-chain

No contract action or registry head may be constructed from:

- InstallationIdentity or continuity statements;
- end-user identity, behavior, content, or usage;
- private feedback, references, or attachments;
- raw private intentions or agent material;
- hashes or commitments derived from any of the above.

A hash over a small or guessable private domain is disclosure, not privacy.
Application-runtime code has no contract-publication authority.

## Narrow public witness

The successor registry can expose only:

- independent random Shot ID;
- Builder controller;
- a digest of the narrow `tohseno.public-checkpoint/1` identity-continuity
  projection;
- witness-local checkpoint count;
- action nonce and registration timing.

The public checkpoint starts a separate chain at witness sequence 1. It binds
only its witness generation/chain/registry, ShotID, prior public checkpoint,
fixed scope, and newly declared publication time. It does not contain or
reference the local coherent-intention lineage, expression/version state,
genome, source/build artifacts, feedback, token relations, runtime data,
content, controllers, or free text.

Builder identity is deliberately linkable after explicit publication. End-user
installation identity stays local and unlinkable by default; it is never a
registry controller and the two identity graphs never touch.

The undeployed handle, Appcoin, App Store attestation, `publicState`, and
generic `contentCommitment` contract surface was removed. Token Association is
an optional signed protocol relationship and is not Shot identity or
ownership.

The retired mixed-ancestry public-action outbox is write-disabled. Marking one
ordinary lineage action public does not make its `previous` commitment safe to
disclose. Existing outbox files are preserved as legacy evidence; new Token
Associations remain intentionally private until a closed, ancestry-free public
relation schema exists.

## Immutable Apple Fascia

`fascia/apple/PRIVACY.md` is part of the already committed Apple Fascia tree,
so changing it would change the immutable Fascia digest carried by existing
Shot fixtures. Its v0.7 public-contract list is retained as historical signed
material. This document and ADR 0006 define the successor protocol boundary;
the next accepted Fascia revision must incorporate it through an explicit
versioned migration rather than silently rewriting sealed artifacts.
