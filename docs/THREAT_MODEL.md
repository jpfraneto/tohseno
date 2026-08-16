# TOHSENO threat model

This document applies to the coherent-intention lineage protocol, the Local
Workspace Service, Studio, the private Companion channel and relay, the local
Apple factory, portable Shot bundles, protocol nodes, and the optional public
contracts. It describes security claims the implementation can test. It does
not treat an LLM, a device, a relay, a node, a chain, or a signed artifact as
inherently trusted.

## Assets and trust boundaries

Protected assets:

- authority to create and continue a Shot;
- exact original intention and private references;
- accepted Shot genome and its revision history;
- expression source, artifacts, and immutable version records;
- private feedback, attachments, plans, and agent material;
- Builder and installation private keys;
- Companion recovery words, device private keys, capability grants, mailbox
  credentials, private command provenance, and durable outboxes;
- availability and integrity of the Local Workspace Service command journal;
- integrity and causal order of signed lineage and public-checkpoint evidence;
- honest artifact availability and verification evidence.

Principal boundaries:

- browser draft ↔ encrypted relay ↔ local claim CLI;
- human owner ↔ local CLI or Studio;
- browser page ↔ loopback-only Local Workspace Service;
- iPhone Companion ↔ content-blind Companion Relay ↔ Local Workspace Service;
- private companion authorization ↔ canonical Builder-authorized engine action;
- immutable installed release ↔ stable launcher ↔ user LaunchAgent;
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

### Local Workspace Service and private Companion channel

Threat: a malicious or altered QR selects an attacker relay, substitutes
Studio keys, carries a bearer secret, or is replayed after the owner intended
one device to pair.

Controls: the URI and payload schemas are versioned and strictly decoded; the
invitation is signed by the durable Studio identity; the relay is selected by
an allowlisted identifier rather than scanned URL; public-key possession is
proved on both sides; session identifiers are unguessable; invitations last
120 seconds with only a documented bounded clock-skew allowance; and a session
has atomic pending, completed, expired, or cancelled state. The first valid
completion consumes it. The QR contains no recovery phrase, private key,
Builder secret, source, path, permanent mailbox credential, or permanent
bearer token. A standard QR and preserved quiet zone avoid reliance on custom
visual decoding.

Threat: the Companion Relay is compromised, malicious, subpoenaed, or backed
up after deletion, and tries to read content, alter a command, forge a wake-up,
or execute work.

Controls: pairing grants and mailbox payloads are recipient-specific
ChaCha20-Poly1305 ciphertext; senders sign canonical outer envelopes with
Ed25519; X25519/HKDF key agreement binds explicit versioned domains; the Mac
verifies the device key and current capability before command admission. The
relay receives no content key, prompt, feedback, marketing text, Shot name,
icon plaintext, source, filesystem path, recovery words, or harness output. It
has no application-service, harness, build, or node credentials and exposes no
code-execution route. A retained ciphertext backup remains traffic-analysis
material, so the operator may still observe timing, approximate size, expiry,
and minimum opaque routing identifiers.

Threat: mailbox or pairing-session enumeration reveals stable identities or
lets an attacker read, write, acknowledge, revoke, or register push tokens.

Controls: IDs and independent 32-byte capabilities are opaque and unguessable;
the relay stores capability verifiers rather than bearer values; read, write,
acknowledgement, revocation, and push authority are separate; response shape
does not disclose private record presence; requests and stored bytes are
bounded by global, mailbox, and source rates. Logs abbreviate or omit stable
identifiers and never combine a remote IP with a stable device identity.
Upstream volumetric protection remains an operator duty.

Threat: an old, reordered, duplicated, or cross-mailbox envelope causes a
second feedback record, Shot, evolution, or execution.

Controls: signatures bind the schema, envelope UUID, mailbox, sender and
recipient device IDs, sender sequence, exact timestamps, ephemeral key, nonce,
and ciphertext. Decryption binds the same canonical associated data. The
receiver enforces per-sender monotonic replay windows, expiry, recipient, and
mailbox; the relay rejects duplicate envelope IDs. More importantly, every
decrypted command carries a stable command ID that is the durable local
journal's idempotency key. Five deliveries therefore return one stable receipt
and cause one semantic action. Delivery acknowledgement and command
acknowledgement are distinct.

Threat: cursor retention, a lost acknowledgement, reconnect, or an iOS
background suspension leaves the phone permanently divergent or causes it to
guess state.

Controls: each mailbox has a cursor; events have stable IDs and sender
sequences; both peers keep durable encrypted outboxes until acknowledgement.
Before relay expiry, the phone reseals the same canonical signed command and
reference chunks with fresh routing metadata; it does not reinterpret or
re-sign owner intent. Commands older than the canonical thirty-day admission
window remain encrypted locally for explicit resolution rather than being
silently changed or discarded, while inbound acknowledgements and revocation
still reconcile. Every launch reconciles; gaps or an expired retention range
request a complete versioned snapshot instead of applying guessed deltas.
Backoff is bounded and foreground/launch reconciliation remains functional
without APNs. “Always
synced” does not claim an indefinitely alive iOS WebSocket.

Threat: a stolen phone or copied Companion phrase submits commands, while a
restored or previously revoked phone silently regains access.

Controls: the phone's signing and agreement secrets use the strongest
practical Keychain accessibility and recovery words are revealed only by an
explicit future UI action. Capabilities are workspace-scoped, device-bound,
signed, and carry a revocation epoch. The Mac checks current device state and
capability action before every command and event, not only during pairing.
Revocation advances the epoch and stops new snapshots, commands, and event
delivery. The BIP-39 phrase deterministically restores only Companion identity;
it is not a Builder key, wallet, or authority over historical Shots and does
not restore a capability. A restored or revoked phone must complete a new
owner-created pairing session.

Threat: a malicious website attacks Studio through cross-origin requests, DNS
rebinding, permissive CORS, a guessed localhost port, or oversized HTTP input.

Controls: the service binds only `127.0.0.1` and optionally `::1`, never
`0.0.0.0`; validates expected Host and the exact issued loopback Origin;
requires an unguessable anti-CSRF token for mutation; accepts bounded JSON
bodies under exact content types; bounds headers; and emits no permissive CORS
policy. Mutations are not simple cross-origin requests. The phone never calls
this API, so relay availability does not justify weakening the loopback
boundary. Local malware running as the user remains inside the trusted Mac
account boundary and can attack local plaintext by other means.

Threat: an unsafe app, service, journal, SDK-vendoring, icon, reference, or
release path traverses a parent, follows a symlink, aliases another name, reads
a device, or overwrites an unrelated file.

Controls: roots are absolute and installer-owned; every user-controlled read
is bounded and requires an unchanged regular file; path components, symlinks,
special files, traversal, and Apple-case collisions are rejected; journal and
note publication use create-new or atomic same-filesystem replacement; icon
decoding and dimensions are bounded. SDK vendoring refuses a populated target
or symlink and emits an exact SHA-256 inventory. Uninstall validates its
LaunchAgent ownership marker and never recursively follows an app or service
path.

Threat: a service update is substituted, partially published, unhealthy, or
rolled forward while preserving attacker-controlled launch configuration.

Controls: the installer downloads only immutable versioned artifacts and a
manifest, verifies checksums and reported versions before publication, stages
under `~/.tohseno/releases`, atomically changes `current`, and uses the stable
installer-owned launcher in a recognized user LaunchAgent. The installed
updater accepts only an immutable, non-draft GitHub release, binds the
installer and aggregate manifest to GitHub's SHA-256 asset metadata, verifies
`oneshot.sh` through `SHA256SUMS`, and rejects redirects outside the exact
GitHub release-asset host allowlist. It never assembles launchctl or shell
arguments from user input. New service health and exact version are checked
after restart; failure restores the prior pointer and restarts it. Service
state, command journals, app folders, Builder identity, and pairing state are
outside release payloads. The public installer pin cannot move until published
bytes and checksums are independently verified.

Threat: a crash occurs after a compound command records Feedback but before it
starts Evolution, or after a semantic action but before its receipt is
published.

Controls: the command request is immutable and durable before validation or
execution; state transitions publish atomically; per-operation markers record
compound progress; deterministic IDs let recovery inspect existing canonical
Feedback and execution records before performing a missing step. Recovery
returns the same receipt and never adds a second Feedback or evolution. Raw
filesystem state is not used as proof that an action completed.

Threat: a mobile evolution or Feedback request was composed against an older
Version and is admitted after the current Expression changes.

Controls: private commands sign the Shot ID, exact Expression ID, Version ID,
and ordinal. The application service revalidates that exact accepted base at
admission. Feedback remains attached to the reviewed Version; an evolution is
rejected as stale rather than silently rebased. Explicit “EVOLVE FROM THIS” on
a currently capable device is sufficient authorization but cannot bypass the
base check.

Threat: an attacker uses oversized encrypted envelopes, reference descriptors,
icons, command bodies, or a backlog to exhaust the relay, phone, or Mac.

Controls: limits exist at the HTTP body, envelope, ciphertext, mailbox record,
cursor page, snapshot, icon, reference, journal, retention, and global-storage
layers; declared sizes are checked before allocation where practical; payload
schemas reject unknown fields; and rate/capacity exhaustion fails closed. The
relay cannot evade inner application limits because decryption is followed by
the same bounded application-service validation used by CLI and Studio.

Threat: APNs tokens or push payloads expose identity or private state, missing
credentials silently disable production delivery, or a forged push is treated
as a command.

Controls: push tokens are stored only behind the relay's independent push
capability and omitted from content logs. A push contains no content or Shot
identifier; it only asks the app to reconcile its authenticated mailbox and
therefore grants no command authority. Production startup fails when APNs is
declared enabled but its team, key, topic, environment, or private-key path is
missing or malformed. No-op and fake providers are explicit modes; CI and
foreground reconciliation need no Apple credential.

Threat: Studio or the Companion leaks raw prompts, source filenames, source
code, credentials, model output, or harness transcripts through snapshots,
events, JavaScript, relay records, logs, metrics, or crash reports.

Controls: workspace snapshots are allowlisted projections containing stable
Shot and accepted-Version identity, bounded encrypted icon material, supported
actions, and privacy-safe execution summaries. Status events use phase names
and structured receipts only. The service's operational logger omits prompts,
unnecessary private paths, full relay IDs, tokens, ciphertext, non-public
digests, source filenames, and harness output. Harness authentication remains
on the Mac and never enters Studio JavaScript or companion payloads.

Threat: a private companion command or marketing note is accepted by the
Public Node or changes public authority.

Controls: companion identity and Builder identity are separate. The Mac
preserves private device provenance, verifies capability, then uses the
existing Builder authority for any canonical engine action. Companion
envelopes, capability grants, marketing notes, snapshots, and private command
records are outside ordinary lineage; `tohseno-node` retains its closed
eligible-action validation and rejects private actions. Pairing cannot rotate
Builder identity, publish a checkpoint, deploy a contract, or perform a token
action.

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

Taking a Shot starts the selected harness immediately in its supported
non-interactive mode and deliberately supplies that adapter's permission-bypass
flag. There is no Terminal Enter or per-tool approval boundary. This grants the
harness broad local execution authority and is a material risk of the
unattended product contract. Controls are the visible isolated Shot folder,
exact Intention and reference digests, Builder binding, durable execution
records and private logs, structural dependency/capability/anatomy gates,
independent Release tests, protocol verification, bounded repairs, and
delivery of the exact candidate before Version acceptance. Those controls
constrain what TOHSENO will accept; they do not make arbitrary agent execution
safe. Builders should not take Shots from untrusted intentions or references
on a machine containing sensitive material the selected harness could reach.

This unattended authority is local and does not authorize publication,
contract deployment, payments, token creation, recovery administration, or
other irreversible external actions. Their existing explicit confirmation and
fail-closed boundaries remain independent.

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
- Never log or export Companion recovery words, mailbox capabilities, APNs
  tokens, ciphertext, or raw harness output.
- Treat a Companion revocation as immediate local authorization state; do not
  wait for relay or push confirmation before refusing the device.
- Keep Studio loopback-only. Do not expose it through a tunnel, public proxy,
  router port, permissive CORS rule, or arbitrary Host alias.
- Keep the production Companion Relay disabled until durable storage, rate and
  capacity bounds, cleanup, health monitoring, release availability, and
  independently verified installer support are recorded.
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

- independent review of the private companion canonicalization, pairing,
  capability, envelope, relay, and Swift Keychain paths before broad rollout;
- operational abuse and traffic-analysis review of the production Companion
  Relay after a bounded canary, without adding content-bearing telemetry;
- canonical offline BuilderAccount device/transfer/recovery proof reduction;
- independent security audit of contracts and node transport;
- authenticated/encrypted peer transport beyond bounded eligible-lineage
  evidence replication;
- sandboxed materialization for untrusted templates and references;
- key-compromise reporting and revocation UX;
- privacy analysis for any future closed public projections.
