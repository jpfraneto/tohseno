# Read-only target observations

Files in this directory are time- and block-scoped audit observations. They
are not contract activations, deployment authorization, reusable deploy-gate
results, or proof that a later target block has the same behavior.

`robinhood-p256-2026-07-30.json` was produced without a transaction by:

```sh
scripts/probe-p256.sh \
  --rpc-url https://rpc.mainnet.chain.robinhood.com \
  --output robinhood-p256-2026-07-30.json
```

The probe bound every call to one canonical block, tested the positive,
negative, and point-at-infinity EIP-7951 vectors, measured all three calls, and
rechecked the block hash. Its 6,900-gas observation informed generation 0.8.0.

Any future deployment workflow must rerun the same complete probe against its
explicit actual target RPC immediately before broadcast. It must not reuse this
file.
