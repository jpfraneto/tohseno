# TOHSENO threat model

This document applies to the coherent-intention lineage protocol, the Local
Workspace Service, Studio, the private Companion channel and relay, the local
Apple factory, portable Shot bundles, protocol nodes, and the optional public
contracts, signed public catalog, immutable source service, and constrained
generation-0.8 relayer. It describes security claims the implementation can test. It does
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
- the Companion-held non-exportable Builder DeviceKey and signed public
  release/profile/alias authorization;
- exact public source bytes and recipient-local build/sign/install evidence.
- Claim Edition immutability, one-account/Shot uniqueness, exact encounter
  release/checkpoint binding, normalized Claim-mark privacy, and canonical
  non-transferable receipt evidence;
- private Follow preferences and high-signal Updates read state.

Principal boundaries:

- browser draft ↔ encrypted relay ↔ local claim CLI;
- human owner ↔ local CLI or Studio;
- signed native Mac app ↔ native helper ↔ loopback Local Workspace Service;
- browser page ↔ loopback-only Local Workspace Service;
- iPhone Companion ↔ content-blind Companion Relay ↔ Local Workspace Service;
- private companion authorization ↔ canonical Builder-authorized engine action;
- immutable installed release ↔ stable launcher ↔ user LaunchAgent;
- local engine ↔ coding agent, templates, dependencies, Xcode, and build tools;
- local service ↔ TOHSENO managed proxy ↔ Bankr/upstream model provider;
- native owner ↔ Stripe hosted Checkout ↔ append-only creation-balance ledger;
- generated repository ↔ public export;
- local record store ↔ peer node;
- node ↔ untrusted network and peer nodes;
- off-chain lineage ↔ optional chain anchor;
- Shot identity ↔ optional token relation;
- current schema ↔ legacy records and imported bundles.
- Builder Companion ↔ Mac publication job ↔ constrained relayer;
- on-chain Registry witness ↔ signed off-chain catalog and immutable blob;
- public link ↔ recipient Companion request ↔ independent Mac verification ↔
  recipient Xcode and physical iPhone.
- Companion Claim gesture/DeviceKey ↔ constrained Claims relayer ↔ additive
  Claims contract ↔ canonical Claims index;
- public Discover events ↔ private local Following/Updates projections.

## Assurance claims

TOHSENO can prove that exact bytes satisfy a supported schema, have a specific
digest, were signed by a presented key, form a deterministic causal sequence,
and passed recorded verification checks. Where an authority proof is
available, it can prove that the signer was authorized for that action.

TOHSENO cannot prove that an intention is subjectively coherent, that feedback
is sincere, that generated source is universally safe, that unavailable data
exists, or that an authorized owner made a wise decision.

## Person-to-person invariants

1. No release is discoverable without a valid Companion-held Builder DeviceKey
   signature and current BuilderAccount authorization.
2. Neither Mac nor server can forge a Builder publication.
3. Companion commands carry only bounded ShotID/release/action facts; the Mac
   resolves the official release independently and never executes a supplied
   arbitrary URL.
4. Signed artifact SHA-256, bounded extraction, and recomputed source-tree
   commitment prevent catalog substitution and path escape.
5. A release becomes visible only after exact transaction receipt, canonical
   block, Registry event, current head/sequence, and public checkpoint agree.
6. Public checkpoints never contain source, intention, artifact, private
   lineage, installation, or end-user facts.
7. One-tap automatic build is limited to the Green profile. Named review cases
   need explicit local review; unsupported cases never enter `xcodebuild`.
8. Apple credentials stay in local Apple tooling. Installed requires exact
   bundle inventory on the intended physical iPhone.
9. Refresh changes local signing/provisioning only and creates no release or
   Registry action.
10. Aliases cannot change Shot identity. Fork gets a new random ShotID and
    never reuses parent authority.

## Software Claim invariants

1. Generation-0.8 Factory, BuilderAccount, Registry ABI, bytecode, and signed
   activation remain unchanged. A Claims deployment is not a Registry
   generation and is untrusted without its separate threshold-signed evidence.
2. Clients require exact chain, Claims address, constructor-bound active
   Registry, runtime hash, deployment receipt/block, source commit/tree, policy
   digest, activation digest, and canonical live rechecks. An environment
   address or operator database cannot activate Claims.
3. Every Shot opens at most one immutable edition, only after the Shot exists
   and only through its current Registry controller's exact ERC-1271
   authorization. Updates cannot reopen, extend, reset, or edit it.
4. One deployed Tohseno account may Claim one Shot once. The signature binds
   chain, Claims contract, Registry, Shot, claimant, exact release, current
   checkpoint, normalized gesture commitment, nonce, and deadline.
5. Changed Registry head, stale nonce, expired action, invalid account,
   exhausted supply, elapsed deadline, duplicate Claim, and noncanonical
   receipt all fail closed. Chain ordering, not a server reservation, allocates
   the number at a finite boundary.
6. Transfers, approvals, arbitrary recipient callbacks, owner mint, policy
   administration, upgrades, and marketplace behavior are absent. A defect
   requires a truthful successor or abandonment.
7. The relayer persists one allowlisted job before submission, retries it
   idempotently, rate-limits source and global preparation, and never receives
   a DeviceKey. A pending or failed transaction never increments canonical
   state.
8. Raw touch samples, timestamps, pressure, velocity, motion, and behavioral
   inference are neither signed nor persisted. Only 64 normalized fixed-width
   points or the explicit accessibility representation remain, and the human
   action is not treated as entropy or identity proof.
9. A Claim publicly links a Tohseno address to a Shot and exact encounter;
   Companion discloses that once. It never publishes IP, Mac, physical iPhone,
   Apple identity, pairing, source path, intention, or install evidence.
10. Canonical Claim confirmation may enqueue the existing private exact-release
    preparation. It cannot claim build, signing, cable, or physical install
    success; the recipient Mac independently verifies every release fact.
11. Public timelines derive from canonical block/transaction/log order and are
    reorg-aware, idempotent, and bounded to one Ship plus Updates/forks/edition
    closure. Individual Claims never become Discover spam.
12. Follows and Updates remain private encrypted preferences/evidence keyed by
    exact stable IDs. Generic feed events and individual third-party Claims
    never enter the private inbox, and handles do not become preference keys.

### Public catalog and executable-source boundary

Threat: Mac or server alters a release, substitutes bytes, exposes staged
source early, or invents a receipt.

Controls: Companion signs the canonical closed release, including source
digest/tree/build recipe/permissions and exact checkpoint. Staging uses opaque
capability, owner-only atomic storage, exact declared length, streaming digest,
short TTL, cleanup, and record/byte/rate limits. Finalization independently
checks the successful active-Registry receipt, block, event, current head and
authorized DeviceKey before atomically promoting content-addressed bytes. The
server revalidates canonical block and live authority before public discovery,
so a reorg or revoked DeviceKey removes stale evidence from normal reads. The
publishing and receiving Macs repeat static, chain, receipt, runtime-code, and
source verification rather than trusting the server's completion response.

Threat: downloaded Xcode source executes arbitrary build-time code or escapes
its destination.

Controls: deterministic tar permits only normalized bounded regular files and
directories; secret paths/content, symlinks, hard links, special files,
traversal, collisions, and excessive trees fail closed. The Green classifier
requires an ordinary app target and refuses scripts, build rules, unverified or
unpinned package/plugin behavior, custom executables, extensions, and
unsupported entitlements. Signed dependency-lock digests are recomputed from
the extracted snapshot before any build. Source remains visible for review; no
downloaded source is silently rewritten.

Threat: profile or scarce alias becomes an alternate identity or hoarding
primitive.

Controls: profiles are closed, canonical, low-s P-256 signed, bound to current
BuilderAccount key authority, and monotonic by nonce. Builder handles are
unique metadata; app slugs are unique only inside one Builder namespace.
External attestations fail closed until an official provider verifier is
configured and therefore cannot be self-asserted. Global alias requests require
an existing installable Shot, current Builder authorization, expiry,
replay-resistant request ID, rate limits, audit storage, and explicit policy
review. Canonical `/s/<ShotID>` resolution never depends on an alias.

## Threats and controls

### Native Mac client, bundled installation, and intelligence

Threat: an arbitrary local process, malicious origin, replayed token, or
substituted app claims native mutation authority.

Controls: the helper verifies that both itself and its parent chain to the same
Apple-anchored release Team ID, and requires the parent's exact
`com.tohseno.mac` identifier. Copying the helper into another team's signed
bundle therefore does not transfer native authority. The workspace key proves
a one-use 30-second challenge; the service returns a
15-minute token bound to the current service instance, native client ID, and
explicit scopes. Native tokens are not browser CSRF tokens. Browser routes
retain exact Host/Origin/CSRF enforcement. Debug-only unsigned behavior is not
compiled into the release trust decision. Malware already executing as the
same user can still attack local plaintext and Keychain prompts by other means.

Threat: the DMG or nested factory is replaced, partially installed, rolled
forward without recovery, or used to erase prior apps/state.

Controls: Developer ID signs nested executables before the outer hardened app;
notarization and stapling are external release gates. First open accepts an
exact sorted SHA-256 manifest with no symlinks or special files, reads files
without following a final symlink, stages an immutable release, atomically
publishes stable programs/current selection, and restores the prior selection
if any activation step fails. Existing app folders, command journals,
identities, entitlements, and Companion state are outside the bundle. The
website download is disabled until an HTTPS DMG URL and independently verified
digest are both configured. No automatic update feed is active.

Threat: a custom command injects a shell, a local endpoint impersonates a
remote service, credentials leak into arguments/environment, or attachments
escape their intended roots.

Controls: custom executables must be absolute executable regular non-symlink
files and receive only bounded literal arguments without a shell. Local
OpenAI-compatible endpoints are explicit loopback HTTP origins with bounded
model discovery, recorded consent, and optional Keychain bearer retrieval.
Sensitive inherited environment names are removed. References are limited to
eight bounded PNG/JPEG regular non-symlink files, validated by exact bytes and
copied to durable private command inputs. These checks do not make an
owner-selected executable or local model trustworthy.

### Managed compute, Stripe, and creation balance

Threat: a stolen Bankr/Stripe/operator secret reaches a Mac, generated app,
repository, process argument, log, crash report, or release bundle.

Controls: secrets exist only in the website secret manager. The native and
Rust clients hold an installation signing key and short-lived admitted
capability, never the provider key. Release verification scans all bundled
bytes for forbidden secret patterns; local harness environments remove known
sensitive names; server access logs use semantic routes and omit bodies and
identifiers. Secret-manager and host compromise remain operator risks requiring
rotation and incident response.

Threat: a forged installation, replay, price change, model substitution,
privacy downgrade, oversized request, or general-purpose proxy call spends
operator credit.

Controls: Ed25519 claims self-bind an opaque derived installation, action,
exact request digest, issue/expiry, and unique claim ID. The server admits only
the narrow completion route, allowlisted live catalog models, advertised
privacy tier, bounded body/tokens/rate, durable command/execution IDs, and an
explicit maximum. Pricing is server-derived with a timestamp and the local
durable command records the estimate and approved cap. A short-lived one-use
capability covers at most the bounded implementation/repair total and cannot
be reused. There is no silent fallback from local/BYO to paid managed work.

Threat: concurrent reservations, duplicate/reordered Stripe events, forged
redirects, refunds/disputes, promotions, or crashes create spendable value or
double-spend it.

Controls: an append-only integer micro-USD ledger is the sole balance
authority; paid and promotional buckets remain distinct; per-installation
locks serialize reservation decisions; holds reduce spendable balance before
provider use; charges and releases are idempotent. Checkout uses server-known
Price IDs and Stripe idempotency. Redirects carry no credit authority. The
webhook verifies the raw signature and retrieves the authoritative paid
Checkout Session, Price, amount, currency, and payment identity. Refunds and
disputes append compensating entries. Operator grants, revocations, and
reconciliations require a separate hashed bearer and preserve private audit
metadata.

Threat: Bankr returns `401`, `402`, `429`, `5xx`, times out, omits usage, or
reports usage above the reservation; a process dies around provider admission.

Controls: locally decidable failures do not consume a capability. Explicit
authentication/balance/rate failures release holds. Ambiguous outcomes append
a pending reconciliation and retain the hold; operator health exposes the
count, and a protected, idempotent decision either charges no more than the
outstanding reservation or releases it. Expired unused capabilities are
released on the next reservation; a use marker is written under the same
account lock before forwarding. Process death after that marker remains
intentionally ambiguous and must be reconciled against provider usage. There
is no indefinite automatic retry.

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
rejected as stale rather than silently rebased. One explicit **Evolve App** on
a currently capable device is sufficient authorization but cannot bypass the
base check; binding the exact base for the person at submission removes a
choice from the interface, not a check from the service.

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
- generation 0.8.0 is active, but secure BuilderID creation remains
  unimplemented and fails closed;
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
  into active-generation authority merely because the node reports
  `active_generation: "0.8.0"`; live controller evidence is still required;
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
- the current activation binds target-chain runtime, canonical block,
  transaction evidence, and a fresh complete EIP-7951 probe;
- build definition, signed activation, and client-trusted release policy are
  distinct;
- the current engine rejects a non-null `tohseno.app-metadata/2` registry
  claim even though generation 0.8.0 is active; its shipped bare coordinates
  remain compatibility data, not a receipt;
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

## Billing and entitlement boundary

Threats include local trial-file tampering, duplicated success evidence,
checkout substitution, forged or stale webhooks, a receipt copied between
Macs, compromised server signing material, and capability logging.

Controls are a private bounded atomic ledger; distinct-date plus
command/execution/Version idempotency; admission below every UI; short-lived
workspace-signed claims; a fixed billing origin; hosted payment pages; bounded
webhooks with HMAC/timestamp verification; provider-event idempotency; P-256
server-signed canonical receipts; a release-pinned public key; exact derived
installation binding; monotonic receipt revisions that reject replay and
rollback; and fail-closed checkout/refresh when configuration or verification
is absent. The signing private key is never present in source,
native/npm artifacts, Studio, or Companion. Rotation requires a new reviewed
release. The explicit development entitlement is compiled only into debug
builds and must never be set in a production LaunchAgent.

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
- This source tree has no contract-generation deployment command. Do not
  redeploy generation 0.8. Ordinary publication may use only the constrained
  active-generation factory/Registry relayer after exact Companion approval;
  any other broadcast requires separate review and authorization.
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
