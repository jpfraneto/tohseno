# GENESIS candidate bundle

The GENESIS bundle is the reproducible, inspectable protocol-candidate
artifact. It is built by `scripts/build-genesis-bundle.sh` after all normative
schemas, vectors, contract sources, ABIs, and candidate deployment facts are in
their checked state.

It is not the permanent Arweave Genesis Bundle and must never be labeled
canonical.

The bundle contains:

```text
dist/genesis/
├── WHITEPAPER.md
├── SPECIFICATION.md
├── IMPLEMENTERS.md
├── CONFORMANCE.md
├── FASCIA.json
├── schemas/
├── test-vectors/
├── contracts/
├── ABI/
├── DEPLOYMENT.json
├── SOURCE_COMMIT.txt
├── FILES.sha256
└── GENESIS.json
```

Reproducibility uses `SOURCE_DATE_EPOCH` when supplied, otherwise the source
commit timestamp. Deployment fields remain explicitly `null` or `pending`
until real RPC evidence exists.

