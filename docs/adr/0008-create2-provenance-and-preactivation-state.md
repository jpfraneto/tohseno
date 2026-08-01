# ADR 0008: CREATE2 provenance and pre-activation state fail closed

- Status: accepted for the generation 0.8.0 inactive-deployment boundary
- Date: 2026-07-31
- Applies to: proposed contract generation `0.8.0` inactive-candidate ceremony
- Security finding: GPT-5.6-Sol audit M-01

## Context

Generation 0.8.0 publishes its top-level factory and registry salts, init code,
and predicted addresses. The canonical singleton CREATE2 deployer is
permissionless. Any caller can therefore deploy the exact frozen bytecode to
the predicted coordinates before, or by copying, the intended ceremony call.

Different bytecode cannot occupy the same CREATE2 address under the same
deployer, salt, and init-code hash. Exact-code predeployment nevertheless
matters operationally:

- the intended payer's transaction will collide and fail;
- code equality alone does not prove the reviewed deployment transaction;
- the registry becomes permissionlessly writable immediately; and
- pre-activation events and state could be mistaken for trusted publication by
  a client that does not enforce the activation block.

This is a denial/provenance risk, not a route to substitute malicious runtime
bytes at the frozen addresses. It is still release-blocking until the project
defines fail-closed handling. The factory's per-account idempotence does not
dispose the separate top-level registry issue.

## Proposed decision

### Exact-code predeployment is not adopted

Generation 0.8.0 may be deployed as an inactive candidate only when both
predicted top-level addresses are empty at the ceremony's fresh, rechecked
canonical preflight block.

The activation evidence MUST identify successful deployment transactions sent
by the explicitly authorized payer and MUST reproduce their sender, nonce,
target, value, calldata, receipt, block, created address, complete runtime
bytes, and runtime hash. Runtime equality without those receipts is
insufficient.

If either address contains code before the authorized transaction lands—even
the exact expected code—or if another sender wins a mempool race, the ceremony
fails. Operators MUST inspect both target addresses and the authorized payer's
nonce/receipt before considering any retry. They MUST NOT adopt the existing
deployment, replay the colliding call, activate those coordinates, or describe
the generation as deployed by TOHSENO. Generation 0.8.0's coordinates are then
abandoned and any successor requires a new versioned generation and audit
cycle.

This rule accepts that a public deterministic coordinate can be denied by a
third party. It prevents that denial from becoming a provenance or activation
confusion failure.

### Deployment does not make registry state trusted

The registry is intentionally permissionless and has no administrator or
global activation switch. Its state may change after the valid deployment
receipt and before TOHSENO activation. Such mutations are protocol-neutral
contract activity, not TOHSENO publication evidence.

The activation record's canonical `activation_block` is the lower trust
boundary for publication receipts. Clients MUST NOT treat an event before that
block as an activated TOHSENO publication, even if its controller and
checkpoint are otherwise well formed. A pre-activation registration can become
relevant only through a separately specified post-activation receipt and
conformance path; the future app-metadata/3 design must define this explicitly
and may require a post-activation checkpoint action.

The three-day canary report MUST enumerate every registry event and affected
Shot ID from the deployment block through the proposed activation block using
two independent RPC/indexer views. Unknown third-party activity is recorded,
not mislabeled as a canary. It does not grant authority and it cannot be erased.

Random Shot IDs that remain private until their owner's commitment is live are
the availability defense against targeted squatting. A public Shot ID has no
contract-level reservation merely because an off-chain lineage claims it. A
conflicting neutral-controller registration is an availability dispute and
MUST NOT be rendered as proof of authorship or endorsement. Public clients must
validate the BuilderAccount generation, checkpoint provenance, activation
boundary, and owner-controlled receipt rather than trusting `controllerOf`
alone.

### Ceremony implementation remains narrow

An authorized one-generation ceremony must make the following conditions
machine-enforced and independently reviewed:

1. target addresses are empty at the pinned preflight block;
2. the exact payer nonce and exact signed transactions are displayed for final
   human confirmation;
3. there is no automatic retry after collision, timeout, replacement, or
   uncertain RPC response;
4. success requires the authorized payer's successful receipts, not merely
   code appearing at the target;
5. post-receipt code and event history are fetched from the receipt blocks and
   an independent RPC; and
6. any provenance mismatch permanently fails the 0.8.0 ceremony.

## Consequences

- An attacker can deny the public coordinates but cannot cause TOHSENO to
  authenticate an unofficial deployment under this policy.
- Exact-code predeployment is a generation-abandonment event, not a recoverable
  partial success.
- Pre-activation registry activity remains visible neutral chain history but is
  not activated publication evidence.
- Activation and the future publication-receipt schema must bind and enforce
  the activation-block boundary.
- This policy dispositions audit M-01 as a fail-closed denial risk. It does not
  by itself authorize deployment; ADR 0009 records the separate narrow
  inactive-deployment authority.
