# Read-only target observations

Files in this directory are time- and block-scoped audit observations. They
are not contract activations, deployment authorization, reusable deploy-gate
results, or proof that a later target block has the same behavior.

The generation 0.8.0 review set is:

- `PREAUDIT_0_8_0_2026-07-31.md` — internal manual review plus Slither/Aderyn
  triage;
- `FABLE_5_AUDIT_0_8_0_2026-07-31.md` — independent Claude Fable 5 AI review;
- `GPT_5_6_SOL_AUDIT_0_8_0_2026-07-31.md` — independent GPT-5.6-Sol AI review;
  and
- `INDEPENDENT_AI_AUDIT_DISPOSITION_0_8_0_2026-07-31.md` — local reproduction,
  remediation, and open-finding status.

AI reports are not human audits or formal verification. Their findings did not
authorize deployment. The disposition record's Medium operational CREATE2
finding is governed by accepted ADR 0008.

The authorized one-time deployment evidence is
`robinhood-inactive-deployment-0.8.0-20260801T021920Z.json`. It records both
successful transactions, canonical blocks, exact CREATE2 inputs, observed
runtime bytes and hashes, gas costs, and the inactive/untrusted boundary. It
also records the Solidity immutable-placeholder discrepancy discovered during
post-deployment verification; ADR 0010 corrects activation semantics without
changing the frozen Solidity generation.

`robinhood-p256-2026-07-30.json` was produced without a transaction by:

```sh
scripts/probe-p256.sh \
  --rpc-url https://rpc.mainnet.chain.robinhood.com \
  --output robinhood-p256-2026-07-30.json
```

The probe bound every call to one canonical block, tested the positive,
negative, and point-at-infinity EIP-7951 vectors, measured all three calls, and
rechecked the block hash. Its 6,900-gas observation informed generation 0.8.0.

Any future deployment or canary workflow must rerun the same complete probe
against its explicit actual target RPC immediately before broadcast. It must
not reuse this file.

`robinhood-contract-candidate-preflight-2026-07-31T234600Z.json` is a separate
historical, read-only observation of the frozen generation 0.8.0 deployment
candidate. It was produced by:

```sh
scripts/verify-contract-candidate-preflight.py \
  --rpc-url https://rpc.mainnet.chain.robinhood.com \
  --payer 0x1eaF00a3F027275077253713C5bF7d0fAC44207F \
  --output \
    contracts/audits/robinhood-contract-candidate-preflight-2026-07-31T234600Z.json
```

The verifier has a closed read-only JSON-RPC allowlist and no signer or
broadcast path. It rebuilt and byte-compared the frozen artifacts, pinned and
rechecked a canonical block, verified the CREATE2 deployer, proved that both
predicted targets were empty, simulated both exact calls, and recorded nonce,
balance, gas and cost estimates. This evidence is not reusable for broadcast;
the authorized ceremony must perform the same checks again synchronously.
