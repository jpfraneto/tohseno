# Person-to-person network production runbook

This is the ADR 0034 activation order. It authorizes no contract-generation
deployment and no release claim without its real evidence.

## Immutable coordinates

- chain: Robinhood Chain `4663`
- generation: `0.8.0`
- BuilderAccountFactory: `0xb1bd208cd2af98e701f43d06aaa889d3a594df65`
- ShotRegistry: `0x3fe6508ba2660bc575080024f402c192a2e035a0`
- activation signing digest:
  `0x2b640260595def403343810d0dc4ee231e1faff427581be4f7b40cff4c189d28`

Verify these from `release/contract-activations/`, runtime-code commitments,
chain ID, live bytecode, and read-only ABI calls. Never substitute environment
coordinates for the signed activation.

## Server configuration

Deploy through the repository's existing Railway service and durable mounted
volume. Before enabling Registry reads, set an absolute non-symlink
`REGISTRY_ROOT`, HTTPS `ROBINHOOD_RPC_URL`, and reviewed
`REGISTRY_GLOBAL_RATE`, `REGISTRY_SOURCE_RATE`,
`REGISTRY_MAX_STAGING_RECORDS`, and `REGISTRY_MAX_STAGING_BYTES`.

Enable `REGISTRY_RELAYER_ENABLED=true` only with one dedicated funded
`REGISTRY_RELAYER_PRIVATE_KEY`. The relayer is allowed to submit only the exact
active factory account creation and ShotRegistry commit/register/append calls
verified by the closed state machine. It is not an operator wallet, Builder
key, recovery key, or generic transaction endpoint. Never print the key.

Registry storage needs normal encrypted backup/restore and free-space alerts.
Expired staging must be garbage-collected; public content-addressed blobs are
immutable. Public reads revalidate receipt block hashes against the canonical
block and current extending Shot state; alert on sustained RPC failures rather
than serving stale evidence as verified. Companion relay settings and storage
remain separate. External profile attestations stay disabled until the official
provider OAuth verifier is configured; never approve self-asserted proof URLs.

## Safe activation order

1. Deploy backward-compatible Registry/catalog/blob code with write paths dark.
2. Deploy compatible Companion relay changes dark and verify health.
3. Merge and push the complete 1.1.0 source; run the full matrix and
   `./scripts/test-network-e2e.sh`.
4. Build from that clean commit; Developer ID sign with hardened runtime,
   notarize, staple, mount, Gatekeeper-check, verify universal architectures,
   Finder layout, embedded CLI/Companion payload, exact manifest, and SHA-256.
5. From the exact public bytes, complete clean-Mac install, new-shell CLI,
   Companion install/pair, create/evolve, existing-project init/deploy,
   second-person Install/Fork/Refresh, and exact physical inventory checks.
6. Enable Registry writes and publish one real smoke Shot through normal
   Companion approval. Record BuilderAccount/commit/RegisterShot hashes and the
   resulting ShotID/release URL; no admin insertion.
7. Activate the immutable DMG URL/digest and website shipping copy only after
   every preceding fact is independently verified.

If a required secret or physical action is unavailable, leave the affected
gate dark and record exactly one blocked boundary. Do not replace it with mock
data, a direct database row, an admin transaction, or a weaker verifier.

## Verification

Local:

```sh
./scripts/test-network-e2e.sh
(cd website && bun run typecheck && bun test)
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

Public after activation:

```sh
curl --fail --show-error --silent https://tohseno.com/ >/dev/null
curl --fail --show-error --silent https://tohseno.com/registry >/dev/null
curl --fail --show-error --silent https://tohseno.com/api/registry/v1/status
curl --fail --show-error --silent https://tohseno.com/healthz
curl --fail --show-error --silent https://tohseno.com/api/native-release/v1
```

Then resolve the real smoke Shot page, exact release evidence, and source blob.
Confirm JSON/no-store headers for mutable evidence, immutable cache headers and
digest for the blob, deep links carrying exact release digest, relay health,
DMG redirect, one-line installer, and a byte-for-byte downloaded DMG hash.

## Rollback

Website/Registry reads and old immutable releases are backward-compatible.
Disable Registry writes/relayer first on an operational fault; do not mutate or
delete public release blobs or fabricate a chain rollback. Disable the new Mac
download channel if client acceptance fails. Preserve failed jobs and receipts
privately for diagnosis, revoke a compromised relayer at the deployment secret
boundary, and treat a compromised Builder key through BuilderAccount recovery,
not operator database edits.
