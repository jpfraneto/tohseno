# Contract generation 0.8.0 production canary

Status: prepared procedure only. The candidate deployment exists; no canary
keys, canary funding, canary transaction authority, or activation exists.

This runbook applies only after the exact factory and registry are deployed as
an inactive, independently verified generation 0.8.0 candidate. It is
subordinate to `protocol/`, ADR 0006, the accepted deployment evidence, and a
separate owner-approved canary transaction budget. The transaction payer funded
for contract deployment is not implicitly authorized to fund or sign canary
actions.

## Safety boundary

- Use new random canary Shot IDs, salts, P-256 device keys and recovery
  authorities. Never use a real user, installation, Shot, intention, feedback,
  reference, artifact, handle, token, or guessable private material.
- Keep every canary private key outside the repository, logs, prompts,
  screenshots, CI and public evidence. Publish only public keys, addresses,
  random identifiers, action bytes, signatures, transactions and chain state.
- Use the exact deployed generation contracts. Do not deploy altered test
  versions at production coordinates.
- Every transaction requires an explicit chain, sender, nonce, fee, maximum
  cost and purpose display plus the canary authorization established by the
  owner. Use one-at-a-time submission and uncertain-response handling identical
  to the inactive-deployment ceremony.
- Any unexplained discrepancy fails the canary. Do not patch, proxy, pause or
  rationalize immutable production code; abandon 0.8.0 and define a successor.

## Preconditions

1. Canonical deployment evidence proves the exact factory at
   `0xb1bd208cd2af98e701f43d06aaa889d3a594df65` and registry at
   `0x3fe6508ba2660bc575080024f402c192a2e035a0`, with the factory runtime matching
   its frozen compiler template and the registry's constructor-patched runtime
   reproduced from the exact generation-bound creation input under ADR 0010.
2. The deployment is still explicitly inactive and absent from all client
   trust roots.
3. A fresh actual-RPC EIP-7951 probe passes and is preserved.
4. The owner approves exact canary funder/relayer addresses, maximum native-ETH
   spend, duration and cleanup/retention policy.
5. At least two RPC observations agree on the canonical deployment blocks and
   current runtime bytes.

## Canary identities

Generate on separate controlled devices or isolated processes:

- account A initial admin/protocol P-256 key;
- account A protocol-only P-256 key;
- account A second admin P-256 key;
- account A replacement P-256 key;
- account B initial admin/protocol P-256 key;
- recovery EOAs R1, R2 and R3; and
- transaction relayer/funder identities with no Builder authority.

Use independent random factory salts for accounts A and B. Before each account
deployment, call `predictAccount(salt,x,y)` and independently reproduce the
factory CREATE2 result. Confirm the predicted address is empty. Deploy through
`createAccount`, verify the `AccountCreated` and constructor
`DeviceAuthorized` events, exact BuilderAccount runtime hash, epoch 1, nonce 0,
one active device, one active admin and full permissions for only the initial
key.

Under ADR 0010, compare the account's runtime to an independently instantiated
copy from the exact frozen creation bytecode, not to the compiler template's
zero immutable placeholders. A local two-instance reproduction currently
observes
`0xb5ff14ddc150b2f64cb2243e6d8c8a0c441007841548f1b0ee8d6e22ad452fc0`;
the production canary must establish this afresh and fail closed on any
different bytes.

## Account A device and ERC-1271 matrix

At every step, reproduce the contract hash helper result and the EIP-712 digest
off-chain before signing. Use short explicit deadlines and current on-chain
nonces.

1. Verify `isValidSignature` returns exact `0x1626ba7e` for a valid low-s
   version-1 signature from the initial key.
2. Verify read-only calls return exact `0xffffffff` for wrong digest, wrong key,
   high-s, malformed length/version, zero/out-of-range coordinate and invalid
   precompile-output cases that can be exercised without state mutation.
3. Authorize the protocol-only key with permission `1`. Verify device count 2,
   admin count 1 and nonce +1.
4. Prove by `eth_call` that permission `0` and values containing any bit outside
   mask `3` (for example `4`, `5` and `7`) are rejected; never broadcast an
   invalid-permission transaction.
5. Authorize the second admin with permission `3`. Verify device count 3,
   admin count 2 and nonce +1.
6. Prove the protocol-only key cannot authorize device management.
7. Revoke the protocol-only key. Verify counts, nonce, event and subsequent
   ERC-1271 refusal.
8. Revoke the second admin only after the recovery preconditions below make the
   expected last-admin rule explicit, and verify exact counters.

Each successful mutation must advance only its documented nonce/counter/state.
Replay every prior signed action through `eth_call` and require refusal.

## Recovery and mandatory real-time delay

1. Set R1 once through the current admin device. Verify
   `RecoveryConfigured`, recovery address and device nonce.
2. Rotate R1 to R2 through `changeRecovery`. Verify `RecoveryChanged`, device
   nonce +1, recovery nonce +1, and no pending recovery.
3. Have R2 authorize a recovery to the replacement P-256 key and R3. Submit
   `initiateRecovery`; verify the recovery ID equals the contract hash, recovery
   nonce +1, exact pending fields and `executeAfter = initiation timestamp +
   259200` seconds.
4. Before maturity, prove `finalizeRecovery` refuses. Have an active admin veto
   the exact ID through `cancelRecovery`; verify the pending record is deleted
   and its device nonce advances.
5. Initiate a second recovery with fresh nonce/deadline. Preserve its canonical
   block timestamp and exact `executeAfter`.
6. Wait the real chain time. Do not use local time travel, storage overrides,
   fork manipulation or a shortened contract. Recheck continuously enough to
   detect reorganization, but do not send a premature transaction.
7. At the exact maturity boundary or later, let an authority-free relayer call
   `finalizeRecovery`. Verify `AccountRecovered` and `DeviceAuthorized`, epoch
   +1, device nonce +1, exactly one active device/admin, R3 installed, pending
   state deleted and replacement key permission `3`.
8. Require every pre-recovery key and presigned action to fail ERC-1271 and
   stateful authorization. Require the replacement key to pass.

This stage consumes at least 72 real hours. Its start, maturity and finalization
blocks and timestamps are activation prerequisites.

## Registry canary

After recovery succeeds, generate an independent random canary Shot ID and
registration salt. Construct a `tohseno.public-checkpoint/1` for witness-local
sequence 1 with no private ancestry or application data and verify its
RFC 8785/SHA-256 commitment independently.

1. Read account A's `registrationNonces` value and choose a deadline whose
   inclusive 24-hour commitment window can complete safely.
2. Reproduce `registrationCommitment(controller,shotId,salt,deadline)` both
   locally and through the contract.
3. Submit `commitShot` through a relayer. Verify exactly one
   `ShotCommitmentRecorded` event and the original `committedAt`.
4. Call the identical `commitShot` at the same state with `eth_call`; require
   `false` and unchanged timestamp, proving duplicate idempotence without a
   second transaction.
5. Prove reveal refusal before `committedAt + 60`. At or after that inclusive
   boundary and before both deadline and `committedAt + 24 hours`, sign the
   exact `RegisterShot` digest with account A and relay `registerShot`.
6. Verify commitment deletion, registration nonce +1, controller A, checkpoint
   1, action nonce 1, exact head and complete `ShotRegistered` event.
7. Construct public checkpoint sequence 2, sign and relay
   `appendCheckpoint`. Verify prior/new heads, checkpoint 2, action nonce 2 and
   event.
8. Through read-only calls, require refusal of the prior append replay, stale
   head, skipped sequence, wrong nonce, expired deadline and malformed
   signature.
9. Deploy account B through the exact factory and verify it as above. Sign and
   relay `transferShot` from account A to B without changing the head or
   checkpoint sequence. Verify action nonce +1 and `ShotTransferred`.
10. Require old controller A to fail a subsequent append and controller B to
    pass an otherwise equivalent fresh append. If broadcasting that final
    append is authorized, verify checkpoint 3; otherwise preserve the successful
    `eth_call` and leave chain state at checkpoint 2.

## Independent observation and report

An independent observer must reproduce every address, digest, signature result,
transaction input, event, nonce, counter, head, sequence, timestamp and runtime
hash from canonical blocks. At least two RPC observations must agree on final
state. Preserve an immutable report containing only public canary facts and:

- the generation and deployment-evidence digests;
- canary authorization and maximum spend, without secrets;
- all public keys, addresses, random IDs/salts and EIP-712 action bytes;
- simulations, signed transactions, receipts and canonical blocks;
- the complete 72-hour recovery timeline;
- all positive and negative assertions with exact observed outputs;
- final balances and total gas cost; and
- an explicit `pass`, `fail`, or `abandoned` disposition.

A pass does not activate the generation. It only permits construction of the
canonical activation payload for the separate offline threshold-signing and
client trust-root ceremony.
