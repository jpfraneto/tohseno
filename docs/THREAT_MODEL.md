# TOHSENO threat model

This document applies to the coherent-intention lineage protocol, the local
Apple factory, portable Shot bundles, protocol nodes, and the optional public
contracts. It describes security claims the implementation can test. It does
not treat an LLM, a node, a chain, or a signed artifact as inherently trusted.

## Assets and trust boundaries

Protected assets:

- authority to create and continue a Shot;
- exact original intention and private references;
- accepted Shot genome and its revision history;
- expression source, artifacts, and immutable version records;
- private feedback, attachments, plans, and agent material;
- Builder and installation private keys;
- integrity and causal order of signed lineage and public-checkpoint evidence;
- honest artifact availability and verification evidence.

Principal boundaries:

- browser draft ↔ encrypted relay ↔ local claim CLI;
- human owner ↔ local CLI or Studio;
- local engine ↔ coding agent, templates, dependencies, Xcode, and build tools;
- generated repository ↔ public export;
- local record store ↔ peer node;
- node ↔ untrusted network and peer nodes;
- off-chain lineage ↔ optional chain anchor;
- Shot identity ↔ optional token relation;
- current schema ↔ legacy records and imported bundles.

## Claims

TOHSENO can prove that exact bytes satisfy a supported schema, have a specific
digest, were signed by a presented key, form a deterministic causal sequence,
and passed recorded verification checks. Where an authority proof is
available, it can prove that the signer was authorized for that action.

TOHSENO cannot prove that an intention is subjectively coherent, that feedback
is sincere, that generated source is universally safe, that unavailable data
exists, or that an authorized owner made a wise decision.

## Threats and controls

### Encrypted web-to-local intention handoff

Threat: script injection at the public origin steals the Browser Draft,
AES-GCM key, or bearer capabilities before claim.

Controls: no external scripts, no inline-script dependency, strict same-origin
CSP, no analytics or service worker, text-only DOM construction, and no
private values in URLs or server-rendered markup. This reduces exposure but
cannot protect a browser already compromised by XSS or an extension.

Threat: a copied `ti1` token is stolen, retained in shell history, replayed,
or sent to a substituted relay.

Controls: three independent random capabilities, single-use claim leasing,
short lifetimes, verifier-only relay storage, no arbitrary origin in the
token, a fixed official HTTPS CLI origin, a debug-only loopback override, and
idempotent local import. Anyone with the token before the legitimate claim can
still import it. The original command may remain in shell history after the
token is expired or consumed.

Threat: chunks are corrupted, swapped, truncated, duplicated, or delivered
across a server restart or expired lease.

Controls: ordered indexes, declared lengths/counts, per-chunk SHA-256,
complete ciphertext SHA-256, authenticated AES-GCM associated data,
idempotent identical-chunk retries, conflicting retry rejection, durable
metadata, and lease expiry back to ready. Browser closure leaves the immutable
transfer state in IndexedDB for retry; the active draft remains the safety
copy. Browser storage can still be cleared by the person or browser.

Threat: a malicious package uses oversized lengths, gaps, overlaps, trailing
bytes, unsafe filenames, path traversal, symlinks, unsupported image bytes,
extension/signature disagreement, or duplicate content.

Controls: both browser construction and the shared Rust parser enforce the
framed schema and conservative web limits; Rust treats every declared length
as untrusted, requires exact contiguous offsets and digests, reuses engine
image-byte validation, and atomically imports only into an opaque private
directory beneath the canonical data root. Roots and records reject symlinks.
The engine's broader local image limit remains unchanged; an oversized web
reference is directed to the local Studio path.

Threat: disk exhaustion, request abuse, cross-origin mutation, path probing,
or arbitrary-origin/SSRF attempts exhaust or redirect the relay and claimant.

Controls: deterministic record/chunk/request bounds, configurable global and
per-source rates, capacity limits, opaque server-generated IDs, no
user-controlled paths, exact browser Origin and Fetch Metadata checks, no
CORS, strict content types, HTTPS production origin, bounded CLI responses,
and opportunistic plus independently runnable cleanup. These controls are not
an abuse-proofing claim; upstream connection and volumetric limits remain an
operator responsibility.

Threat: the relay operator observes private content, deletion fails, or the
server stops during upload/claim.

Controls: the relay receives ciphertext but never the AES key, prompt,
references, filenames, or canonical Shot identity. It may see transport IP,
time, ciphertext size, chunk count, expiry, and state. Completion deletes the
ciphertext synchronously before returning success; expiry also deletes it and
leaves a short-lived metadata tombstone. Infrastructure snapshots or a
malicious operator can retain ciphertext, although it remains undecryptable
without the separately held key. Durable records and recoverable leases
survive ordinary process restarts.

Threat: local disk persistence fails after decryption, a completion response
is lost, or the local machine is compromised.

Controls: the engine validates before writing, uses private permissions,
fsync plus atomic rename, deduplicates by complete package digest, retains the
local package if relay acknowledgement fails, and completes the relay only
after durable import. A compromised local account can read local plaintext;
TOHSENO cannot protect content from the machine authorized to use it.

Threat: the website becomes ready before the public installer release can
claim packages.

Controls: production availability requires explicit relay storage,
`INTENT_RELAY_ENABLED`, HTTPS canonical origin, and the separate
`CLAIM_INSTALLER_READY` assertion. Release ordering is mandatory and public
installer pins are tested. Until activation the browser offers only the
honestly described private file and generic installer paths.

### Forged lineage actions

Threat: an attacker invents an origin, genome acceptance, version, ownership
change, feedback author, or token association.

Controls:

- RFC 8785 canonical signed bytes;
- SHA-256 payload and action commitments;
- low-s P-256 signatures;
- closed action-specific schemas;
- authority and reducer rules per action type;
- exact frozen v0.7 BuilderID derivation only for legacy offline verification;
- activated-generation controller evidence for future public authority;
- deterministic fixture and tampering tests.

Remaining limitation: offline proof of BuilderAccount device rotation,
recovery, or ownership transfer is incomplete until the canonical authority
proof chain is implemented. Such a transition must remain unverified rather
than being accepted by assertion.

### Replay and causal substitution

Threat: a valid action is applied twice, moved to another Shot, placed after a
different head, or replayed against another contract or chain.

Controls:

- Shot ID, lineage sequence, previous action commitment, actor, and payload
  digest are signed;
- reducers reject duplicate commitments, sequence gaps, invalid previous
  references, and disallowed forks;
- public EIP-712 actions bind nonce, deadline, chain ID, contract address, and
  domain;
- imports deduplicate by content commitment without treating duplication as a
  new event.

### Unauthorized genome mutation

Threat: ordinary materialization silently changes hard Shot invariants.

Controls:

- human and machine genome forms share a deterministic digest;
- accepted versions bind a specific accepted genome revision and digest;
- mutations use distinct proposal and acceptance actions;
- the reducer requires current-owner authority;
- evolve fails closed on unaccepted genome drift;
- the native Apple plan fails before declaration when its resolved Organs do
  not provide every required capability or when the Genome commits to a
  platform the iPhone factory cannot satisfy.

### Stolen or rotated owner keys

Threat: a stolen DeviceKey authorizes malicious continuity, or a legitimate
replacement cannot be distinguished from an attacker.

Controls:

- Secure Enclave/Keychain custody where available;
- private keys never enter generated repositories or JSON records;
- BuilderAccount device epochs, permissions, revocation, and recovery nonces;
- visibly test-only software keys cannot authorize a public action; their
  legacy v0.7 identity escape hatch is local-only;
- no secure BuilderID is created for the inactive 0.8.0 generation;
- no CLI rotation claim until the complete proof chain is verifiable.

Recovery can restore authority; it cannot retract already valid historical
actions. Owners must publish rotation/revocation promptly when the supported
path exists.

### Malicious or incomplete nodes

Threat: a peer serves tampered records, omits actions, invents completeness,
exhausts resources, or tries to inject private material.

Controls:

- content-address verification on ingest and read;
- schema, signature, payload, segment, and availability validation before
  storage;
- unresolved authority and missing parents are explicit; no branch is promoted
  into active-generation authority while the node reports
  `active_generation: null`;
- public/replicable availability required for peer ingest;
- explicit partial histories and missing-artifact lists;
- bounded request, action, response, and file sizes;
- no code execution during record synchronization;
- atomic create-new storage and symlink rejection;
- derived indexes rebuildable from the content store;
- per-peer errors do not invalidate locally verified records.

A node can withhold data or go offline. Replication improves preservation but
does not prove global completeness.

### Tampered portable bundles

Threat: a downloaded directory alters records, replaces artifacts, hides
omissions, or claims ownership transfer.

Controls:

- manifest commits every included payload file by normalized relative path,
  length, and SHA-256;
- import rejects symlinks, traversal, collisions, duplicate paths, unknown
  required schemas, and digest mismatches;
- all lineage is reverified before persistence;
- omissions and availability are explicit;
- import never adopts ownership or executes materialization;
- private attachments require a separate explicit local action.

The canonical bundle manifest is an integrity inventory, not an owner
signature. Signed lineage authenticates canonical actions, but transport
authentication for the bundle as a whole still requires a trusted channel or a
separately communicated manifest digest. The current candidate also omits
expression source and retained build artifacts and marks imported bundles as
not ready for materialization.

### Schema downgrade

Threat: an attacker presents an older or unknown schema to bypass a newer
authorization or validation rule.

Controls:

- protocol name, protocol version, schema identifier, and schema version are
  signed;
- unknown major versions and unsupported action types fail closed;
- compatibility adapters are explicit and version-specific;
- a v1 adapter preserves the v1 meaning instead of parsing it as a looser v2
  object;
- migrations never re-sign historical bytes.

### Chain and contract mismatch

Threat: valid coordinates or signatures are presented for another chain,
contract, runtime, or deployment.

Controls:

- EIP-712 domain and live chain ID binding;
- immutable generation definitions bind code hashes and conditional CREATE2
  arithmetic without claiming deployment;
- a future activation must bind target-chain runtime, canonical block,
  transaction evidence, and a fresh complete EIP-7951 probe;
- build definition, signed activation, and client-trusted release policy are
  distinct;
- the current engine rejects a non-null `tohseno.app-metadata/2` registry
  claim while no generation is active; its shipped bare coordinates remain
  compatibility data, not a receipt;
- receipt, runtime, transaction-envelope, and post-state verification;
- no invented or undocumented production address.

### Token-association spoofing and identity collapse

Threat: a token address is presented as the Shot itself, as ownership, or on
the wrong chain.

Controls:

- token association is a distinct signed relation containing Shot ID, target
  chain ID, and token address;
- chain ID `8453` identifies Base explicitly;
- association never alters Shot ID, expression ID, genome, or owner;
- current relation and historical actions/events are distinguishable;
- duplicate/replacement behavior is frozen in lineage reducers and
  interoperability tests.

The relation does not attest token economics, issuer intent, code safety, or
market value.

For example, an Anky Shot may associate `$ANKY` on Base (`eip155:8453`) without
making that token contract the Shot, the Shot controller, TOHSENO itself, any
`$TOHSENO` association, or evidence of token legitimacy. No `$ANKY` address is
asserted by the protocol documentation.

### Leakage of private intention or feedback

Threat: a recursive repository upload, public node action, build resource,
page, log, or bundle publishes private bytes.

Controls:

- private-by-default availability;
- generated ignore rules and a dedicated `.tohseno/private` area;
- public export is an allowlisted projection, not a recursive copy;
- public export includes exact intention material only when its signed
  intention availability is public, and it never relabels private material;
- source and retained artifacts are scanned for raw private-input sequences;
- Shot-level intention, genome working surfaces, feedback, and private agent
  material are excluded from expression source snapshots;
- node ingest refuses local/private actions;
- the registry accepts only the digest of the closed, ancestry-free public
  checkpoint projection plus public coordinates; it never accepts an ordinary
  lineage digest as a head.

Digests can reveal equality and enable guessing of low-entropy private input.
No contract path may construct a checkpoint from an intention, feedback,
installation identity, runtime continuity data, end-user data, or hashes of
those values. Owner confirmation does not turn a guessable hash into privacy.

### Malicious references and paths

Threat: a dragged file, attachment, archive, repository path, or peer record
escapes its root, follows a symlink, aliases another Apple-case path, or
overwrites existing material.

Controls:

- canonical roots, regular-file checks, symlink rejection, NFC normalization,
  Apple-case collision checks, bounded file counts and sizes;
- create-new persistence and atomic rename;
- no absolute or parent-traversal bundle paths;
- the Apple factory accepts at most eight explicitly supplied references per
  intent, reads at most 64 MiB from each unchanged regular file, rejects
  unsafe or case-colliding names and duplicate content, and stores exact bytes
  privately at `.tohseno/references/<sha256>`;
- initial binary references enter the signed Intention as descriptor-only
  `OriginalMaterial` values; their bytes are never inlined in lineage;
- pending evolution Feedback and artifact references share one exact-prompt
  binding, and every local reference is rehashed before the
  `EvolutionaryIntent` is signed;
- references are data until explicitly selected for a materializer.

### Arbitrary code execution during materialization

Threat: source, an agent, template, organ, build phase, dependency, or imported
bundle executes attacker-controlled code.

Controls:

- verification and import never execute material;
- materialization is a separate explicit local boundary;
- Apple project anatomy rejects executable build scripts and undeclared
  runtime containers;
- fixed normative Fascia sources and commitment;
- dependency and sensitive-capability checks;
- generic iOS compilation, retained artifact inspection, and conformance;
- received Shots must be verified before any supported materialization.

Prepared Shot execution launches the selected harness only after the person
presses Enter in its authentic terminal interface. TOHSENO adds no
permission-bypass flag: the harness retains its native questions, approvals,
and permission controls. The harness can still receive broad local authority
when the person grants it, so builders should use isolated workspaces for
untrusted intentions or references.

### Dependency and organ substitution

Threat: a capability declaration resolves to different code or permissions on
another machine.

Controls:

- capability locks bind each full Organ declaration: identity, provided
  capability, owned state, dependencies, permissions, events, Genome
  constraints, supported platforms, and acceptance-test text;
- canonical graph ordering and hashing are deterministic, and both
  VerificationResult and Version must match the exact graph present at their
  actions;
- every Organ acceptance test has a distinct signed gate whose name commits
  the test text, preventing changed wording from inheriting an old result;
- a graph transition must be declared with `organ` change scope;
- Organ records do not claim to identify implementation source. The pinned
  Apple Fascia source commitment and Version source digest bind code through
  separate, explicit mechanisms;
- substitution therefore requires new declarations, verification, and an
  accepted Version.

### Divergent derived state

Threat: `shot.json`, `app.toml`, a node index, or Studio view disagrees with
canonical history.

Controls:

- signed actions and immutable v1 records are canonical;
- snapshots carry lineage head and source action references;
- verification reconstructs state from the action stream;
- node integrity can rebuild indexes;
- cache disagreement fails verification and can be repaired without rewriting
  history.

## Operational guidance

- Never store Builder, recovery, or installation private keys in a Shot.
- Do not commit `.tohseno/private` or private feedback attachments.
- Verify a portable Shot before inspecting artifacts or materializing source.
- Treat absence and partial replication as normal, not as validation failure.
- Never construct a public checkpoint or on-chain record from a genome,
  intention, feedback, private material, or a digest derived from those values.
- This source tree has no deployment command. Do not deploy these contracts.
  Any future workflow requires separate review, exact human authorization, and
  the actual-target EIP-7951 hard gate immediately before broadcast.
- Preserve failed materialization evidence privately; never advance canonical
  version state on failure.

## Deferred security work

- canonical offline BuilderAccount device/transfer/recovery proof reduction;
- independent security audit of contracts and node transport;
- authenticated/encrypted peer transport beyond bounded eligible-lineage
  evidence replication;
- sandboxed materialization for untrusted templates and references;
- key-compromise reporting and revocation UX;
- privacy analysis for any future closed public projections.
