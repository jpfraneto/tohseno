# TOHSENO contracts

Status: security-remediated successor draft, undeployed, unaudited, and not
deployment-authorized.

> [!WARNING]
> The frozen v0.7 contract generation will never be deployed by the TOHSENO
> project. Its predicted addresses are historical verification inputs, not
> durable BuilderIDs or future coordinates. The unversioned `next`
> deployment-plan compatibility artifact is also non-authoritative; only the
> versioned 0.8.0 build definition is frozen, and it remains inactive. The
> user-facing notice text lives at
> [`release/V0_7_CONTRACT_GENERATION_NOTICE.md`](../release/V0_7_CONTRACT_GENERATION_NOTICE.md).

These contracts are neutral public witnesses. They are non-upgradeable, have
no administrator, hold no tokens, and grant no privilege to a TOHSENO client,
node, relayer, company, website, or deployer.

## Current surface

- `P256Verifier` calls EIP-7951 `P256VERIFY` at `0x100`. It accepts only the
  exact 129-byte application signature `0x01 || x || y || r || s`, validates
  the public key on P-256, enforces `0 < r < n` and low
  `0 < s <= floor(n/2)`, and accepts only an exact 32-byte integer `1`.
- `BuilderAccount` is an ERC-1271 Builder identity controlled by replaceable
  P-256 device keys. Device administration and delayed recovery preserve an
  unconditional active-device floor and an exact active-admin count.
- `BuilderAccountFactory` deploys those accounts with CREATE2. It never owns or
  controls them, and a front-run deployment returns the same account.
- `ShotRegistry` records only a Shot ID, controller, public-checkpoint head,
  checkpoint count, and action nonce. Registration uses a permissionless
  commit followed by a controller-signed reveal.

`ShotRelations` was cut before deployment. Handles and unverifiable App Store
self-claims do not earn permanent on-chain state. Token Association remains a
signed, chain-specific lineage relationship outside these contracts; a token
is never a Shot, controller, or ownership substitute.

## Public identity and privacy boundary

A Builder identity becomes public and linkable only when an authorized actor
publishes a Shot to a registry. A private or local Shot does not enter this
graph merely because it exists. End-user installation and continuity
identities remain local and unlinkable by default and never serve as registry
controllers.

The registry `head` may identify only the canonical digest of
`tohseno.public-checkpoint/1`, a closed ancestry-free projection. It must never
be an ordinary lineage-action digest or be built from app-runtime continuity
records, end-user identity or behavior, private feedback, private references,
raw private intentions, or hashes of those values. Hashing a small or
guessable private domain is disclosure, not privacy. The removed generic
`contentCommitment` is not part of the successor ABI.

## EIP-712 domains

Every domain uses:

```text
EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)
```

| Contract | Name | Version |
| --- | --- | --- |
| `BuilderAccount` | `TOHSENO BuilderAccount` | `1` |
| `ShotRegistry` | `TOHSENO ShotRegistry` | `2` |

The live `block.chainid` and contract address are included, so signatures do
not cross chains or registry generations.

## Exact successor action types

| Hash | Canonical encoding |
| --- | --- |
| `0x916bdb07dc63f8f944e630d491d633db4e254b88c225dda462fbae8afc34e6e4` | `ShotRegistrationCommitment(address controller,bytes32 shotId,bytes32 salt,address registry,uint256 chainId,uint64 deadline)` |
| `0xc356ba3244a346558a5821261a4eccfb38382e0f90a60dc903003a671d5e828c` | `RegisterShot(bytes32 shotId,address controller,bytes32 head,bytes32 salt,uint64 nonce,uint64 deadline)` |
| `0x4ada9482c2ee717b1b8faa0707d2096906a4cc7d3e9ab28cf94f2b8d220e22f5` | `AppendCheckpoint(bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 checkpointSequence,uint64 nonce,uint64 deadline)` |
| `0x1b48fe9103fb5a4d3c6d61b8a9c98fada30123086d138b1c3c4407fa467c6d22` | `TransferShot(bytes32 shotId,address currentController,address newController,bytes32 currentHead,uint64 checkpointSequence,uint64 nonce,uint64 deadline)` |

BuilderAccount action hashes are asserted in
`test/SpecificationHashes.t.sol`; generated ABI is the review surface for
their exact field order.

## Registration and checkpoint laws

- `commitShot(bytes32)` is permissionless and makes no external call. This
  permits reservation before a counterfactual BuilderAccount is deployed.
- A live duplicate commitment succeeds idempotently without emitting or
  changing its original timestamp. This prevents anyone who observes the
  commitment from indefinitely resetting its maturity.
- Reveal is valid in the inclusive interval
  `[committedAt + 60 seconds, min(committedAt + 24 hours, deadline)]`.
  Successful reveal deletes the commitment; an expired commitment may be
  recorded again only after the original 24-hour boundary.
- The commitment binds controller, independent random Shot ID, salt, registry,
  chain ID, and reveal deadline. The signed reveal additionally binds the
  initial public-checkpoint head and controller registration nonce.
- Reveal requires a deployed ERC-1271 controller. The registry accepts any
  correctly behaving ERC-1271 contract. Successor clients MUST recognize a
  BuilderID only when it matches an approved BuilderAccount generation. This
  keeps the immutable registry neutral and the evolvable semantic rule in
  clients.
- EIP-7702 delegation designators (`0xef0100 || address`, exactly 23 bytes) are
  rejected as controllers because their signing logic can disappear or
  silently change.
- Registration always creates public checkpoint `1`. Each append requires the
  exact current head and increments by exactly one. Transfer preserves head
  and checkpoint count while incrementing the shared Shot nonce.
- `checkpointSequence` counts accepted public witness checkpoints. It is never
  an Apple `CFBundleVersion`, local ontology `Version.ordinal`, or fabricated
  historical version count. An adopted expression at local version N still
  begins at registry checkpoint 1.
- `msg.sender` is never authority. Any address may relay a valid reveal,
  checkpoint, or transfer; authority is resolved live from registry state.

## BuilderAccount safety laws

- There is always at least one active device.
- `activeDeviceCount` and `activeAdminCount` exactly count current-epoch keys;
  the last admin may be revoked only while a recovery authority exists.
- Recovery accepts strict low-s EOA signatures or exact ERC-1271 magic.
- Recovery initiation consumes its own nonce and starts a three-day delay.
  Any active device admin may cancel before finalization; finalization is
  permissionless only after maturity.
- `changeRecovery` lets an active device admin correct or rotate the authority
  and cancels any pending attempt.
- Finalized recovery advances the epoch, installs exactly one all-permissions
  replacement key, and invalidates every prior device in O(1).

## Deterministic artifacts

Run:

```sh
../scripts/build-contract-abi.sh
../scripts/build-contract-abi.sh --check
```

The generator writes current ABI JSON plus the immutable build definition,
versioned ABIs, and portable BuilderAccount creation bytecode beneath
`generations/0.8.0/`. It recomputes the exact source inventory, compiler output,
artifact hashes, raw creation/runtime code hashes, and conditional EIP-1014
coordinates. The source tree is frozen to commit
`862ca6cd3d396271b56b336fee0513ddcf6ecc64`; any source drift requires a new
generation rather than silently rewriting 0.8.0.

The files at `bytecode/BuilderAccount.next.creation.hex` and
`deployments/robinhood-mainnet-next.json` remain explicitly unversioned
development projections. Neither a generation definition nor a predicted
CREATE2 address is deployment or activation evidence.

The frozen v0.7 files `bytecode/BuilderAccount.creation.hex` and
`deployments/robinhood-mainnet-genesis.json` remain byte-for-byte verification
inputs and are never regenerated.

## EIP-7951 deployment gate

`P256Verifier` cannot distinguish a missing `0x100` precompile from a
legitimate invalid signature because both can return empty bytes. The
deployment gate is therefore part of the security boundary.

`../scripts/probe-p256.sh` requires an explicit actual target RPC. It:

- parses every JSON-RPC response with duplicate-member rejection;
- requires chain `4663`;
- obtains one fresh latest block, pins every call to its canonical block hash,
  and verifies the same block is still canonical after the final call;
- requires official positive, negative, and point-at-infinity outputs;
- proves the meter account is empty before injecting only a read-only
  state-override helper there;
- requires exact 7,057-gas meter output for all three vectors, decomposed as
  6,900 EIP-7951 gas plus 157 fixed meter overhead.

The committed vector asset records an upstream inconsistency explicitly: its
JSON still says 3,450 gas, while final EIP-7951 specifies 6,900. The final EIP
is normative. Probe output is evidence, not reusable authorization.

## Deployment status

There is no deployment command on `main`.

The v0.7 deployment, mainnet lifecycle, Genesis archive builder, and stable
release builder fail closed. Their immutable historical implementations remain
auditable at tag `v0.7.1`. A successor deployment path will exist only after
the actual target RPC passes the complete EIP-7951 positive, negative,
infinity-edge, and 6,900-gas hard gate and the final generation coordinates
are committed in a separately authorized release activation. The immutable
0.8.0 build definition is committed, but no activation record or release
authority trust root exists.

No contract in this repository has been deployed by this work.

See
[`docs/MIGRATION_0_8_CONTRACT_GENERATION.md`](../docs/MIGRATION_0_8_CONTRACT_GENERATION.md)
for the exact ABI break, consumer changes, legacy verification rule, and
future activation boundary.

## Verification

```sh
forge fmt --check
forge build --sizes
forge test
forge snapshot --root . --check ../.gas-snapshot \
  --fuzz-seed 0x746f6873656e6f --fuzz-runs 256
../scripts/tests/test-probe-p256.sh
../scripts/build-contract-abi.sh --check
```

The committed snapshot measures every contract unit, fuzz, and invariant test
under a fixed seed. The time- and block-scoped
[`contracts/audits/robinhood-p256-2026-07-30.json`](audits/robinhood-p256-2026-07-30.json)
observation separately measured the actual Robinhood RPC at exactly 6,900 gas.
It is not reusable deployment authorization. The rejected Solidity fallback
was approximately 232,000 gas per verification—about 34 times the native path.

Passing tests are not a smart-contract audit.
