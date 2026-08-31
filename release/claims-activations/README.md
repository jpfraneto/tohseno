# Claims activation evidence

This directory is the only repository location for signed activation evidence
for additive Claims contracts. It is separate from
`release/contract-activations/`: generation 0.8 remains the active Registry
generation and is not redeployed or amended by a Claims activation.

The first possible evidence file is
`signed-claims-activation-1.json`. Its absence is intentional. The 1.2 source
and every deployed service/client keep Claims dark until that exact canonical
envelope exists, satisfies the already pinned release-authority threshold, and
binds the live Robinhood runtime to the active generation-0.8 ShotRegistry.

Deployment, an operator environment value, a relayer key, or a database row is
not activation. Follow
[`../CLAIMS_V1_DEPLOYMENT_AND_ACTIVATION.md`](../CLAIMS_V1_DEPLOYMENT_AND_ACTIVATION.md)
for the owner-attended ceremony and independent verification.
