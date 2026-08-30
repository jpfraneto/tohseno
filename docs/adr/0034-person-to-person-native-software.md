# ADR 0034: Tohseno makes native software person-to-person

Status: accepted

Date: 2026-08-30

Supersedes:

- ADR 0033 only where it places public distribution, Registry publication,
  public Builder creation, the internet relay expansion required by this path,
  and deployment outside the product boundary;
- ADR 0033 and ADR 0026 only where they make the Registry an optional local
  detail with no production catalog; and
- ADR 0030 only where creation rather than person-to-person shipping is the
  website's central story.

It retains ADR 0033's non-destructive existing-project adoption, exact source
directory, Mac-owned execution, durable authenticated Companion intentions,
Xcode-owned Apple credentials, separation of build from install, physical
device verification for Installed, private local history, and generated-app
factory. It changes no frozen protocol encoding or deployed generation-0.8
contract ABI.

## Context

ADR 0033 connected an existing native app to the Mac project that can evolve
it. The connection stopped at one person's devices. Tohseno already has the
remaining trustworthy organs: a content-blind internet relay, a persistent Mac
factory, an iPhone Companion, protocol-exact P-256 support, an active
generation-0.8 BuilderAccountFactory and ShotRegistry, signed activation
evidence, deterministic local source commitments, Xcode signing, and verified
CoreDevice installation. The product has not connected those organs into a
public source catalog, a Builder-authorized publication, or a recipient-local
build and install path.

Native iPhone software can travel between people without pretending Apple's
security model disappeared. A Builder publishes buildable source and a
cryptographic authorization. A recipient's Mac verifies the exact publication,
builds it with Xcode, signs that local copy with the recipient's Apple identity,
and installs it on the recipient's own development iPhone. This removes App
Store submission and review from this direct path; it does not remove signing,
provisioning, Trust, Developer Mode, Xcode, or Apple's operating-system
boundary.

## Decision

Tohseno is the person-to-person native software network. Existing Xcode
projects and Tohseno-generated projects are both first-class sources of a new
public Shot. Adoption of an existing project begins a truthful new Tohseno root
at the observed source state. It never fabricates earlier Tohseno lineage, Git
history, App Store history, a previous commitment, or a historical Builder.

The Mac is the factory.

The Companion is the human's Tohseno authority.

The Registry is the shared public witness and discovery layer.

A link is how native software travels through the rest of the internet.

Tohseno does not remove Apple's code-signing model.

Tohseno removes App Store submission and review from this person-to-person
path.

A server may transport source and relay transactions.

A server may not impersonate a Builder.

No public Shot publication exists without authorization from the Builder
DeviceKey held by the paired iPhone Companion.

### Three separate truths

The Companion owns identity truth: whether the human authorized the exact
bounded action. Its Builder DeviceKey is a protocol-compatible P-256 key whose
private scalar stays on the iPhone. The Mac, website, Registry, relay, and
transaction relayer receive only public coordinates, key ID, signatures, and
bounded public identity state. Pairing keys, recovery authority, Installation
keys, and Apple signing identities remain distinct.

The Mac owns execution truth: the exact source observed, changed, snapshotted,
built, locally signed, installed, and verified. It remains the only coding and
Xcode execution boundary. Tohseno never commits, pushes, rewrites Git history,
or discards unrelated working-tree state as a side effect of initialization,
evolution, installation, or publication.

The Registry owns public network truth: active-generation ShotRegistry state
plus independently verifiable signed catalog and transaction-receipt evidence.
The operator database is an index, not authority. Security-sensitive clients
read fresh chain state and verify the signed activation rather than trusting
copied coordinates or catalog claims.

### Builder identity and public checkpoints

The Companion creates or loads the Builder DeviceKey using the strongest
compatible this-device-only, non-exportable Keychain or Secure Enclave
mechanism. It signs an already computed 32-byte protocol digest exactly once
and normalizes low-s. Tests may inject a visibly test-only software signer.

A BuilderAccount is created only for the first public Builder action. The Mac
verifies the signed generation-0.8 activation, derives the exact DeviceKey ID
and CREATE2 BuilderAccount address from normative protocol code, and checks
live code and state. The constrained relayer may call the existing factory to
deploy the exact account. A correct front-run deployment is idempotent. A
random EOA is never substituted for BuilderID.

ShotID remains 32 CSPRNG bytes independent of names, paths, bundles, Builders,
tokens, or content. First public registration uses a fresh privately persisted
salt, the exact 60-second minimum delay, and checkpoint 1. Later publication
uses AppendCheckpoint against the exact current head. Checkpoint sequence is
never a local Version, build number, Git count, or app release ordinal.

Every Registry head remains the digest of the closed, ancestry-free
`tohseno.public-checkpoint/1` object. It contains no intention, source,
artifact, private lineage, installation fact, end-user fact, or digest derived
from those values. Generation 0.8.0 stays immutable; no new contract or
contract generation is introduced.

### Signed catalog release

The public service adds the closed off-chain object
`tohseno.catalog-release/1`. Its canonical payload binds the active generation
and witness, ShotID, BuilderID, immutable release ID, publication time, display
metadata, source artifact SHA-256 and byte length, source-tree commitment,
Xcode container and relative path, scheme, original bundle identifier, minimum
iOS, device family, dependency lock facts, build-safety classification,
install and fork declarations, optional exact parent release, expected Registry
checkpoint sequence, and exact public-checkpoint digest.

The release does not contain a transaction hash because it is signed before a
transaction exists. After publication, it is paired with separately verified
receipt evidence. It contains no private intention, references, prompts,
lineage, local absolute path, environment, pairing or relay capability, Apple
credential, provisioning profile, certificate, or private key.

The Companion receives the complete structured payload, rejects unknown or
invalid fields, independently recomputes its canonical digest and relevant
generation-0.8 action digests, presents a bounded human summary, and requires
explicit approval. One first-publication approval may cover only the exact
catalog release, RegisterShot action, and exact account bootstrap facts for
that release. A later approval may cover only the exact catalog release and
AppendCheckpoint. The Companion never signs an opaque Mac-supplied digest or
future release authority.

A release becomes discoverable only after all of these agree:

1. the catalog manifest's Builder DeviceKey signature;
2. current BuilderAccount authority;
3. the declared generation, chain, and ShotRegistry;
4. the transaction receipt and canonical block;
5. the ShotRegistry head and checkpoint sequence;
6. the manifest's exact public-checkpoint digest; and
7. the staged source bytes and declared SHA-256.

Thus the chain proves a Builder-controlled checkpoint, the Companion-signed
manifest binds that checkpoint to one exact software artifact, and content
addressing proves the fetched bytes. Neither the Mac nor server can forge a
Builder publication, and the server cannot substitute source undetected.

### Source publication and safe local build

Publication of buildable source is a deliberate security and privacy boundary.
The Mac constructs a deterministic sanitized snapshot in a temporary owner-only
directory without modifying source. It excludes VCS internals, build output,
DerivedData, user data, caches, environment files, private Tohseno state,
pairing state, logs, Apple provisioning/signing material, and known secrets.
High-confidence secret findings fail closed and name paths without revealing
contents. `.gitignore` is not the security boundary.

Artifact paths are NFC-normalized, sorted, relative, collision-checked, and
bounded. Symlinks, hard links, special files, traversal, oversized files or
trees, and archive ambiguities are rejected. Deterministic metadata and exact
SHA-256 make repeated snapshots byte-identical. A recipient safely extracts to
a new root and recomputes the source-tree commitment before build.

Internet source is executable input. A deliberately narrow Green Install
Profile permits automatic building only for an ordinary native iOS app with
pinned dependencies and no arbitrary Run Script phase, custom executable,
unsafe build rule or package/compiler plugin, unsupported entitlement, or
unsafe archive structure. Anything else says **Requires review on your Mac**
and names the reasons before any `xcodebuild` invocation.

The recipient uses their local Xcode development team. Build-setting overrides
derive a stable recipient-local bundle namespace when the original identifier
cannot be registered; the downloaded source is not silently rewritten.
Capabilities that cannot survive personal re-signing fail with their exact
reason. Installed means devicectl succeeded and the exact bundle appeared in
the intended physical iPhone inventory. Provisioning expiration is observed
from the installed profile and remains visible. Refresh rebuilds and re-signs
the same network release without an AI call, catalog release, or Registry
checkpoint.

### Registry, links, profiles, and transport

The Registry consists of the existing on-chain witness and a small auditable
off-chain catalog. The catalog uses durable operator-controlled storage,
content-addressed immutable blobs, atomic writes, private expiring staging,
strict schemas, bounded requests, rate and capacity limits, and a constrained
transaction relayer. The relayer may submit only exact active-generation
factory, commit, reveal, append, and implemented permissionless-finalization
calls. It is never a generic wallet or transaction endpoint and never holds a
Builder key.

Canonical links use immutable ShotID. Builder-namespaced slugs are unique only
inside a signed Builder profile. Global aliases are server-managed convenience,
not protocol ownership; claims require an existing installable Shot,
Companion authorization, rate limits, audit, and explicit policy approval.
Canonical publication never waits for a scarce alias.

A closed signed Builder profile may contain BuilderID, display name, granted
handle, avatar digest, verified external attestations, and update time. The
Builder DeviceKey authorizes it. External identities such as X attest that an
account maps to a BuilderID; they never replace DeviceKey → BuilderAccount →
BuilderID authority.

Initial physical pairing remains short-lived and authenticated. After pairing,
the content-blind encrypted relay is the normal durable transport while Mac or
phone is away or offline. It sees no source, plaintext request, signing key, or
publication authority. Create, Evolve, Install, Fork, publication approval,
profile update, and durable statuses all cross the same signed, encrypted,
idempotent command boundary.

### One product and one normal path

The signed/notarized DMG and one-line installer converge on the same
`Tohseno.app`, canonical Rust service/factory, `~/.tohseno/bin/tohseno` CLI,
Companion payload, and verified release metadata. A new login shell resolves
that CLI through one exact idempotent Tohseno-owned shell-integration block.
The app can repair or remove only that block and preserves every unrelated
byte. No Node, npm, Bun, Homebrew, Rust, sudo, Gatekeeper bypass, or second
binary updater is part of consumer setup.

The normal Mac navigation is **Apps · Registry · Profile · Settings** with one
clear Create action. The normal Companion navigation is **Apps · Registry ·
Profile** with one clear Create action. Internal engine phases, raw harness
output, prompts, identities, and protocol controls remain outside the normal
path. Studio is the native app: `tohseno` and `tohseno studio` make the service
healthy and open `Tohseno.app`.

`tohseno init [path]` adopts an ordinary Xcode project non-destructively,
preserves a stable candidate ShotID on retry, and ends with **Ready. Next:
tohseno deploy**. ADR 0014 recording initialization moves behind the explicit
`tohseno recording init` compatibility namespace and never migrates silently.
`tohseno deploy` is the hero command. It prepares and explains the public
snapshot, waits for Companion approval, resumes its durable publication job,
and prints a URL only after source, Builder authorization, Registry state, and
catalog discovery are all verified.

The website leads with **Ship iPhone apps. Person to person.** It shows the two
commands, exact Mac/Xcode/iPhone requirements, real Registry data, canonical
Shot pages, Builder pages, and deep links into the native apps. It says “Skip
App Store submission. Keep Apple's signing security.” It does not market an
App Store bypass, token economics, fabricated cards, or unimplemented trust.

Fork fixes one immutable parent release, verifies and materializes visible
owner source, assigns a new local project identity and random ShotID, and
records a Companion-signed off-chain parent relation binding child ShotID,
parent ShotID, and parent release digest. It never reuses parent authority or
claims an on-chain ShotRelations contract. Generated creation remains one
source of software and never publishes automatically. Evolution remains a
private living-project action until **Ship Update** or `tohseno deploy`.

## Consequences

Tohseno's product promise becomes a small causal chain a stranger can inspect:
install, pair, initialize, approve, ship, open a link, verify, locally sign,
install, fork, change, and ship again. Public activity begins only with a
Builder's explicit Companion approval. End-user installs, devices, Apple teams,
private forks, prompts, and local evolution remain private.

The release is a minor product release. Production services deploy dark and
backwards-compatible before client activation. A Mac release is advertised
only after a clean source commit, Developer ID signing, notarization, stapling,
Gatekeeper and mounted-DMG verification, exact immutable URL and digest,
Companion payload verification, and clean-Mac acceptance. A real smoke Shot
must use the ordinary Companion-approved flow; no admin row or fabricated
receipt is evidence.

No token purchase, staking, Appcoin, or `$TOHSENO` action is required to
install, browse, publish, install a Shot, or fork. The active contracts provide
identity, authorization, integrity, and public provenance. They do not become
a marketplace, Apple-signing service, source transformer, or custodial wallet.
