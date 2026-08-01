# One Dollar Audit engagement record

Date: 2026-07-31

Status: payment settled; provider job creation failed; no audit job exists.

## Candidate

- generation: `0.8.0`
- generation-definition digest:
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`
- generation source commit: `862ca6cd3d396271b56b336fee0513ddcf6ecc64`
- submitted source-bundle SHA-256:
  `624dc778536b305bc207476c4a92e7a496be2a83118b1b4d02f793cf778b7ed0`
- intended target: Robinhood Chain mainnet, chain ID `4663`

## Payment evidence

- payer: `0x1eaF00a3F027275077253713C5bF7d0fAC44207F`
- Base USDC: `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- recipient: `0xCfB32a7d01Ca2B4B538C83B2b38656D3502D76EA`
- amount: `1,000,000` base units, or `1 USDC`
- Base transaction:
  `0x3447833c959f8adf82b5b7d17b47c57bca9271adcb8d5161ea2d38e405c5f994`

## Failure

The paid request embedded all six Solidity files as a 41,237-byte tight-system
source bundle. The provider settled the x402 authorization first, then attempted
to call its Base `postJobFor` function with that public description. Gas
estimation exceeded the Base transaction gas limit and the API returned HTTP
500. No event from job contract
`0xb2fb486a9569ad2c97d9c73936b46ef7fdaa413a` accompanied the payment, and no
job ID, status URL or report was returned.

Do not poll a guessed job ID and do not count this as an external audit. Do not
repeat this payment automatically.

## Recovery and retry

First seek provider recovery or credit using the transaction evidence above.
The public recovery request was posted as
<https://github.com/clawdbotatg/leftclaw-services/issues/58> at
`2026-07-31T23:22:35Z`; await the provider response before any retry.
If a new payment is explicitly authorized, use a compact description containing
the generation digest and immutable public raw GitHub URLs for the six files at
commit `862ca6cd3d396271b56b336fee0513ddcf6ecc64`. Include their hashes from
`contracts/generations/0.8.0/generation.json`; do not paste the source on-chain.
Persist the returned job ID before polling.
