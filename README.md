# TOHSENO

TOHSENO is a protocol for giving coherent human intentions persistent
computational identity and allowing them to become, remain, and evolve as
verifiable expressions.

The first factory is concrete: declare one coherent intention, let the Apple
factory and your coding agent materialize a native application, experience an
immutable version, attach feedback to that exact state, and evolve it without
losing origin or ownership continuity.

A Shot is the durable identity created by committing the intention. The app,
folder, repository, current source, deployment, and optional token are
expressions or relationships of that Shot—not the Shot itself.

The current product release is **TOHSENO 0.7.1**. It ships the local GENESIS
protocol implementation without claiming a canonical 1.0 protocol, deployed
contracts, or public infrastructure.

## The ordinary loop

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
```

On a new Mac, that command verifies the complete immutable 0.7.1 release,
installs it transactionally, starts the loopback-only Studio server, opens
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
TOHSENO 0.7.1 is available. Run `tohseno update`.
```

Update transactionally with `tohseno update` (`tohseno upgrade` is an alias).
Remove only the installed program and its exact shell PATH line with:

```sh
tohseno uninstall
```

Uninstall never deletes visible Shot folders, identities, feedback, v0.6 data,
or other machine state.

Automation or package inspection can install without launching Studio:

```sh
curl -fsSL https://tohseno.com/oneshot.sh |
  TOHSENO_START_STUDIO=0 bash
```

Shots are visible folders under `~/Desktop/Tohseno/`
(override with `TOHSENO_HOME`). Each carries its local protocol body in
`.tohseno/`, exact intention and genome surfaces, an evolutionary-intent
working file, immutable version worlds, and version-bound feedback. The first
expression is one native Apple app. Edit that expression with anything—your
coding agent, Xcode, or an editor—and accept its next state through:

```sh
cd ~/Desktop/Tohseno/my-app
tohseno evolve
```

An engine-written `AGENTS.md` tells any entering agent to obey both the factory
constitution and the accepted Shot genome, maintain the expression's
`MEMORY.md`, and run the recording command only when its work is whole.

An Evolution completes on the Mac: once the world builds, the engine
materializes a Simulator artifact, captures a `preview.png` of the running
first screen, signs the record, and verifies it — no iPhone required.
`tohseno refresh my-app` installs the latest Evolution on a phone
whenever one is cabled. An intent-bearing `create` or `evolve` prepares a
private execution boundary and opens a native terminal with
`tohseno shot run …` visible but unexecuted. The person presses Enter to start
the selected Codex or Claude Code interface. TOHSENO observes durable lifecycle
and repository evidence without proxying the conversation or bypassing the
harness's native permissions (ADR 0005).

Upgrading from 0.6.0 preserves the old hidden ledger at
`~/.tohseno/apps`. Run `tohseno migrate-legacy` after installation to copy
those apps into visible folders and project their frozen signed histories.
The operation never deletes or rewrites the 0.6.0 source.

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

TOHSENO 0.7.1 requires macOS 13 or later and Xcode. A physical install
requires an iPhone connected through Apple’s device tooling and a usable Apple
signing identity. A free Apple ID is the default development path; paid Apple
Developer membership is only needed for longer-lived signing or App Store
distribution.

## Protocol candidate

The canonical v2 model is additive around the frozen v1 Apple records:

- a random permanent `ShotID`, independent of names, folders, repositories,
  bundle identifiers, tokens, and controllers;
- a signed Commitment and exact original Intention;
- an explicitly proposed and accepted Shot Genome;
- stable Expression IDs and content-bound immutable Version IDs;
- declared capability organs, with the Apple Fascia as the first concrete
  substrate;
- Feedback bound to an exact expression Version;
- authorized Evolutionary Intents and verified Evolution transitions;
- signed append-only lineage actions with honest artifact availability;
- ownership under the existing BuilderID and P-256 DeviceKey system;
- optional Token Associations that never replace identity;
- verifiable public replication through independent partial nodes.

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

The candidate node stores signed public lineage actions, derives indexes, and
reports unresolved parents and missing artifacts. It does not store referenced
artifact bytes or manufacture one global network head. A public segment can be
signature-valid while authority remains unresolved until its predecessor is
available.

Portable export/import is a verified record projection, not a source clone,
ownership transfer, or trusted materialization. Public export does not relabel
private intention or feedback as public. The candidate bundle inventory commits
every included payload file, while source and retained build artifacts remain
explicit omissions.

Local creation and verification do not require a TOHSENO server. Publishing is
a separate signed action. The candidate Robinhood Chain contracts are
non-upgradeable, unaudited, and currently only a deterministic deployment plan;
the three values in
[`contracts/deployments/robinhood-mainnet-genesis.json`](contracts/deployments/robinhood-mainnet-genesis.json)
are planned CREATE2 addresses, not deployed contracts.

The normative entry points are:

- [`WHITEPAPER.md`](WHITEPAPER.md) for the short model;
- [`docs/IMPLEMENTATION_MAP.md`](docs/IMPLEMENTATION_MAP.md) for the audited
  pre-change system and compatibility decisions;
- [`docs/adr/0004-coherent-intention-lineage.md`](docs/adr/0004-coherent-intention-lineage.md)
  for the ontology decision;
- [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md) for security and privacy
  boundaries;
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
tohseno verify my-app
tohseno --json verify my-app
```

Strict local verification does not call an LLM.
`tohseno verify --public` adds bounded read-only RPC checks; it
currently fails honestly because the candidate registry is undeployed.

Recovery-root backups are encrypted locally and require explicit confirmation.
Secret words are never emitted as JSON:

```sh
tohseno identity backup --confirm
tohseno identity import-backup --confirm
```

`identity backup` and `identity import-backup` store encrypted local
recovery-authority material bound to the current BuilderID. They do not
activate recovery, recover or rotate an account, authorize a replacement
device, or submit an on-chain action.

The GENESIS CLI intentionally exposes no DeviceKey authorize, revoke, rotate,
or recover command. Its signer and offline verifier accept only the original
DeviceKey that reproduces the CREATE2 BuilderID. A valid signature by another
key fails closed because the candidate has no canonical authorization proof
chain or evidence-backed nonce source yet. See
[ADR 0001](docs/adr/0001-device-key-replacement-deferred.md).

The candidate's complete local Shot lifecycle is explicit and automation-safe:

```sh
tohseno create field-notebook \
  --prompt-file intention.md \
  --accept-genome \
  --no-launch

# After the accepted Apple expression has been materialized:
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
tohseno verify field-notebook
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
public-record nodes.

The neutral v2 Token Association lifecycle is local-first and chain-specific.
This example signs a relationship to a mock Base address and explicitly makes
only that action node-ingestible:

```sh
tohseno --json token associate anky 8453 \
  0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7 \
  --symbol ANKY --public
tohseno-node --root /absolute/path/to/node \
  ingest /path/reported/in/outbox_path
```

`--public` writes exact canonical signed bytes beneath the Shot's ignored local
outbox. It does not contact a node, prove the token contract exists, broadcast
a transaction, or verify a chain anchor. Because the rest of a private Shot is
not leaked, a node receiving only this public child reports the exact private
predecessor as missing and authority as unresolved. The signature and available
segment still verify. Omit `--public` to keep the action intentionally private;
use `token remove` for an explicit historical removal.

Legacy `v0.6` apps keep their existing ledger. Their first protocol record is
an explicit N+1 adoption root; TOHSENO does not invent cryptographic history:

```sh
tohseno adopt my-legacy-app
```

A deterministic private static page can be prepared without publishing it:

```sh
tohseno page build my-app
```

The older `publish`, `handle`, and `appcoin` commands below are the frozen
GENESIS contract-compatibility lifecycle; they are distinct from neutral v2
Token Associations. These public mutation commands require an explicit RPC URL and a future Unix
deadline. They first verify chain 4663, every pinned candidate runtime, the
P-256 precompile, the relations binding, the exact BuilderAccount code and
DeviceKey permission, and all relevant controller/head/sequence/nonce state at
one concrete block. No public action is signed if any read is missing or
mismatched:

```sh
deadline="$(( $(date +%s) + 900 ))"

tohseno publish my-app \
  --rpc-url "$ROBINHOOD_RPC_URL" --deadline "$deadline"
tohseno handle claim field-notebook my-app \
  --rpc-url "$ROBINHOOD_RPC_URL" --deadline "$deadline"
tohseno appcoin associate my-app 4663 0x1111111111111111111111111111111111111111 \
  --rpc-url "$ROBINHOOD_RPC_URL" --deadline "$deadline"
```

Preparation is the default. It writes a closed `SignedPublicAction`, exact
`0x01 || x || y || r || s` compact signature, target, calldata, expected
post-state, and block-pinned read evidence beneath
`TOHSENO/public-actions/` as a private create-new file. If the predicted
BuilderAccount is absent, preparation instead writes the exact factory request
without claiming it was signed or deployed. The current candidate publishes no
caller-selected content commitment (the optional field is deterministically
zero), and `appcoin associate` refuses to overwrite an existing relation.

Relaying is separately opt-in. It accepts only a named Foundry keystore or an
attached Ledger/Trezor—never a raw private key, mnemonic, password, or unlocked
RPC account:

```sh
tohseno publish my-app \
  --rpc-url "$ROBINHOOD_RPC_URL" --deadline "$deadline" \
  --submit \
  --confirm-experimental-mainnet \
  "I UNDERSTAND THIS WILL BROADCAST TO ROBINHOOD CHAIN MAINNET 4663" \
  --confirm-builder-account-deployment \
  "I UNDERSTAND THIS WILL IRREVERSIBLY DEPLOY MY BUILDERACCOUNT TO ROBINHOOD CHAIN MAINNET 4663" \
  --foundry-account genesis-relayer
```

`--hardware-wallet ledger` or `--hardware-wallet trezor` is the alternative.
Immediately before each relay the CLI repeats the exact live reads and refuses
changed state. The second confirmation is checked at both the CLI flow and the
engine submission boundary only when `--submit` finds that the predicted
BuilderAccount is missing; it separately authorizes that irreversible
deployment through the pinned factory. Preparation and relays through an
already-deployed BuilderAccount do not require it.
`scripts/lifecycle-mainnet.sh` forwards the flag only when the operator supplies
the same exact sentence in
`TOHSENO_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION`; leaving it unset still
permits an already-deployed account and fails closed if deployment is needed.
After deployment, the CLI waits for a successful receipt, verifies the pinned
account runtime and permission, then prepares the Shot action. Every action
receipt is duplicate-key-strict and must contain the expected target, sender,
nonzero transaction/block hashes, block number, and success status. The CLI
then retrieves that transaction by hash and requires the same sender, target,
block hash/number, chain 4663, zero value, and byte-exact prepared calldata
before performing a fresh exact public-state verification. Any verification
failure after a structurally identified broadcast retains the known
transaction hash and block in the error. The checked-in deployment plan remains
an honestly undeployed baseline; actual deployment evidence is separate.

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
scripts/build-contract-abi.sh --check
```

The ontology lifecycle smoke uses a unique software-test identity and an
isolated temporary data root. It performs real Apple Simulator builds for
versions `0001` and `0002`, binds selected feedback by signed action
commitment, reconstructs lineage, and verifies a private record-only
export/import. It requires macOS, Xcode, a matching development identity, and
the source-built CLI and Apple identity helper.

To use a source-built Apple identity helper:

```sh
swift build --package-path apple-identity
export TOHSENO_APPLE_IDENTITY_HELPER="$PWD/apple-identity/.build/debug/tohseno-apple-identity"
cargo run --locked -p tohseno -- --json identity show
```

The helper uses Secure Enclave/Keychain by default and does not silently fall
back. Its software backend is explicitly test-only.

## Deterministic Genesis material

From a clean checkout:

```sh
scripts/build-genesis-bundle.sh
scripts/build-genesis-bundle.sh --check
(cd dist/genesis && shasum -a 256 -c FILES.sha256)
```

`dist/genesis` contains the normative documents, all schemas and vectors, the
complete reusable Apple Fascia, contract sources, ABIs, BuilderAccount creation
bytecode, and the explicitly undeployed deployment plan. Its creation time is
the source commit time unless `SOURCE_DATE_EPOCH` is supplied. `--check`
rebuilds independently and compares every output byte.

`TOHSENO_ALLOW_DIRTY_BUNDLE=1` exists for local candidate inspection only. A
release bundle must come from committed inputs.

## Status at this source revision

| Area | Status |
|---|---|
| Protocol schemas, vectors, canonicalization, identities, and lineage law | Implemented; covered by local automated tests |
| Apple identity helper and reusable Fascia | Implemented; software-backed tests completed |
| Solidity factory, account, registry, and relations | Implemented and locally tested; unaudited |
| Public lineage node | Implemented for signed action records and explicit static-peer synchronization; no artifact store or production node claimed |
| Neutral Token Association | Implemented as signed v2 lineage with private-by-default or explicit node outbox handling; no token existence or chain anchor claimed |
| Portable Shot bundle | Implemented as a verified record projection; source materialization intentionally unavailable |
| Robinhood Chain P256VERIFY read-only probe | Observed successfully; exact request/result is in the lifecycle report |
| Candidate contracts | Planned deterministic addresses; not deployed |
| Guarded prepare/sign/relay/receipt verification for publish, handle, and appcoin | Implemented; no mainnet lifecycle transaction performed in this source task |
| BuilderAccount, Shot #1, Evolutions 1–2, handle, and appcoin relation | Not completed on mainnet |
| Physical iPhone build, install, and launch | Not completed |
| Stable v0.7.1 patch and installer-from-release test | Recorded in `release/V0_7_1_READINESS.json`; v0.7.0 remains immutable |
| Canonical release or Arweave publication | Deliberately not completed |

TOHSENO is Apache-2.0 software. It uses established cryptography to create a
portable ownership and continuity layer for personal software.
