# ADR 0006: the public witness is narrow, explicit, and generation-scoped

- Status: accepted for contract generation `0.8.0`
- Date: 2026-07-30
- Supersedes: ADR 0004's compatibility promise for the undeployed v0.7 public
  contract ABIs
- Closes: ADR 0001 through delayed, vetoable device replacement and recovery

## Context

The v0.7 release archive included sources, ABIs, predicted addresses, and
BuilderAccount creation bytecode, but no TOHSENO contract was deployed and no
hardware-backed public BuilderID or Shot registration escaped into evidence.
Security review therefore found one remaining opportunity to correct the
surface without migrating chain state.

The old surface permanently linked a controller to handles and App Store
self-claims, offered a generic content commitment, and let a creator declare
arbitrary registry sequence history. Nothing in the repository consumed the
handle text or App Store claim. Current import rejected nonzero content
commitments. Keeping those fields would expand public disclosure, ownership
semantics, transaction cost, and external authority without a demonstrated
product need.

An immutable, administrator-free witness has no pause or in-place repair.
Every field and contract must therefore earn permanent existence.

## Decision

### Identity and publication boundary

Builder identity is deliberately public and linkable after an authorized
publication. End-user installation and continuity identity remains local and
unlinkable by default; it never becomes a registry controller and the two
identity graphs never touch.

A private/local Shot does not enter this graph merely because it exists.

The registry accepts any deployed contract that returns the exact ERC-1271
magic value for an action. It does not pin one BuilderAccount runtime code
hash: doing so would force migration of the stable registry every time the
more complex account implementation changes. Engine, node, and verifier
policy recognize a public controller as a TOHSENO BuilderID only when its
runtime identity matches an approved contract generation.

EIP-7702 delegation designators are excluded. A delegated EOA can silently
change or remove its signing implementation, which would make controller
semantics mutable beneath third-party registry readers. The contract detects
the exact 23-byte `0xef0100 || address` designation and reports a specific
error.

### No end-user data on-chain

The successor registry stores only:

- independent random Shot ID;
- controller;
- intentionally public checkpoint head;
- public checkpoint count;
- action nonce.

The public head may identify only the canonical ancestry-free public
checkpoint defined below. No path may construct it from app-runtime continuity
records, installation identity, end-user identity or behavior, private
feedback, private references, raw private intentions, ordinary lineage action
digests, or hashes of those values. A hash over a small or guessable private
domain is disclosure, not privacy.

The generic `contentCommitment` and `publicState` fields are removed. Removing
`publicState` dissolves the sealed-commitment mutation defect rather than
preserving and patching a field with no successor responsibility.

### Contracts that remain

- `BuilderAccount` supplies replaceable P-256 device authority and ERC-1271.
- `BuilderAccountFactory` supplies permissionless deterministic deployment.
- `ShotRegistry` supplies registration, controller transfer, and append-only
  public lineage checkpoints.

`ShotRelations` is removed. On-chain handles introduce permanent plaintext and
squatting without a current product need. Unverified App Store attestations
look more authoritative than they are. Token Association remains an optional,
signed, chain-specific protocol lineage relationship and never becomes Shot
identity or ownership merely because no relation contract exists.

### Permissionless commit and signed reveal

Registration has two steps.

1. Anyone may submit an opaque commitment. This path has no controller check,
   signature, nonce, deadline evaluation, or external call, so a
   counterfactual BuilderAccount may reserve before deployment.
2. Reveal recomputes the commitment, requires the controller to be deployed
   and eligible, and validates the controller's EIP-712 signature.

The commitment binds controller, Shot ID, salt, registry, chain ID, and reveal
deadline. The signed reveal additionally binds the initial public head and
registration nonce. An observer can relay a victim's exact reveal only to the
victim's controller. A different controller needs its own commitment to mature
and its own signature.

The effective reveal window is inclusive:

```text
[committedAt + 60 seconds, min(committedAt + 24 hours, deadline)]
```

A duplicate live commitment succeeds idempotently without writing, emitting,
or resetting its original timestamp. This is a deliberate anti-griefing rule:
permissionless submission must not let an observer postpone another person's
reveal forever. At exactly 24 hours the commitment remains live. Successful
reveal deletes it. It may be submitted with a new timestamp only after that
boundary.

### Public checkpoints are not application versions

Every registration starts at `checkpointSequence = 1`; every append increments
exactly once. The counter records observations accepted by this particular
public witness. It is not local lineage length, ontology `Version.ordinal`,
Apple `CFBundleVersion`, or App Store build history. An adopted expression at
local version N still begins at registry checkpoint 1.

The initial head may represent the current accepted public lineage when a Shot
first opts into publication. The registry does not manufacture claims about
unwitnessed earlier checkpoints.

### Registry heads use a public-only projection

The canonical coherent-intention lineage may cross intentionally private
actions. Even a later public action commits to its private predecessor through
`previous`, so its action digest is not a safe registry head.

Contract generation 0.8 clients use the separate closed
`tohseno.public-checkpoint/1` projection. It commits only witness coordinates,
random ShotID, witness-local sequence, the prior public checkpoint, fixed
identity-continuity scope, and a newly declared public timestamp. It contains
no local lineage or expression-state digest. The RFC 8785/SHA-256 commitment is
the registry head and its witness coordinates must equal the EIP-712 action
domain.

The checkpoint has no second signature or controller field. Authority remains
the paired registry action, live ERC-1271 decision, and receipt. A private
checkpoint-to-local-state motivation map may exist locally but is never a
public protocol record.

The earlier mixed-ancestry public-action outbox is write-disabled. Its own
availability flag could not remove the private commitment carried in
`previous`. Existing files remain legacy evidence that a node may preserve as
an unresolved partial segment, but clients must not create new files or use
them as registry heads. Token Associations remain private until a separate
closed, ancestry-free public relation record is defined.

### Recovery closes ADR 0001

BuilderAccount now:

- accepts a strict low-s EOA recovery signature or exact ERC-1271 magic;
- initiates recovery under a separate nonce;
- waits three days;
- allows any active `DEVICE_ADMIN` key to cancel until finalization lands;
- permits permissionless finalization only after maturity;
- permits a device admin to correct or rotate recovery authority;
- advances the device epoch to invalidate prior keys in O(1);
- maintains exact active-device and active-admin counts;
- never allows the active device count to reach zero.

These rules provide the bounded authorization, public recovery receipt,
replay protection, veto, and deterministic key-state transition ADR 0001
required. Offline verification of a frozen v0.7 private artifact remains
separate from successor public activation.

### EIP-7951 is a deployment prerequisite

P-256 verification is precompile-only and fails closed. No 232k-gas
pure-Solidity fallback is retained for a target where the measured native
operation costs 6,900 gas.

An absent precompile and a legitimate invalid EIP-7951 signature can both
return empty bytes. The contract cannot distinguish them at runtime. The
deployment gate is therefore load-bearing and MUST run against the actual
target RPC immediately before any broadcast. It MUST require:

- the positive vector returns exactly integer `1`;
- the negative vector returns empty;
- the point-at-infinity edge vector returns empty;
- measured verifier gas is exactly 6,900, not legacy RIP-7212's 3,450.

A mock, cached observation, warning, or positive-only probe is insufficient.
The committed fixture pins official vectors 1, 3, and 136. The upstream vector
asset still labels every case as 3,450 gas, but final
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) is normative: it specifies
6,900 gas and equal gas consumption for valid and invalid inputs.

The probe obtains a fresh `latest` block, binds every code, semantic, and gas
call to that exact EIP-1898 `{blockHash, requireCanonical: true}` reference,
then resolves the original block number again and requires the same hash before
emitting evidence. It does not silently try another tag. This is deliberate:
the verifier implementation and gas schedule are fork-wide properties, while
the official target RPC could not serve EIP-1898 state metadata for its
`safe`-tag block at decision time.

Gas is measured without deployment or a transaction. The probe first proves a
dedicated meter address is empty, then uses `eth_call` state override to install
a 26-byte helper only at that address. The helper calls the unmodified `0x100`
precompile and reports 7,057 gas: 6,900 for EIP-7951 plus an independently
fixed 157-gas meter overhead. All three vectors must report that same total.
Overriding `0x100`, relying on `eth_estimateGas`, or accepting a provider that
does not support the exact state-override call is forbidden.

Every RPC response is parsed with duplicate-member rejection before semantic
validation. Generated probe evidence records what was observed but is never
reusable deployment authorization. A future deployment command MUST invoke the
probe synchronously against its exact explicit RPC immediately before
broadcast. Until such a command exists, the retired deployment tombstones are
the hard stop.

### Generation and successor resolution

Contract generation `0.8.0` is a clean successor, not an in-place ABI
migration. The v0.7 generation will never be deployed. Its exact private
verification inputs remain frozen, while current operational clients must not
publish through them.

The closed `tohseno.contract-generation/1` record commits immutable build facts:
the exact source inventory and source-state commit, compiler profile, ABIs,
portable BuilderAccount creation bytecode, creation/runtime code hashes, and
conditional CREATE2 arithmetic. Its digest is SHA-256 over RFC 8785 bytes. It
has no deployment state, transaction, block, authority,
signature, or trust root. Predicted addresses are not activation evidence.

A future finalized release activation identifies an active generation by:

- protocol major;
- chain ID;
- contract address;
- runtime code hash;
- activation block.

It also binds the immutable generation-definition digest and actual target-RPC
deploy-gate evidence under an explicit release-authority policy. No production
activation instance or authority trust root is committed now, so generation
0.8.0 remains inactive.

The protocol defines closed activation, release-authority policy, and signed
threshold envelope types now so successor resolution is executable rather than
prose. Activation signatures are domain-separated P-256 signatures from
dedicated offline release keys. Valid signatures prove a threshold only under
the supplied policy; clients still need an independently pinned policy digest.
Builder DeviceKeys, Shot owners, installation identities, relayers, and
deployment senders never become release authorities implicitly.

No unversioned `next` plan is authority. If the immutable registry is later
wrong, TOHSENO deploys a new generation and resolves the signed successor by
release-manifest rules. The old registry and its events remain historical
evidence; no administrator rewrites them and no off-chain resolver pretends
they disappeared.

## Consequences

The successor chain surface witnesses continuity without pretending to contain
the creative object. It exposes fewer privacy, naming, spoofing, and semantic
failure modes. Relayers remain permissionless and `msg.sender` remains
irrelevant to authority.

Publication becomes an explicit, delayed two-transaction operation. Clients
must persist the salt and commitment time safely, distinguish registry
checkpoint count from application version, validate recognized BuilderAccount
generations, and refuse publication when the actual target has not passed the
EIP-7951 hard gate.

The removal is ABI-breaking by design and belongs to contract generation
`0.8.0`. Frozen v0.7 decoding remains only for offline legacy verification.
