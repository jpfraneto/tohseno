# Contract generation 0.8.0 migration

Status: historical migration design; generation 0.8.0 was later deployed and
activated, while the public product workflows remain incomplete

Current-state note (2026-08-26): inactive/undeployed statements below describe
the migration boundary when this document was written. Current activation
authority is in `protocol/SPECIFICATION.md` and
`release/contract-activations/`. Frozen v0.7 retirement and the rule that
app-metadata `/2` is not a publication receipt remain unchanged.

The successor contracts are a clean generation, not an in-place upgrade. No
TOHSENO contract was deployed from v0.7, and no hardware-backed v0.7 BuilderID
or registered Shot was found in repository evidence. There is therefore no
chain state to migrate and no proxy, administrator, or storage-layout
compatibility requirement.

The released v0.7.1 sources, ABIs, deployment-plan bytes, and BuilderAccount
creation bytecode remain frozen for offline verification. Their predicted
addresses are not durable public identities and will never be deployed by the
TOHSENO project.

## ABI changes

`ShotRelations` is removed in full. On-chain handles, Appcoin mutations, and
App Store self-attestations have no successor ABI.

`ShotRegistry` changes generation and EIP-712 domain version:

- `createShot` becomes permissionless `commitShot` followed by the
  controller-signed `registerShot` reveal;
- creation nonces become `registrationNonces`;
- `appendEvolution` becomes `appendCheckpoint`;
- witness `sequence` becomes `checkpointSequence`, begins at one, and
  increments exactly once per accepted append;
- `publicState`, generic `contentCommitment`, `setPublicState`, and their
  constants/events are removed;
- registration commitment timing and inspection APIs are added;
- action type hashes and events change accordingly.

`BuilderAccount` replaces immediate `recover` with:

- `initiateRecovery`;
- `cancelRecovery`;
- permissionless `finalizeRecovery` after three days;
- admin-authorized `changeRecovery`;
- `activeAdminCount` and an unconditional active-device floor.

Recovery accepts either a strict low-s EOA signature or exact ERC-1271 magic.
Every new signed action has its own type hash, nonce, and deadline.

## Consumer changes

- `protocol` keeps frozen v0.7 action decoders and adds distinct registry-v2,
  public-checkpoint, generation-definition, release-policy, and activation
  types. It never changes meaning based only on an ABI filename.
- `engine` dispatches legacy BuilderID verification only for exact
  `jpfraneto/tohseno` / `0.7.0` record provenance. Existing private v0.7
  records remain verifiable. A frozen descriptor claiming `deployed` is
  rejected because the v0.7 generation never deployed. New secure BuilderID
  creation and all public signing fail closed until a trusted signed activation
  exists.
- The shipped `tohseno.app-metadata/2` schema and sealed Apple decoder retain
  their optional legacy registry field byte-for-byte for offline compatibility.
  Current engine policy clears that field on projection and rejects any
  non-null value during generation, acceptance, or verification while no
  generation is active. Activated publication evidence requires a new
  app-metadata schema and versioned successor Fascia; bare coordinates are not
  upgraded into receipts.
- `node` may preserve legacy public lineage neutrally, but reports
  `active_generation: null` and never promotes a v0.7 CREATE2 prediction into
  current public authority. Peer-derived classifications are ignored and
  recomputed locally.
- `cli` and Studio must not query retired addresses or expose old mutation
  flows. They report the versioned build definition separately from activation.
- `fascia/apple` continues to decode its sealed v0.7 provenance without
  rewriting it. Its next accepted revision must remove historical
  `ShotRelations` language and add generation-scoped publication-receipt
  verification explicitly; current sealed artifacts are not mutated in place.
- Apple `CFBundleVersion` remains the local expression `versionOrdinal`.
  Registry checkpoint sequence never feeds Xcode or App Store build numbering.

No generated Shot folder, random ShotID, v1 record, expression Version, token
relationship, or private lineage is rewritten by this contract change.

## Activation and future successor rule

`contracts/generations/0.8.0/generation.json` identifies reproducible build
facts and conditional CREATE2 coordinates. Its digest is
`SHA-256(RFC8785(definition))`. It does not identify deployed contracts.

Activation requires a separate threshold-signed record under an independently
trusted release-authority policy. That record must bind the generation digest,
chain, observed addresses and runtime hashes, deployment transactions and
blocks, canonical activation block, and a fresh actual-target EIP-7951 probe.
No policy trust root or activation instance is committed.

A later repair uses a new immutable generation and a signed successor
activation. Clients follow the trusted activation chain; no registry
administrator rewrites old state.

## Release placement

These ABI-breaking changes do not alter or republish stable v0.7.1. They belong
to contract generation `0.8.0` and to a future product release only after
independent audit, trusted activation-policy review, and explicit human
deployment authorization. Nothing in this migration note authorizes a
deployment.
