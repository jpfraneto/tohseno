# TOHSENO

TOHSENO is a protocol for giving coherent human intentions persistent
computational identity and allowing them to become, remain, and evolve as
verifiable expressions.

The first factory is concrete: TOHSENO transforms a coherent human intention
into a complete native app by interpreting the intention, resolving the
relevant powers of the iPhone, materializing the product, experiencing it as
its target user, and only then accepting it into an evolving lineage.

A Shot is the durable identity created by committing the intention. The app,
folder, repository, current source, deployment, and optional token are
expressions or relationships of that Shot—not the Shot itself.

**TOHSENO 0.8.4** ships for macOS. The system is in beta and evolving
quickly: stable releases ship continuously. It ships the local GENESIS
protocol implementation without claiming a canonical 1.0 protocol or public
infrastructure. The 0.8.0 contract generation is an inactive, untrusted
candidate; no release reads from or writes to it.

The immutable `v0.8.4` release is published and the public installer is pinned
byte-for-byte to its claim-capable installer. The production ciphertext relay
is active at the canonical HTTPS origin on an owner-controlled durable volume;
its release and capability gates remain fail-closed by default in source.

Contract generation `0.8.0` is deployed on Robinhood Chain mainnet
(`eip155:4663`) as an **inactive, untrusted candidate** — see
[On-chain status](#on-chain-status) below. Deployment is not activation:
no release reads from or writes to those contracts.

> [!IMPORTANT]
> If an older installed CLI shows a predicted BuilderID or BuilderAccount
> address, that prediction belongs to a retired contract generation and will
> never be deployed by the TOHSENO project. It is not a durable public
> identity, ownership evidence, or a future deployment coordinate — and it is
> not an address in the deployed generation. Local Shots, identities, and
> signed history remain valid and verifiable offline.

## Birth model in 0.8.4

The exact Intention remains the human source of truth. An intelligence reads it
with its references and the locally discovered Apple Capability Profile, then
proposes a generative, app-specific Genome: what must remain true for this
particular product to become itself. The static factory Constitution contains
only cross-Shot integrity, privacy, safety, provenance, and honesty rules.

The Apple Fascia is not a product author or capability denylist. It
deterministically reports what the built app uses, requests, stores, transmits,
and embeds. A live `devicectl` tunnel identifies a connected iPhone; a paired
but disconnected device contributes only sanitized last-known context. Neither
profile contains a UDID or private device identifier. The Release app still
feature-detects optional hardware at runtime. The first
accepted Version is complete within its bounded intention. Evolution starts
from that complete living app after contact with life. A conformant build alone
does not prove that the product promise was fulfilled.

These rules govern new births in 0.8.4. They do not retroactively alter an
older installed CLI or any accepted lineage.

## The ordinary loop

The public journey now starts with the intention:

1. visit `tohseno.com`;
2. write or paste the intention and add up to eight reference images;
3. press **TAKE A SHOT**;
4. run the one copied command on the Mac;
5. let the selected intelligence propose the app-specific Genome, capability
   plan, target users, journeys, and completion contract, then review that
   actual proposal;
6. approve the terminal handoff without entering the intention again.

The Browser Draft stays in IndexedDB. When encrypted handoff is available,
the browser uploads only a temporary AES-256-GCM ciphertext package; the Mac
decrypts it into a durable Local Pending Intention. The website and relay do
not create or identify a Shot. A Shot still begins only through the existing
local engine after the person approves the plan and boundaries. The active
production handoff now presents the one claim command after encryption and
finalization. The private `.tohseno-intent` download and generic installer
remain available as local fallbacks.

The CLI-first route remains available as an alternative:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
```

On a new Mac, that command verifies the current complete immutable stable
release, installs it transactionally, starts the loopback-only Studio server, opens
Studio in the default browser, and presents the resumable first-Shot guide.
The guide checks the actual Mac for Xcode, Apple Development signing, and an
authenticated `$0.00`
subscription route through Codex or Claude Code before enabling the first
Shot. The official harness install commands are copied for the person to run
and authenticate themselves; TOHSENO never installs an agent or receives its
credentials silently.

Studio remains attached to the installing terminal so the process has an
obvious lifetime. Press Control-C there to stop it. Later launches use:

```sh
tohseno studio
```

The installed CLI checks the stable release channel at most once per day. When
a newer release exists, interactive commands show one short instruction:

```text
TOHSENO 0.8.4 is available. Run `tohseno update`.
```

Update transactionally with `tohseno update` (`tohseno upgrade` is an alias).
Remove only the installed program and its exact shell PATH line with:

```sh
tohseno uninstall
```

Uninstall never deletes visible Shot folders, identities, feedback, legacy
data, imported pending intentions, or other machine state.

Automation or package inspection can install without launching Studio:

```sh
curl -fsSL https://tohseno.com/oneshot.sh |
  TOHSENO_START_STUDIO=0 bash
```

Shots are visible folders under `~/Desktop/Tohseno/`
(override with `TOHSENO_HOME`). Each carries its local protocol body in
`.tohseno/`, exact intention and genome surfaces, an evolutionary-intent
working file, immutable version worlds, and version-bound feedback. The first
expression is one complete native Apple app within the accepted intention's
scope. A buildable skeleton is not accepted. Once that living app has been
accepted, change it through a new intention with:

```sh
cd ~/Desktop/Tohseno/my-app
tohseno evolve
```

An engine-written `AGENTS.md` tells any entering agent to obey both the static
factory Constitution and the accepted app-specific Genome. Product memory is
optional and high-signal; it does not carry generic factory-debugging prose or
ritual unfinished-work sections. The harness returns a candidate and evidence;
the engine alone decides whether to accept and seal it.

Birth acceptance has three independent dimensions: protocol conformance,
intent fidelity, and experience verification. The harness builds Release,
runs tests, traverses target-user scenarios in Simulator with controlled
fixtures where appropriate, captures multi-state evidence, reviews the result,
and repairs mismatches before the engine seals anything. The engine also boots
its own Simulator and independently reruns the checked-in Release
XCTest/XCUITest action; the resulting log is a separate Birth Receipt
criterion. Simulator is a test environment, not the definition of an iPhone.
If a must-level experience needs physical sensors, the real framework path
remains in Release and a compatible connected iPhone is required for the
physical trial; without it the candidate remains
`implementation_complete; acceptance_pending_physical_experience`.
`tohseno refresh my-app` installs an accepted Version on a paired phone.
An intent-bearing `create` or `evolve` prepares a
private execution boundary and opens a native terminal with
`tohseno shot run …` visible but unexecuted. The person presses Enter to start
the selected Codex or Claude Code interface. TOHSENO observes durable lifecycle
and repository evidence without proxying the conversation or bypassing the
harness's native permissions (ADR 0005).

Upgrading preserves any older hidden ledger at
`~/.tohseno/apps`. Run `tohseno migrate-legacy` after installation to copy
those apps into visible folders and project their frozen signed histories.
The operation never deletes or rewrites the original source.

Publication requires machine-readable stable-release authorization, GitHub
[release immutability](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes)
and an active tag ruleset that prevents updates or deletion of the exact
release tag.
Immediately before publication, the workflow also reads the exact live GitHub
tag ref, safely peels a bounded annotated-tag chain, and requires its commit to
match the workflow event commit. It attaches every asset to a draft, repeats
the live tag check, and only then publishes the stable release. Interrupted runs
may reuse only the same commit-bound unpublished draft; expected assets are
clobbered, downloaded, and compared byte-for-byte before publication. A retry
after a lost publish response accepts only the exact immutable release after
the same asset and tag verification. The checkout remains credential-free, and
write permission remains scoped to the publish job.

TOHSENO requires macOS 13 or later and Xcode. A physical install or trial
requires a paired iPhone reachable through Apple’s device tooling (USB or a
supported local-network connection) and a usable Apple
signing identity. A free Apple ID is the default development path; paid Apple
Developer membership is only needed for longer-lived signing or App Store
distribution.

## Protocol candidate

The canonical v2 model is additive around the frozen v1 Apple records:

- a random permanent `ShotID`, independent of names, folders, repositories,
  bundle identifiers, tokens, and controllers;
- a signed Commitment and exact original Intention;
- an explicitly proposed and accepted Shot Genome;
- a private structured Birth Plan that binds target users, stable requirements,
  Apple materials, forbidden substitutions, and app-specific organs to the
  exact intention;
- stable Expression IDs and content-bound immutable Version IDs;
- universal protocol-substrate organs separated from product-specific organs,
  with the Apple Fascia as a deterministic membrane of platform, privacy,
  capability, data-movement, identity, and provenance truth;
- target-user Experience Contracts and local Birth Receipts whose digests make
  protocol conformance, intent fidelity, and experience verification explicit;
- Feedback bound to an exact expression Version;
- authorized Evolutionary Intents and verified Evolution transitions;
- signed append-only lineage actions with honest artifact availability;
- ownership under versioned BuilderID and P-256 DeviceKey policy, with frozen
  legacy identity limited to offline verification;
- optional Token Associations that never replace identity;
- neutral replication of bounded lineage evidence through independent partial
  nodes.

`tohseno.shot/1` remains byte-for-byte valid. It is the compatibility record
for one accepted state of the first Apple expression, not a second competing
architecture.

Token Association is a relationship, never identity. A Shot does not need a
token, and one owner may control any number of Shots and token relationships.
For example, Anky can remain its own independently owned Shot while `$ANKY` is
associated with it on Base (`eip155:8453`); that does not make the token
contract the Shot, transfer Shot ownership, merge Anky with TOHSENO, or
conflate `$ANKY` with any `$TOHSENO` association. This repository asserts no
`$ANKY` token address.

The candidate node preserves legacy signed lineage records, derives indexes,
and reports unresolved parents and missing artifacts. It does not store
referenced artifact bytes or manufacture one global network head. Because no
contract generation is active, even a complete neutrally valid branch remains
explicitly unresolved as public Builder authority. The ancestry-free public
checkpoint format is defined, but this node revision does not yet inventory
checkpoint receipts.

Portable export/import is a verified record projection, not a source clone,
ownership transfer, or trusted materialization. Public export does not relabel
private intention or feedback as public. The candidate bundle inventory commits
every included payload file, while source and retained build artifacts remain
explicit omissions. Source-first community testing is a separate, noncanonical
transport: a `.tohseno-workshop` capsule carries one accepted source snapshot,
its reviewed license, and the public records needed to verify it, but never the
Builder's private `.tohseno/` ledger or retained binary. See
[`docs/WORKSHOP.md`](docs/WORKSHOP.md).

Local creation and verification do not require a TOHSENO server. Publication
is a separate, currently inactive boundary. The versioned
[`0.8.0` contract generation](contracts/generations/0.8.0/generation.json)
commits exact source, compiler, ABI, bytecode, runtime hashes, and conditional
CREATE2 arithmetic for audit.
No signed activation or release-authority trust root is committed.

## On-chain status

On 2026-08-01 an authorized one-time ceremony
([ADR 0009](docs/adr/0009-one-time-inactive-0-8-0-deployment.md)) deployed the
non-upgradeable generation `0.8.0` contracts to Robinhood Chain mainnet
(`eip155:4663`) as an **inactive, untrusted candidate**:

| Contract | Address |
|---|---|
| `BuilderAccountFactory` | `0xb1bd208cd2af98e701f43d06aaa889d3a594df65` |
| `ShotRegistry` | `0x3fe6508ba2660bc575080024f402c192a2e035a0` |

Deployment is not activation. No release reads from or writes to these
contracts, no client trust root references them, and activation stays gated
behind independent review, the production canary, and a threshold activation
ceremony; a defective candidate is abandoned, never repaired in place. Do not
send anything to these addresses. The complete signed deployment evidence is
preserved in
[`contracts/audits/`](contracts/audits/) and
[`contracts/deployments/`](contracts/deployments/).

The normative entry points are:

- [`WHITEPAPER.md`](WHITEPAPER.md) for the short model;
- [`docs/IMPLEMENTATION_MAP.md`](docs/IMPLEMENTATION_MAP.md) for the audited
  pre-change system and compatibility decisions;
- [`docs/adr/0004-coherent-intention-lineage.md`](docs/adr/0004-coherent-intention-lineage.md)
  for the ontology decision;
- [`docs/adr/0006-public-witness-and-contract-generation.md`](docs/adr/0006-public-witness-and-contract-generation.md)
  for the narrowed witness and generation/activation boundary;
- [`docs/adr/0007-app-metadata-publication-policy.md`](docs/adr/0007-app-metadata-publication-policy.md)
  for the frozen metadata compatibility and current fail-closed policy;
- [`docs/adr/0011-encrypted-web-to-local-intention-handoff.md`](docs/adr/0011-encrypted-web-to-local-intention-handoff.md)
  for the additive browser-to-local private transport and release gate;
- [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md) for security and privacy
  boundaries;
- [`docs/MIGRATION_0_8_CONTRACT_GENERATION.md`](docs/MIGRATION_0_8_CONTRACT_GENERATION.md)
  for the ABI break and deterministic legacy boundary;
- [`protocol/SPECIFICATION.md`](protocol/SPECIFICATION.md) for protocol law;
- [`protocol/IMPLEMENTERS.md`](protocol/IMPLEMENTERS.md) for independent
  implementations;
- [`protocol/CONFORMANCE.md`](protocol/CONFORMANCE.md) for verification;
- [`fascia/apple/FASCIA.json`](fascia/apple/FASCIA.json) for the Apple Fascia;
- [`genesis/lifecycle/GENESIS_CANDIDATE_REPORT.md`](genesis/lifecycle/GENESIS_CANDIDATE_REPORT.md)
  for exact completed and pending evidence.

## Candidate commands

Protocol and identity inspection support structured output:

```sh
tohseno protocol info
tohseno protocol vectors
tohseno --json identity show
tohseno identity devices
tohseno inspect my-app
cd ~/Desktop/Tohseno/my-app
tohseno verify
tohseno --json verify
```

Strict local verification does not call an LLM.
`tohseno verify --public` is unavailable while no contract generation has a
signed activation. Predicted or retired addresses are never queried as though
they were authority.

Recovery-root backups are encrypted locally and require explicit confirmation.
Secret words are never emitted as JSON:

```sh
tohseno identity backup --confirm
tohseno identity import-backup --confirm
```

`identity backup` and `identity import-backup` store encrypted local
recovery-authority material bound to the stored legacy BuilderID. They do
not activate recovery, recover or rotate an account, authorize a replacement
device, or submit an on-chain action.

The legacy CLI exposes no DeviceKey authorize, revoke, rotate, or recover
command. Its offline verifier accepts only the original DeviceKey that
reproduces the frozen legacy BuilderID. The successor `BuilderAccount` now has
permissioned device administration plus delayed, vetoable recovery, closing
the contract-design question in
[ADR 0001](docs/adr/0001-device-key-replacement-deferred.md); an off-chain
proof format and owner UX remain future work and are not fabricated here.

The candidate's complete local Shot lifecycle is explicit and automation-safe:

```sh
tohseno create field-notebook \
  --prompt-file intention.md \
  --no-launch

# An external/private conception runner reads .tohseno/CONCEPTION.md and emits
# strict conception-output.json. Validate and accept that actual proposal:
tohseno create field-notebook \
  --prompt-file intention.md \
  --accept-genome \
  --conception-file conception-output.json \
  --no-launch

# After the complete candidate and Experience Trial have been materialized:
tohseno evolve field-notebook

feedback_action="$(
  tohseno --json feedback field-notebook \
    --version 1 \
    --file feedback.md |
  jq -er .action_commitment
)"

tohseno evolve field-notebook \
  --prompt-file EVOLUTIONARY_INTENT.md \
  --feedback-action "$feedback_action" \
  --no-launch

# After applying the proposed source change:
tohseno evolve field-notebook
cd ~/Desktop/Tohseno/field-notebook
tohseno verify
```

The local authentic-harness flow is:

```sh
tohseno shot harnesses

tohseno create field-notebook \
  --prompt-file intention.md \
  --image reference-one.png \
  --image reference-two.jpg \
  --accept-genome \
  --harness claude-code \
  --model opus \
  --route claude-subscription

# Terminal opens in the Shot repository with this line visible but unexecuted:
# tohseno shot run --app field-notebook --execution <execution-id>
# Press Enter there to start Claude Code's native interface.

tohseno shot follow --app field-notebook --execution <execution-id>
tohseno shot result --app field-notebook --execution <execution-id>
```

Use `--harness codex --model default --route chatgpt-subscription` for Codex.
`create` and an intent-bearing `evolve` both prepare the same durable local
execution state. `--no-launch` preserves the existing automation-safe staging
behavior. Preparation never starts inference, commits a result, publishes,
deploys, tags, or releases anything.

`feedback_id` identifies the canonical Feedback payload.
`action_commitment` identifies its signed lineage action; only the latter is a
valid `--feedback-action` reference. The engine verifies every selected action
belongs to the current exact Expression and Version. Staging an intent does
not accept a Version, and a failed materialization leaves the last accepted
Version unchanged.

Receiving records is separate from cloning source, adopting ownership, or
materializing code:

```sh
tohseno export field-notebook \
  --output /absolute/path/field-notebook.shot \
  --include-private
tohseno import /absolute/path/field-notebook.shot \
  --output /absolute/path/received-field-notebook
tohseno verify /absolute/path/received-field-notebook
```

The current portable bundle carries verified lineage and explicit omissions;
it deliberately does not carry expression source or owner keys. See
[`node/README.md`](node/README.md) for running and synchronizing independent
neutral/legacy-evidence nodes.

The neutral v2 Token Association lifecycle is local-first, private by default,
and chain-specific. This example signs a relationship to a mock Base address:

```sh
tohseno --json token associate anky 8453 \
  0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7 \
  --symbol ANKY
```

The relationship does not prove the token contract exists, broadcast a
transaction, verify a chain anchor, replace Shot identity, or transfer
ownership. The legacy `--public` flag now fails closed: an ordinary lineage
action commits its predecessor and can therefore disclose a link to private
ancestry. Public Token Associations require a future ancestry-free relation
record. Use `token remove` for an explicit private historical removal.

Legacy apps keep their existing ledger. Their first protocol record is
an explicit N+1 adoption root; TOHSENO does not invent cryptographic history:

```sh
cd /path/to/my-legacy-app
tohseno adopt
```

For an ordinary existing iOS repository, normalize its folder, project,
scheme, target, and product to one TOHSENO-safe slug; add the exact pinned
Apple Fascia and capability declarations; then run `tohseno adopt` from the
repository root. Adoption creates `.tohseno/` and the first verified Evolution.
It does not make incompatible dependencies or capabilities conformant. The
complete adoption and workshop flow is in [`docs/WORKSHOP.md`](docs/WORKSHOP.md).

A deterministic private static page can be prepared without publishing it:

```sh
tohseno page build my-app
```

The frozen legacy `publish`, `handle`, and `appcoin` mutations are no longer
exposed by the CLI, and their deployment and lifecycle scripts fail closed.
Their source and decoding law remain available at the immutable legacy release
tags for offline verification only. No successor public-witness command will be
enabled until an independently trusted release policy authorizes a signed
activation and the actual target RPC passes the complete EIP-7951 deployment
gate.

## Build and verify from source

The candidate toolchain is Rust 1.88, Swift 6, Xcode, and Foundry 1.3.5.

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
scripts/test-ontology-lifecycle.sh

swift build --package-path apple-identity
swift test --package-path apple-identity
sh fascia/apple/tests/validate-fascia.sh
swift test --package-path fascia/apple

forge fmt --root contracts --check
forge build --root contracts
forge test --root contracts -vvv
forge snapshot --root contracts --check .gas-snapshot \
  --fuzz-seed 0x746f6873656e6f --fuzz-runs 256
scripts/tests/test-probe-p256.sh
scripts/build-contract-abi.sh --check
```

The ontology lifecycle smoke uses a unique software-test identity, an isolated
temporary data root, and a deterministic conception harness fixture. It
validates an app-specific Birth Plan, runs a real XCUITest target-user journey,
accepts version `0001` through all three birth dimensions, performs real Apple
Simulator builds for versions `0001` and `0002`, binds selected feedback by
signed action commitment, reconstructs lineage, and verifies a private
record-only export/import. It requires macOS, Xcode, a matching development
identity, and the source-built CLI, node, and Apple identity helper.

To use a source-built Apple identity helper:

```sh
swift build --package-path apple-identity
export TOHSENO_APPLE_IDENTITY_HELPER="$PWD/apple-identity/.build/debug/tohseno-apple-identity"
cargo run --locked -p tohseno -- --json identity show
```

The helper uses Secure Enclave/Keychain by default and does not silently fall
back. Its software backend is explicitly test-only.

## Immutable legacy Genesis material

The released legacy Genesis archive is reproduced only from its immutable
release tag. Main deliberately refuses to rebuild or archive it: doing so
would mix changing successor sources with a frozen legacy deployment plan.

Use the signed legacy release artifact for offline inspection and private
verification. Its predicted contract and BuilderAccount addresses are
undeployed historical inputs, not durable public identities or future
deployment coordinates.

## Status at this source revision

| Area | Status |
|---|---|
| Protocol schemas, vectors, canonicalization, identities, and lineage law | Implemented; covered by local automated tests |
| Apple identity helper and reusable Fascia | Implemented; software-backed tests completed |
| Solidity factory, delayed-recovery account, and narrowed checkpoint registry | Implemented and locally tested; two independent AI reviews complete, independent human audit still pending |
| Bounded lineage-evidence node | Implements legacy-action preservation and explicit static-peer synchronization; active-generation authority stays unresolved, and checkpoint receipt inventory is deferred |
| Neutral Token Association | Implemented as private signed v2 lineage; mixed-ancestry public outbox retired, and no token existence or chain anchor claimed |
| Portable Shot bundle | Implemented as a verified record projection; it still omits source and does not transfer ownership |
| Source-first workshop capsule | Implemented as a separate noncanonical, licensed source transport with local rebuild and exact-version feedback |
| Robinhood Chain P256VERIFY read-only probe | Complete positive/negative/infinity and 6,900-gas observation preserved in [`contracts/audits/`](contracts/audits/); re-verified as a fresh gate in the 2026-08-01 deployment ceremony |
| Successor contracts | Immutable `0.8.0` generation deployed on Robinhood Chain mainnet as an inactive, untrusted candidate (see [On-chain status](#on-chain-status)); no signed activation or client trust root, and activation stays gated behind independent review, the canary, and the threshold ceremony |
| Frozen legacy publish, handle, and appcoin contract flow | Retired and fail-closed on main; retained only at its immutable release tag |
| BuilderAccount, Shot #1, and public checkpoints | Not completed on mainnet |
| Physical iPhone build, install, and launch | Not completed |
| Stable 0.8.1 release and installer-from-release test | Recorded in `release/V0_8_1_READINESS.json`; every earlier stable tag remains immutable |
| Canonical release or Arweave publication | Deliberately not completed |

TOHSENO is Apache-2.0 software. It uses established cryptography to create a
portable ownership and continuity layer for personal software.
