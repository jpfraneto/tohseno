# Claims activation evidence

This directory is the only repository location for signed activation evidence
for additive Claims contracts. It is separate from
`release/contract-activations/`: generation 0.8 remains the active Registry
generation and is not redeployed or amended by a Claims activation.

`signed-claims-activation-1.json` is the first activation. It satisfies the
already pinned 2-of-3 release-authority threshold and binds TohsenoClaimsV1 at
`0x5012703d48d99224ac0035d58bc373de9e8b1934` to the live generation-0.8
ShotRegistry. Its signing digest is
`0xec418380f588b9a6f72fc251b7a0ae7bee8a19a1d843017e4733ebd2d094966d`.

Deployment, an operator environment value, a relayer key, or a database row is
not activation. Claims writes remain separately gated by the owner-attended
physical acceptance in
[`../CLAIMS_V1_DEPLOYMENT_AND_ACTIVATION.md`](../CLAIMS_V1_DEPLOYMENT_AND_ACTIVATION.md).
