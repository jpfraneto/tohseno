# TOHSENO contracts

Status: `0.7.0`, GENESIS protocol candidate, undeployed and unaudited.

> [!WARNING]
> The v0.7 contract generation is frozen only for verification and will never
> be deployed by the TOHSENO project. Predicted v0.7 BuilderAccount addresses
> are not durable public BuilderIDs or future deployment coordinates. The
> security-reviewed successor is still under development and has no finalized
> address. See `release/V0_7_CONTRACT_GENERATION_NOTICE.md`.

These contracts are a neutral public witness. They are non-upgradeable, have no
administrator, hold no tokens, and grant no privilege to a TOHSENO client,
server, relayer, company, website, or deployer.

## Components

- `P256Verifier` calls EIP-7951 `P256VERIFY` at `0x100`. It accepts only the
  exact 129-byte application signature `0x01 || x || y || r || s`, validates
  that the public key is on P-256, enforces `0 < r < n` and low
  `0 < s <= floor(n/2)`, sends the exact 160-byte precompile input
  `digest || r || s || x || y`, and accepts only an exact 32-byte integer `1`.
- `BuilderAccount` is an ERC-1271 BuilderID controlled by replaceable P-256
  device keys. Device permissions are `PROTOCOL = 1` and
  `DEVICE_ADMIN = 2`. Recovery setup is a one-time device-signed action after
  account prediction; later recovery rotates the recovery authority and
  invalidates every prior device epoch.
- `BuilderAccountFactory` uses CREATE2 and has no ownership relationship with
  the accounts it creates. Prediction depends only on factory, salt, and the
  initial P-256 coordinates. Optional recovery setup never changes BuilderID.
- `ShotRegistry` records only controller, current Evolution head, sequence,
  public state, optional public content commitment, and action nonce. Any
  caller may relay a valid action and the caller never becomes owner.
- `ShotRelations` keeps handles, appcoins, and App Store attestations outside
  the neutral registry.

`appcoin` is the frozen candidate contract/API name for a **Token
Association**. It does not make the token the Shot, transfer Shot ownership,
grant controller authority to a token holder, or require a Shot to have a
token. The relation stores the token's own chain ID, so a Shot witnessed on
Robinhood Chain 4663 may independently associate a token such as `$ANKY` on
Base 8453. Replacing or removing that current relation is a separate
owner-authorized action; historical events remain chain history.

`keyId` is exactly `keccak256(abi.encodePacked(bytes32(x), bytes32(y)))`.

## EIP-712 domains

Every domain uses:

```text
EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)
```

Type hash:
`0x8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f`.

| Contract | Name | Version |
| --- | --- | --- |
| `BuilderAccount` | `TOHSENO BuilderAccount` | `1` |
| `ShotRegistry` | `TOHSENO ShotRegistry` | `1` |
| `ShotRelations` | `TOHSENO ShotRelations` | `1` |

The live `block.chainid` and contract address are included, so signatures do
not cross chains or deployments.

## Exact action types

| Hash | Canonical EIP-712 type string |
| --- | --- |
| `0xcdb0126f85b19b28642fc350e8d771c41afd39854da572f865a3612c3242ee95` | `AuthorizeDevice(address account,bytes32 keyId,uint256 x,uint256 y,uint32 permissions,uint64 nonce,uint64 deadline)` |
| `0xc3323bbac7e11d8f4946087779bc3f90a673ec602cab93c377d33aa1f5648f8e` | `RevokeDevice(address account,bytes32 keyId,uint64 nonce,uint64 deadline)` |
| `0xb6ec3e464c0650bda67f76c0b0d7d72afcd16d403c610d18cd7793118c1a6ed4` | `SetRecovery(address account,address recovery,uint64 nonce,uint64 deadline)` |
| `0x539858b6492297e81f07fffbe9f57c1363d0529cf5783087d05848f72fedb820` | `RecoverAccount(address account,address currentRecovery,address newRecovery,bytes32 newKeyId,uint256 newX,uint256 newY,uint64 nonce,uint64 deadline)` |
| `0xe627bc9302992c61fc4043b351fb7d7551f9ed0e0753a1e76a0e68e7a9a60b99` | `CreateShot(bytes32 shotId,address controller,bytes32 head,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)` |
| `0x3a0d9d9dfaedfea172f8ba24e22ce2e86abf77a208168a5246f7a7be2d72de67` | `AppendEvolution(bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 sequence,bytes32 contentCommitment,uint64 nonce,uint64 deadline)` |
| `0x0de266e673064af8f761f1bf3366a2565f963d431128044ec828ab11fcff4e62` | `TransferShot(bytes32 shotId,address currentController,address newController,bytes32 currentHead,uint64 sequence,uint64 nonce,uint64 deadline)` |
| `0x1ab3043484eb2409f5e939f9f5666eac599cffe910808e5711554613bc513c2c` | `SetPublicState(bytes32 shotId,bytes32 currentHead,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)` |
| `0x01bbf31631b6914cf984c8f743c927970ade109f7195087d5670a72ae645ac0d` | `ClaimHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)` |
| `0x1671629a28ceca947ec5843ce55671eda943cfcc82e96017424e06ce90c17e74` | `ReleaseHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)` |
| `0x7872c921a764a03a974ddf09ac8a0086034006a1a64534f83111087a00ff71d7` | `AssociateAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)` |
| `0xd28f6beda3c41ce6f132f41a0800be466a97d65ab19b16c69f5a9202fa85ef3e` | `RemoveAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)` |
| `0xb6f951e532ceef1e89f76812699e4039dac302f458a23fa83e69dffa475d1e96` | `AttestAppStore(bytes32 shotId,bytes32 bundleIdHash,uint64 storeId,bytes32 evolutionHead,uint64 nonce,uint64 deadline)` |

## State laws

- A BuilderAccount always retains at least one active device. It may retain no
  active `DEVICE_ADMIN` key only while a nonzero recovery authority exists.
  `activeDeviceCount` and `activeAdminCount` are exact current-epoch counts,
  not historical counters.
- `CREATE_SHOT` requires a nonzero starting sequence, a deployed ERC-1271
  controller, a nonzero ShotID and head, controller creation nonce, and state
  `PUBLISHED`. Native roots begin at `1`; adopted legacy roots may begin at
  their exact next sequence `N + 1`. Every later append remains exactly `+1`.
- Public state is exactly `PUBLISHED = 1` or `APP_STORE = 2`. Local/private
  Shots do not enter the registry. State may remain unchanged or graduate from
  Published to App Store; App Store cannot downgrade to Published.
- Every accepted Shot transition consumes the current Shot nonce. Append
  requires the exact previous head and next integer sequence. Transfer requires
  the exact current controller, head, and sequence.
- Handles are lowercase ASCII letters, digits, and interior hyphens, 1–63
  bytes. A Shot has at most one handle and a handle names at most one Shot.
- Appcoin relations require an explicit nonzero chain ID and token address.
  They are never required for Shot creation, ownership, publication, evolution,
  transfer, or use.

## Deterministic artifacts

Run:

```sh
../scripts/build-contract-abi.sh
../scripts/build-contract-abi.sh --check
```

The generator writes canonical ABI JSON under `abi/` plus explicitly
non-versioned development artifacts at
`bytecode/BuilderAccount.next.creation.hex` and
`deployments/robinhood-mainnet-next.json`. The frozen v0.7 files
`bytecode/BuilderAccount.creation.hex` and
`deployments/robinhood-mainnet-genesis.json` are verification inputs and the
generator never rewrites them.

A final contract generation receives a versioned bytecode and deployment-plan
path exactly once, after all bytecode-affecting remediation is complete. Until
then, no `next` artifact is a durable identity input or deployment authority.

## Candidate deployment procedure

`scripts/deploy-candidate.sh` is the only supported deployment path for these
candidate contracts. It sends `salt || initcode` to the pinned Arachnid
deterministic deployment proxy at
`0x4e59b44847b379578588920ca78fbf26c0b4956c`. It refuses any other chain,
proxy code, proxy code hash, salt, initcode hash, predicted address, or runtime
code hash.

Provision a Foundry encrypted-keystore account ahead of time. Never pass a raw
private key, mnemonic, keystore password, or password file through an
environment variable or shell argument. The script unlocks only the explicit
account name at Foundry's password prompt and requires its derived address to
match a separately supplied address:

```sh
export ROBINHOOD_RPC_URL='https://authenticated-robinhood-rpc.example'
export TOHSENO_DEPLOYER_ACCOUNT='genesis-deployer'
export TOHSENO_EXPECTED_DEPLOYER_ADDRESS='<explicit 20-byte address>'
export TOHSENO_ALLOW_EXPERIMENTAL_MAINNET=1

../scripts/deploy-candidate.sh --dry-run
```

Supply the exact expected signer; the script rejects zero. Dry-run mode
performs the P-256 probe, complete Foundry tests, artifact drift checks, live
chain checks, account/address comparison, and a stateful fork simulation. It
prints the signer, nonce, balance, gas estimate, and all planned addresses
without signing or broadcasting anything.

After independent review of that output, a real run additionally requires a
clean committed source tree and this second exact guard:

```sh
export TOHSENO_DEPLOY_CONFIRMATION='DEPLOY_GENESIS_1_0_0_RC1_TO_ROBINHOOD_MAINNET_4663'
../scripts/deploy-candidate.sh
```

Transactions are submitted sequentially with the named encrypted account and
confirmed before continuing. Every receipt, transaction envelope, deployed
code size, and runtime code hash is checked. A failure after any broadcast
leaves the immutable undeployed plan untouched and writes no evidence, so an
operator can inspect chain state and safely rerun. Only after all contracts
verify does the script atomically create the separate
`deployments/robinhood-mainnet-genesis.actual.json`; it never overwrites an
existing evidence file.

## Verification

```sh
forge fmt --check
forge build
forge test
```

The tests use no third-party Solidity dependency. A configurable local mock at
`0x100` exercises strict precompile return handling; `scripts/probe-p256.sh`
checks a real EIP-7951 vector through the Robinhood Chain mainnet precompile.

Passing tests are not a smart-contract audit. Do not treat these candidate
addresses or sources as canonical until independent review and the complete
Genesis lifecycle succeed.
