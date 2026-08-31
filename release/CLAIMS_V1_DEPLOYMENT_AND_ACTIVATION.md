# TohsenoClaimsV1 deployment and activation

This is the fail-closed owner runbook for ADR 0035's additive Claims contract.
It does not authorize a deployment. The exact 1.2 source must first be one
reviewed, pushed, clean commit, the verification matrix must be green at that
commit, and the repository owner must explicitly begin the ceremony.

Generation 0.8 is already active. Never deploy or modify its Factory,
BuilderAccount, ShotRegistry, bytecode, ABI, activation, or public-policy
record during this procedure. The only constructor value permitted here is
the active Registry:

```text
chain                 4663
ShotRegistry          0x3fe6508ba2660bc575080024f402c192a2e035a0
Claims implementation contracts/src/TohsenoClaimsV1.sol
```

## 1. Capture source and prepare the workstation

Use a fresh checkout of the reviewed source commit. Confirm that `HEAD` is the
commit named in `release/V1_2_0_READINESS.json`, that `git status --porcelain`
is empty, and that these checks pass without regenerating tracked files:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
forge fmt --root contracts --check
forge build --root contracts --sizes
forge test --root contracts -vvv
scripts/build-contract-abi.sh --check
./scripts/test-network-e2e.sh
```

Record the exact Foundry version and obtain the existing governed deployment
credential through the owner-attended release environment. Do not put a key,
mnemonic, RPC credential, or signature in this repository, shell history,
Forge broadcast artifact, log bundle, or activation JSON.

## 2. Rehearse, then deploy exactly once

First simulate the script against a fork pinned to a fresh canonical block and
inspect the trace. `DeployClaimsV1.s.sol` refuses every chain except Robinhood
mainnet and hard-codes the active Registry. When the owner approves the exact
trace, run the same script with the governed signer using Foundry's interactive
or hardware-wallet path:

```sh
forge script contracts/script/DeployClaimsV1.s.sol:DeployClaimsV1 \
  --root contracts \
  --rpc-url "$ROBINHOOD_RPC_URL" \
  --broadcast \
  --slow
```

No salt, proxy, CREATE2 factory, administrator, initializer, second
constructor argument, or retry deployment is permitted. Preserve the
transaction hash and address printed by Foundry, then remove any local
broadcast file containing operator-sensitive metadata from the evidence copy.
A failed or ambiguous transaction stops the ceremony for independent chain
inspection; never guess or redeploy merely to obtain a preferred address.

## 3. Prepare the unsigned observation

The preparation tool is read-only. It requires HTTPS RPC without embedded
credentials, a completely clean checkout at the declared commit, and the
actual direct-creation transaction. It verifies chain ID, canonical receipt
and block, successful creation address, exact committed creation bytecode plus
the Registry constructor word, unchanged runtime code, the live immutable
`shotRegistry()` result, and the continuing active Registry runtime.

```sh
python3 scripts/prepare-claims-activation.py \
  --repository-root "$PWD" \
  --rpc-url "$ROBINHOOD_RPC_URL" \
  --policy "$PWD/release/contract-activations/release-authority-policy.json" \
  --source-commit "$(git rev-parse HEAD^{commit})" \
  --claims-contract 0x... \
  --deployment-transaction 0x... \
  --issued-at "$ISSUED_AT" \
  --output /absolute/offline/handoff/claims-activation-1.json
```

The source-tree digest is SHA-256 of `git archive --format=tar` for the exact
commit. Creation-code hash covers the committed unparameterized bytecode;
transaction equality separately proves the exact Registry constructor word.
Runtime hash covers the constructor-instantiated bytes observed both at the
deployment block and latest canonical state.

## 4. Obtain threshold approval offline

Each release-authority custodian independently inspects the source commit,
contract, deployment receipt/block, address, code hashes, Registry binding,
policy digest, and printed activation digest. On separate offline custody
devices, two policy members sign the exact canonical file:

```sh
python3 scripts/sign-contract-activation.py \
  --key /offline/custodian.pem \
  --activation /absolute/offline/handoff/claims-activation-1.json \
  --output /absolute/offline/handoff/claims-approval.json
```

The coordinator assembles only known, unique, low-s approvals over the same
digest:

```sh
python3 scripts/assemble-signed-activation.py \
  --activation /absolute/offline/handoff/claims-activation-1.json \
  --policy "$PWD/release/contract-activations/release-authority-policy.json" \
  --approval /absolute/offline/handoff/approval-a.json \
  --approval /absolute/offline/handoff/approval-b.json \
  --output /absolute/offline/handoff/signed-claims-activation-1.json
```

Verify with the independent Rust implementation under the already pinned
policy digest:

```sh
cargo run --quiet --locked -p tohseno-network \
  --example verify_signed_claims_activation -- \
  release/contract-activations/release-authority-policy.json \
  /absolute/offline/handoff/signed-claims-activation-1.json \
  0xf14410692ebe34f6855b8dbec5cb08733aa737f1cd86f385694e4fb575df943c
```

Copy the exact verified envelope to
`release/claims-activations/signed-claims-activation-1.json` in a new reviewed
commit. That commit must also pin the same signing digest and envelope in the
Rust engine, website configuration, Mac, and Companion. Partial pins fail
closed. Re-run live code, Registry, receipt, and canonical-block verification
immediately before review and after deployment of each consumer.

## 5. Dark read deployment, physical acceptance, then writes

Deploy the website with the complete Claims coordinates and
`CLAIMS_INDEXER_ENABLED=true` while `CLAIMS_RELAYER_ENABLED=false`. Rebuild the
index from the canonical deployment block, restart it, and compare the rebuilt
edition/Claim state with direct RPC. No Claim button or write advertisement is
live yet.

Only an owner-attended acceptance session may enable the constrained Claims
relayer. It must record every item in `V1_2_0_READINESS.json`: one real first
Ship with immutable edition; a second Tohseno identity's physical Companion
circle and canonical receipt; an offline Mac followed by automatic preparation
of the exact claimed release; recipient-local signed iPhone install; a later
Update preserving the Claim and edition; private Follow reconciliation; live
receipt/metadata routes; and exactly one Ship in the public timeline.

If any signature, code, block, Registry head, catalog release, Claim number,
private outbox, Xcode build, signing identity, physical install, or website
origin is ambiguous, leave Claims writes and advertising disabled. Preserve
the deployed contract as inactive evidence; do not fabricate, edit, pause,
upgrade, or administratively repair it.
