# One Dollar Audit payment recovery request

Target: public issue at
<https://github.com/clawdbotatg/leftclaw-services/issues/58>

Status: posted as GitHub issue #58 at `2026-07-31T23:22:35Z`.

At `2026-07-31T23:52:02Z`, a read-only Base recovery trace proved that the
provider contract still returns no jobs for the audit wallet while later jobs
have been created. The published pay-to wallet's USDC balance increased by
exactly 1 USDC in the settlement block and remained at that post-payment value
at the recovery check. The evidence was posted to issue #58 as
<https://github.com/clawdbotatg/leftclaw-services/issues/58#issuecomment-5148414853>
and is preserved in
`ONEDOLLAR_AUDIT_RECOVERY_EVIDENCE_2026-07-31.json`.

## Proposed public message

### Title

`x402 audit payment settled but oversized description created no job`

### Body

On 2026-07-31, one x402 payment for the One Dollar Audit endpoint settled but
the provider returned HTTP 500 before creating the corresponding audit job.

- endpoint: `https://leftclaw.services/api/audit`
- Base USDC payment transaction:
  `0x3447833c959f8adf82b5b7d17b47c57bca9271adcb8d5161ea2d38e405c5f994`
- payer: `0x1eaF00a3F027275077253713C5bF7d0fAC44207F`
- recipient: `0xCfB32a7d01Ca2B4B538C83B2b38656D3502D76EA`
- amount: `1.00 USDC`
- intended service: audit / service type 4
- intended candidate: TOHSENO contract generation `0.8.0`, definition digest
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`

The request used a 41,237-byte inline six-contract source description. The
payment settled first; the subsequent `postJobFor` gas estimate exceeded the
Base transaction gas limit. The receipt contains the USDC authorization and
transfer events but no event from the LeftClaw job contract
`0xb2fb486a9569ad2c97d9c73936b46ef7fdaa413a`. The API returned no job ID,
status URL, or report URL.

Could you please either:

1. create the missing audit job using the already-settled payment and a compact
   immutable-URL description; or
2. provide a credit/recovery mechanism for one compact retry?

Please do not ask for a private key. All candidate sources are public at these
immutable raw URLs:

- <https://raw.githubusercontent.com/jpfraneto/tohseno/862ca6cd3d396271b56b336fee0513ddcf6ecc64/contracts/src/P256Verifier.sol>
- <https://raw.githubusercontent.com/jpfraneto/tohseno/862ca6cd3d396271b56b336fee0513ddcf6ecc64/contracts/src/EIP712Domain.sol>
- <https://raw.githubusercontent.com/jpfraneto/tohseno/862ca6cd3d396271b56b336fee0513ddcf6ecc64/contracts/src/IERC1271.sol>
- <https://raw.githubusercontent.com/jpfraneto/tohseno/862ca6cd3d396271b56b336fee0513ddcf6ecc64/contracts/src/BuilderAccount.sol>
- <https://raw.githubusercontent.com/jpfraneto/tohseno/862ca6cd3d396271b56b336fee0513ddcf6ecc64/contracts/src/BuilderAccountFactory.sol>
- <https://raw.githubusercontent.com/jpfraneto/tohseno/862ca6cd3d396271b56b336fee0513ddcf6ecc64/contracts/src/ShotRegistry.sol>

Their exact hashes and the generation definition are public at commit
`e3f1396e38ad7180ff619dfc7e932ce797850c8e`:
<https://raw.githubusercontent.com/jpfraneto/tohseno/e3f1396e38ad7180ff619dfc7e932ce797850c8e/contracts/generations/0.8.0/generation.json>.

If a job was created through another path, please return its exact on-chain job
ID and status URL so we can persist and poll it without paying again.

## Posting rule

Recheck that the transaction, addresses, amount, generation digest, and compact
request URL are public and exact immediately before posting. Do not attach logs,
environment output, Keychain data, raw request headers, or any credential. A
human must explicitly authorize creating the public issue; preparing this file
does not authorize sending it.
