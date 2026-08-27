# TOHSENO production lifecycle prompt

Status: historical, partially completed operational prompt. Generation 0.8.0
was subsequently activated under the evidence in
`release/contract-activations/`. Do not execute this document as a current
resume instruction, infer transaction authority from it, or repeat its
deployment/activation phases. Current truth is defined by `protocol/`, accepted
ADRs, and `docs/STATE.md`; the registry product workflows remain unimplemented.

You are operating in the `jpfraneto/tohseno` repository. Your objective is to
take the successor TOHSENO contract generation through independent review,
production deployment as an inactive candidate, production canary testing,
threshold activation, and one complete product lifecycle from private creation
through public discovery and an optional Bankr Appcoin relationship.

Read `AGENTS.md`, all normative material in `protocol/`, ADR 0006, ADR 0007,
`docs/STATE.md`, `docs/MIGRATION_0_8_CONTRACT_GENERATION.md`,
`contracts/README.md`, and `node/README.md` before acting. Protocol and accepted
ADR authority always wins over this operational prompt.

Also read `release/CONTRACT_0_8_0_PRODUCTION_READINESS.json`,
`release/CONTRACT_0_8_RELEASE_AUTHORITY_RUNBOOK.md`, and
`release/CONTRACT_0_8_0_DEPLOYMENT_AUTHORITY_PROPOSAL.md`. Read the prepared,
non-authorizing operator procedure at
`release/CONTRACT_0_8_0_INACTIVE_DEPLOYMENT_CEREMONY.md`. For the separate human
review, use `release/CONTRACT_0_8_MANUAL_AUDIT_BRIEF.md`. These are preparatory
documents only; never infer provider selection, spend or owner acceptance from
their existence.

## Execution mode

Treat this as a long-running, persistent objective. If the runtime supports
persistent goals, create one for the complete objective before beginning. Work
autonomously through every safe read, build, test, audit, evidence, simulation,
canary and integration step. Preserve machine-readable progress so a restarted
session resumes instead of repeating payments or broadcasts. Continue through
ordinary failures by diagnosing and fixing them within scope.

Do not stop merely to narrate progress. Stop only for a real authorization or
external-state boundary: a new payment, acceptance of a normative policy/ADR,
release-authority signatures, a broadcast confirmation not already granted by
this prompt, missing credentials that cannot safely be created, or a failed
security gate that changes the immutable candidate. When stopping, leave the
repository ready to resume and state the single exact action required.

## Governing outcome

The target chain is Robinhood Chain mainnet, chain ID 4663. Production testing
must use only dedicated canary identities and random canary Shot IDs. Never put
private intention, feedback, references, installation identity, user data, or a
hash of guessable private material on-chain.

The contracts are immutable and administrator-free. Never use "fix it after
activation" as a release strategy. The safe production sequence is:

1. audit and freeze a generation;
2. deploy it to production but leave it inactive and untrusted;
3. run production canaries against the exact deployed bytes;
4. abandon it and create a new generation if anything is wrong;
5. threshold-activate it only after every gate passes.

A deployment is not an activation. A CREATE2 prediction is not a deployment.
An activation with valid signatures is not trusted unless the client already
pins the corresponding release-authority policy digest.

## Current resume checkpoint — do not redeploy

Generation 0.8.0 was deployed successfully on Robinhood Chain mainnet on
2026-08-01 UTC as an inactive, untrusted candidate:

- factory `0xb1bd208cd2af98e701f43d06aaa889d3a594df65`, transaction
  `0x259f8f6d7fc09b392e46928066d172c3cce8f436c7a63591c64ec9c58409a5ef`;
- registry `0x3fe6508ba2660bc575080024f402c192a2e035a0`, transaction
  `0x1f7d2ccc24f66e9826b2a2729808a2fcc58dfd2f830ee10631ebf30eac72de91`;
- public evidence
  `contracts/audits/robinhood-inactive-deployment-0.8.0-20260801T021920Z.json`.

Never sign, submit, replace, or retry either deployment transaction. The
one-time ADR 0009 authorization is consumed. `main` still forbids a reusable
deployment command; preserve that boundary.

Post-deployment verification found that Solidity immutable references make
compiler deployed-bytecode template hashes differ from instantiated runtime
hashes for BuilderAccount and ShotRegistry. ADR 0010 and both activation
validators now distinguish them without changing any Solidity or generation
bytes. Begin by independently reviewing that small normative/tooling change,
running its tests, and verifying live code against the deployment evidence.
Then resume at Phase 6. Do not recreate completed audits, pay One Dollar Audit
again, or repeat the deployment ceremony.

## Funded transaction wallet

The human owner has designated the following limited-purpose transaction payer
for audit fees and, after every production gate and repository authorization,
the exact generation 0.8.0 inactive-candidate deployment ceremony:

- macOS Keychain service: `tohseno-onedollar-audit-2026-07-31`
- Keychain account: the current macOS `$USER`
- public address: `0x1eaF00a3F027275077253713C5bF7d0fAC44207F`
- Robinhood RPC: `https://rpc.mainnet.chain.robinhood.com`
- expected Robinhood chain ID: `4663`

At 2026-07-31T21:16Z the address held `0.000777` native Robinhood ETH. Treat
that as historical evidence only; query the balance again before simulation or
broadcast. This wallet is a transaction payer, not a Builder identity and not a
release authority. Its existence and funding do not waive audits, threshold
activation, the fresh P-256 gate, exact-address checks, or the repository's
current deployment tombstone.

The human owner's designated deployment use of that Robinhood ETH was consumed
by the two successful transactions above. The last verified balance after both
receipts was `681052043008000` wei. Do not infer permission to use the remainder
for a canary, another generation, activation, token launch, transfer, or an
arbitrary call. A canary needs the exact separate approval defined in Phase 6.

Historical read-only preflight at 2026-07-31T21:16Z observed:

- canonical CREATE2 deployer
  `0x4e59b44847b379578588920ca78fbf26c0b4956c`, 69-byte runtime, code hash
  `0x2fa86add0aed31f33a762c9d88e807c475bd51d0f52bd0955754b2608f7e4989`;
- predicted factory `0xb1bd208cd2af98e701f43d06aaa889d3a594df65`
  with no code;
- predicted registry `0x3fe6508ba2660bc575080024f402c192a2e035a0`
  with no code;
- a valid P-256 positive/negative/infinity probe measuring exactly 6,900 gas.

These are historical pre-deployment observations. The targets are no longer
empty; compare live bytes and receipts to the completed deployment evidence.

A later candidate-specific read-only preflight is recorded at
`contracts/audits/robinhood-contract-candidate-preflight-2026-07-31T234600Z.json`.
It rebuilt the frozen artifacts, verified the canonical deployer and empty
targets, simulated both exact CREATE2 calls, and checked payer nonce, balance
and gas. Its verifier is
`scripts/verify-contract-candidate-preflight.py`; its safety test is
`scripts/tests/test-verify-contract-candidate-preflight.sh`. The record is
historical and explicitly non-reusable for broadcast.

## Phase 1 — Establish the exact candidate

1. Require a clean or understood worktree. Preserve unrelated user changes.
2. Resolve the versioned generation definition from
   `contracts/generations/<version>/generation.json`. Never deploy the retired
   v0.7 generation or the unversioned `next` projection.
3. Recompute its canonical definition digest, source tree, compiler profile,
   ABI hashes, creation/runtime code hashes, CREATE2 coordinates, and portable
   artifacts with the repository's own validators.
4. Confirm the generation source commit is immutable. If any audited source
   must change, create a new semantic generation such as `0.8.1`; never rewrite
   the committed `0.8.0` definition in place.
5. Write a machine-readable readiness record listing the exact commit,
   generation digest, chain, compiler, contracts, runtime hashes, predicted
   addresses, test commands, and outstanding blockers. Do not call it an
   activation or deployment plan.

## Phase 2 — Run all local security gates

Run and preserve the results of at least:

```sh
cargo fmt --all --check
cargo test --locked --workspace --all-targets --all-features
swift build --package-path apple-identity
swift test --package-path apple-identity
swift test --package-path fascia/apple
forge fmt --check --root contracts
forge build --sizes --root contracts
forge test --root contracts -vvv
forge snapshot --root contracts --check .gas-snapshot \
  --fuzz-seed 0x746f6873656e6f --fuzz-runs 256
./scripts/tests/test-probe-p256.sh
./scripts/tests/test-verify-contract-candidate-preflight.sh
./scripts/tests/test-contract-production-readiness.sh
./scripts/build-contract-abi.sh --check
node --test studio/tests/static_assets.test.mjs
```

Also run any available Solidity static analyzers. Classify tool output by
exploitability; gas and naming lints are not security findings. Review every
external call, signature domain, nonce, deadline, epoch, recovery transition,
counter, CREATE2 calculation, commit/reveal boundary, ERC-1271 return value,
EIP-7702 exclusion, and timestamp edge manually.

## Phase 3 — Commission external review

Resume from the two completed independent AI reports and their disposition:

- `contracts/audits/FABLE_5_AUDIT_0_8_0_2026-07-31.md`;
- `contracts/audits/GPT_5_6_SOL_AUDIT_0_8_0_2026-07-31.md`; and
- `contracts/audits/INDEPENDENT_AI_AUDIT_DISPOSITION_0_8_0_2026-07-31.md`.

Do not rerun either model audit merely because a session restarted. Verify the
recorded hashes from the readiness record. The independent-AI gate is complete,
but GPT finding M-01 remains an open Medium operational finding until proposed
ADR 0008 is accepted or rejected. The P-256 mock gap and Rust action-coordinate
gap were remediated without changing the frozen contract bytes; rerun their
tests and preserve the results.

The unresolved One Dollar Audit recovery is no longer the only route to the AI
gate. Continue checking its issue without paying again; if the provider later
creates the already-paid job, preserve and disposition its report as additional
evidence. Do not count the two model reports as a human audit or formal
verification.

If the provider supplies recovery or the human separately authorizes a fresh
retry, fetch `https://www.onedollaraudit.com/skill.md` again and follow its
then-current x402 procedure. Treat any resulting report as another independent
AI pass, not the final manual audit required for immutable production
contracts. Do not start a new paid request merely because this prompt says to
resume the lifecycle.

This workstation has the limited-purpose transaction signer above stored in
macOS Keychain:

- Keychain service: `tohseno-onedollar-audit-2026-07-31`
- Keychain account: the current macOS `$USER`
- expected public address: `0x1eaF00a3F027275077253713C5bF7d0fAC44207F`

Retrieve it only at the last responsible moment with
`security find-generic-password -a "$USER" -s
"tohseno-onedollar-audit-2026-07-31" -w`, capturing stdout directly into a
non-exported shell variable. Disable shell tracing first. Never display the
result, interpolate it into a logged command, pass it as a command-line
argument, write it to a file, include it in a prompt, or inspect the process
environment. Derive the address inside the payment process and fail closed
unless it exactly matches the expected address above. Expose the key as
`PRIVATE_KEY` only to the single payment subprocess, then unset all copies.
Keychain access may require the human to approve a macOS prompt.

Before asking for payment authorization, query the Base USDC `balanceOf` for
the expected public address and report only that address and balance. If the
wallet is unfunded, stop and ask the human to send Base USDC to the public
address. Never request ETH for the gasless EIP-3009 path. The presence of this
credential does not itself authorize a payment: require a fresh explicit human
confirmation of the exact live amount, token, network, recipient, endpoint and
audit scope. After commissioning exactly one job, persist its job ID before
polling and do not pay again for that scope.

An initial 1 USDC request on 2026-07-31 settled in Base transaction
`0x3447833c959f8adf82b5b7d17b47c57bca9271adcb8d5161ea2d38e405c5f994`,
but no audit job was created. The 41,237-byte inline source bundle made the
provider's subsequent on-chain `postJobFor` estimate exceed the Base gas limit.
The provider settles x402 before creating the job. See
`contracts/audits/ONEDOLLAR_AUDIT_2026-07-31.md`. Never treat this payment as a
job, invent a job ID, or repeat the oversized request. Seek provider recovery
or require a fresh explicit authorization before spending another 1 USDC.
The recovery request is public as
`https://github.com/clawdbotatg/leftclaw-services/issues/58`; resume from a
provider-created job ID or retry credit and never commission a duplicate merely
because a new session began.
The read-only recovery trace in
`contracts/audits/ONEDOLLAR_AUDIT_RECOVERY_EVIDENCE_2026-07-31.json` proves
that the provider contract still had no job for the client after later jobs
were created, while the pay-to wallet retained the exact post-payment USDC
balance. Its evidence comment is linked from the readiness record. Check the
issue and contract again before concluding that recovery is still pending.

The engagement is one tight system:

- `contracts/src/P256Verifier.sol`
- `contracts/src/EIP712Domain.sol`
- `contracts/src/IERC1271.sol`
- `contracts/src/BuilderAccount.sol`
- `contracts/src/BuilderAccountFactory.sol`
- `contracts/src/ShotRegistry.sol`

For a compact retry, do not paste the source inline. The generation's exact
source commit is public at
`https://github.com/jpfraneto/tohseno/tree/862ca6cd3d396271b56b336fee0513ddcf6ecc64/contracts`.
Submit immutable raw GitHub URLs for each of the six paths, their hashes from
the generation definition, generation version and digest, target chain, and
these review priorities: P-256 input/return semantics; low-s enforcement;
ERC-1271 authorization; device permissions and counters; delayed recovery,
veto and finalization; nonce/deadline replay; cross-chain and cross-contract
domain separation; CREATE2 behavior; commit/reveal front-running and griefing;
checkpoint and transfer integrity; EIP-7702 controllers; malicious ERC-1271
return data; denial of service; and invariant completeness.

The description is public. Include no secrets. Before any payment:

- inspect the live 402 response;
- require exact USDC on Base and a total no greater than 1.00 USDC;
- require a human-approved, dedicated funded signer;
- never print, log, persist, or request a raw private key in chat;
- stop if the price, asset, chain, recipient, or endpoint differs from the
  freshly fetched skill and payment terms.

Persist the returned job ID and status URL immediately in a non-secret audit
tracking record. Never repay because polling or context was lost. Poll until
complete, save the delivered report with its URL/hash, and reproduce every
finding locally. Critical, high, and medium findings block deployment. Resolve
or explicitly disposition every low/informational item. Any source change
creates a new generation and requires the audit loop again.

Then obtain a separate human or competitive audit of the final frozen
generation. Record scope, exact generation digest, findings, remediations, and
auditor attestation. Do not label the generation independently audited until
this exists.

## Phase 4 — Establish release authority

Design and review the release-authority policy already defined by the protocol:

- dedicated offline P-256 release keys, never Builder or installation keys;
- an explicit threshold greater than one for production;
- unique ordered authority key IDs and a pinned policy digest;
- documented key custody, loss, rotation, incident, and successor-generation
  procedures;
- deterministic generation and activation signing ceremonies;
- no private key material in the repository, shell history, logs, screenshots,
  prompts, or CI.

Exercise threshold signing against fixtures first. Independently verify the
policy and signatures with two implementations where possible. The client
trust-root decision requires explicit human authorization.

The public-only policy preparation tooling is already present:

```sh
./scripts/tests/test-prepare-release-authority-policy.sh
scripts/prepare-release-authority-policy.py --help
```

After, and only after, the owner accepts the proposed 2-of-3 design and three
offline devices create independent production keys, follow
`release/CONTRACT_0_8_RELEASE_AUTHORITY_RUNBOOK.md`. Supply only the three
public P-256 coordinates to the preparer, reproduce its policy digest with the
independent Rust example, and ask the owner to approve that exact digest. Never
run fixture key generation as a substitute for the production offline ceremony.

## Phase 5 — Completed inactive production ceremony

Treat this phase as complete. Verify, but do not reproduce, the public evidence
and live code. The exact signed transaction bytes and receipts remain in
owner-controlled restricted storage; the ephemeral ceremony keystore was
destroyed. ADR 0008 and ADR 0009 are accepted, and ADR 0009's one-time authority
is consumed.

## Phase 6 — Production canary before activation

The freshly deployed contracts remain inactive. Do not ship their coordinates
as trusted authority yet.

Before spending canary funds, independently review ADR 0010 and the activation
validator changes and require all Rust and independent Python verifier tests to
pass. This review is about release metadata, not a reason to alter or redeploy
the exact Solidity contracts.

Follow `release/CONTRACT_0_8_0_PRODUCTION_CANARY_RUNBOOK.md`. It is a prepared,
non-authorizing procedure and does not grant a canary transaction budget or
permit reuse of the contract-deployment payer. Obtain separate owner approval
for exact canary funders, relayers and maximum spend before any transaction.

Using dedicated canary keys and no user material:

1. deploy a canary BuilderAccount through the exact factory and verify its
   predicted address and runtime;
2. verify valid and invalid P-256 ERC-1271 results against the production
   precompile;
3. authorize protocol-only and admin devices, exercise refusal of invalid
   permission bits, revoke devices, and prove active device/admin counters;
4. set and rotate recovery, initiate recovery, verify admin veto, initiate
   again, wait the real three-day production delay, finalize permissionlessly,
   and prove the prior epoch is invalid;
5. create a random canary Shot ID and ancestry-free public checkpoint;
6. submit a registry commitment, prove duplicate idempotence, wait the real
   60-second minimum, reveal through a relayer, and verify checkpoint 1;
7. append a checkpoint, reject stale/replayed actions through read-only calls,
   transfer to a second controlled canary account, and prove the old controller
   can no longer authorize;
8. verify every event, nonce, head, checkpoint sequence and runtime hash from an
   independent RPC/indexer;
9. write an immutable canary report containing only public test facts.

The recovery canary makes this ceremony take at least three days. Do not waive
it. Any unexplained mismatch abandons the candidate; create and audit a new
generation rather than rationalizing production behavior.

## Phase 7 — Threshold activation

Construct the canonical activation record only after the canary passes. Bind
the exact generation digest, chain, factory/registry addresses, approved
BuilderAccount runtime, deployment transactions and blocks, observed runtime
hashes, canonical activation block, and fresh production P-256 evidence.

Collect the policy threshold of offline signatures. Verify canonical bytes,
domain separation, ordering, uniqueness, threshold, prior activation link and
all chain evidence independently. Publish the policy and activation evidence.
Update clients to pin the explicitly approved policy digest and resolve the
activation. A client that lacks the trust root or any evidence must continue to
report `active_generation: null`.

## Phase 8 — Complete the product lifecycle

Activation alone is insufficient. ADR 0007 requires an accepted additive
`tohseno.app-metadata/3` publication receipt and successor Apple Fascia; the
engine, CLI, Studio and node also need a verified activation resolver and
receipt-aware public paths. Read
`release/POST_ACTIVATION_PRODUCT_GAP_AUDIT.md`. Do not mutate or reinterpret
the frozen `/2` metadata or sealed v0.7 Fascia, and do not treat ordinary node
lineage ingestion as publication.

After those accepted designs and implementations pass their cross-language and
privacy gates, run one dedicated production canary app through:

1. secure hardware-backed Builder identity creation and BuilderAccount
   deployment;
2. coherent intention, intelligent app-specific conception, local Apple
   capability resolution, accepted Genome and Birth Plan, complete Apple
   expression, Release build, target-user Simulator and applicable physical
   trials, independent protocol-conformance / intent-fidelity /
   experience-verification acceptance, signature, installation and accepted
   birth;
3. exact-version private feedback, selected evolutionary intent, evolution,
   verification and second accepted Version;
4. explicit publication opt-in using only the ancestry-free public checkpoint;
5. registry commit/reveal and receipt verification;
6. ingestion of public checkpoint plus receipt by a node;
7. explicit peer synchronization and independent discovery from a second node;
8. public page/metadata rendering that clearly distinguishes local versions,
   registry checkpoint sequence, controller authority and artifact
   availability.

Do not claim network feedback is complete until there is a bounded remote
feedback transport, spam/authentication policy, exact-version binding, and an
owner-controlled acceptance path. Remote feedback must never become an
authorized Shot transition merely because a node received it.

## Phase 9 — Bankr Appcoin lifecycle

For a verified selected Shot with no current token association:

1. use a dedicated Bankr user key with token-launch access;
2. choose the fee/vesting recipient explicitly;
3. provide HTTPS metadata and verify the live image preview;
4. simulate first and pin chain, token address, resolved recipient and exact
   configuration digest;
5. require the single-use typed confirmation and process-level deployment
   unlock;
6. broadcast once and handle uncertain outcomes without retry;
7. verify the transaction independently and record the private signed Token
   Association with the exact Shot-derived symbol.

Bankr token deployment does not publish a Shot and is not a TOHSENO registry
write. Keep the association private unless a separately accepted protocol and
ADR define the closed ancestry-free public token-relation record required by
ADR 0006. Never smuggle an ordinary private-lineage digest into a registry head.

## Completion report

Finish with one evidence-backed report containing:

- exact commits and generation/policy/activation digests;
- audit engagements, reports, findings and dispositions;
- every local gate and production canary result;
- transactions, blocks, addresses and runtime hashes;
- trust-root and activation verification;
- end-to-end app, node, feedback and Bankr outcomes;
- all remaining limitations and operational runbooks;
- an explicit statement of what was deployed, what was activated, what remains
  private, and what can be abandoned or superseded.

Never report success because tests passed alone. Never report deployment from a
predicted address. Never report activation from an unsigned record. Never
report network discovery from one local node. Never report a token as Shot
identity or ownership.
