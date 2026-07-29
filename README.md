# TOHSENO

TOHSENO is a printing press for Apple apps and an open continuity protocol for
the worlds it creates. Describe one app, let a coding agent build it, and put a
complete signed Shot on your iPhone. Evolving the app creates another complete
world with the same permanent Shot identity and an append-only history.

The repository is currently **GENESIS 1.0.0-rc.1**. It is a protocol candidate,
not the canonical shipped protocol. Its contracts are planned but undeployed,
the candidate GitHub release has not been created, and the real-iPhone Genesis
lifecycle has not been completed. The stable `v0.6.0` installer remains
unchanged.

## The ordinary loop

The current stable release remains:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
tohseno create my-app
tohseno evolve my-app
```

The prepared candidate channel will be installable after the
`v1.0.0-rc.1` prerelease exists:

```sh
curl -fsSL https://tohseno.com/oneshot.sh |
  TOHSENO_CHANNEL=genesis bash
```

The candidate uses `~/.tohseno-genesis/` and the command
`tohseno-genesis`; it does not replace the stable `~/.tohseno/` ledger or
`tohseno` binary.

In the candidate, apps are visible folders under `~/Desktop/Tohseno/`
(override with `TOHSENO_HOME`), each carrying its own ledger in `.tohseno/`
(ADR 0003). One Shot per app — the enduring intent — with many recorded
Evolutions. Edit the folder with anything — your coding agent, Xcode, an
editor — and record its state as the next signed Evolution:

```sh
cd ~/Desktop/Tohseno/my-app
tohseno-genesis evolve
```

An engine-written `AGENTS.md` in every folder tells whatever agent enters
to obey the genome, maintain the Shot's `MEMORY.md`, and run that command
itself when its work is whole — the builder never has to remember it.

A Shot completes on the Mac: once the world builds, the engine materializes
a Simulator artifact, signs the record, and verifies it — no iPhone
required. `tohseno-genesis refresh my-app` installs a completed Shot on a
phone whenever one is cabled. Interactive `create` opens your own detected
agent in a new terminal on the folder; `--harness <id|/absolute/path>`
instead drives an agent headlessly inside the isolation boundary, where
stock agents currently cannot authenticate — the planned fix is the
credential broker in
[`docs/adr/0002-harness-credential-broker.md`](docs/adr/0002-harness-credential-broker.md).

Do not use that candidate command as evidence of a release today: no candidate
tag or release artifact has been published yet. The release workflow requires
both machine-readable candidate readiness for a prerelease and explicit
prerelease authorization; both remain false in the current evidence report.
Publication also requires recorded verification that GitHub
[release immutability](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes)
is enabled and that an active tag ruleset prevents updates or deletion of
`v1.0.0-rc.1`; those prerequisites remain false as well.
Cleanup readiness is a separate post-release acceptance signal and remains
false until the published prerelease has been installed and exercised.
Immediately before publication, the workflow also reads the exact live GitHub
tag ref, safely peels a bounded annotated-tag chain, and requires its commit to
match the workflow event commit. It attaches every asset to a draft, repeats
the live tag check, and only then publishes the prerelease. Interrupted runs
may reuse only the same commit-bound unpublished draft; expected assets are
clobbered, downloaded, and compared byte-for-byte before publication. A retry
after a lost publish response accepts only the exact immutable prerelease after
the same asset and tag verification. The checkout remains credential-free, and
write permission remains scoped to the publish job.

The GENESIS candidate requires macOS 13 or later and Xcode. A physical install
requires an iPhone connected through Apple’s device tooling and a usable Apple
signing identity. A free Apple ID is the default development path; paid Apple
Developer membership is only needed for longer-lived signing or App Store
distribution.

## Protocol candidate

A Shot is an Apple app with:

- a random permanent `ShotID`;
- a stable `BuilderID`, predicted before its smart account is deployed;
- a finite Apple Fascia describing identity, storage, continuity, privacy,
  provenance, distribution, and declared capabilities;
- a complete source-tree commitment;
- an ordered Evolution record signed by an authorized P-256 DeviceKey.

Local creation and verification do not require a TOHSENO server. Publishing is
a separate signed action. The candidate Robinhood Chain contracts are
non-upgradeable, unaudited, and currently only a deterministic deployment plan;
the three values in
[`contracts/deployments/robinhood-mainnet-genesis.json`](contracts/deployments/robinhood-mainnet-genesis.json)
are planned CREATE2 addresses, not deployed contracts.

The normative entry points are:

- [`WHITEPAPER.md`](WHITEPAPER.md) for the short model;
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
tohseno-genesis protocol info
tohseno-genesis protocol vectors
tohseno-genesis --json identity show
tohseno-genesis identity devices
tohseno-genesis inspect my-app
tohseno-genesis verify my-app
tohseno-genesis --json verify my-app
```

Strict local verification does not call an LLM.
`tohseno-genesis verify --public` adds bounded read-only RPC checks; it
currently fails honestly because the candidate registry is undeployed.

Recovery-root backups are encrypted locally and require explicit confirmation.
Secret words are never emitted as JSON:

```sh
tohseno-genesis identity backup --confirm
tohseno-genesis identity import-backup --confirm
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

Legacy `v0.6` apps keep their existing ledger. Their first protocol record is
an explicit N+1 adoption root; TOHSENO does not invent cryptographic history:

```sh
tohseno-genesis adopt my-legacy-app
```

A deterministic private static page can be prepared without publishing it:

```sh
tohseno-genesis page build my-app
```

Public mutation commands require an explicit RPC URL and a future Unix
deadline. They first verify chain 4663, every pinned candidate runtime, the
P-256 precompile, the relations binding, the exact BuilderAccount code and
DeviceKey permission, and all relevant controller/head/sequence/nonce state at
one concrete block. No public action is signed if any read is missing or
mismatched:

```sh
deadline="$(( $(date +%s) + 900 ))"

tohseno-genesis publish my-app \
  --rpc-url "$ROBINHOOD_RPC_URL" --deadline "$deadline"
tohseno-genesis handle claim field-notebook my-app \
  --rpc-url "$ROBINHOOD_RPC_URL" --deadline "$deadline"
tohseno-genesis appcoin associate my-app 4663 0x1111111111111111111111111111111111111111 \
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
tohseno-genesis publish my-app \
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

swift build --package-path apple-identity
swift test --package-path apple-identity
sh fascia/apple/tests/validate-fascia.sh
swift test --package-path fascia/apple

forge fmt --root contracts --check
forge build --root contracts
forge test --root contracts -vvv
scripts/build-contract-abi.sh --check
```

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
| Robinhood Chain P256VERIFY read-only probe | Observed successfully; exact request/result is in the lifecycle report |
| Candidate contracts | Planned deterministic addresses; not deployed |
| Guarded prepare/sign/relay/receipt verification for publish, handle, and appcoin | Implemented; no mainnet lifecycle transaction performed in this source task |
| BuilderAccount, Shot #1, Evolutions 1–2, handle, and appcoin relation | Not completed on mainnet |
| Physical iPhone build, install, and launch | Not completed |
| Candidate tag, GitHub prerelease, and installer-from-release test | Not completed |
| Canonical release or Arweave publication | Deliberately not completed |

TOHSENO is Apache-2.0 software. It uses established cryptography to create a
portable ownership and continuity layer for personal software.
