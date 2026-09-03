# Person-to-person network production runbook

This is the ADR 0034 activation order. It authorizes no contract-generation
deployment and no release claim without its real evidence.

The original 1.1 source candidate remains bound to its recorded commit. For
the current 1.2 line, this Registry/catalog order is necessary but not
sufficient: follow the separate
[`CLAIMS_V1_DEPLOYMENT_AND_ACTIVATION.md`](../../release/CLAIMS_V1_DEPLOYMENT_AND_ACTIVATION.md)
before any Claim write or advertising is enabled.

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

`REGISTRY_BLOB_STORE=filesystem` keeps the existing local permanent blob
layout. `REGISTRY_BLOB_STORE=r2` requires `REGISTRY_R2_ACCOUNT_ID`,
`REGISTRY_R2_BUCKET`, `REGISTRY_R2_ACCESS_KEY_ID`, and
`REGISTRY_R2_SECRET_ACCESS_KEY` from the hosting secret manager.
The R2 bucket is private. These credentials are server-only and must never be
placed in source, logs, browser variables, native bundles, or the Registry
root. Only immutable source/icon bytes move: every catalog/index, publication
job, Claim record, profile, alias, incoming staging file, and audit stays on the
durable mounted volume.

Enable `REGISTRY_RELAYER_ENABLED=true` only with one dedicated funded
`REGISTRY_RELAYER_PRIVATE_KEY`. The relayer is allowed to submit only the exact
active factory account creation and ShotRegistry commit/register/append calls
verified by the closed state machine. It is not an operator wallet, Builder
key, recovery key, or generic transaction endpoint. Never print the key.

The Registry root still needs normal encrypted backup/restore and free-space
alerts. Expired staging must be garbage-collected; pending remote objects for a
nonterminal publication job must not be collected. Public content-addressed
blobs are immutable. Public reads revalidate receipt block hashes against the canonical
block and current extending Shot state; alert on sustained RPC failures rather
than serving stale evidence as verified. Companion relay settings and storage
remain separate. External profile attestations stay disabled until the official
provider OAuth verifier is configured; never approve self-asserted proof URLs.

## R2 inventory and cutover

Cloudflare account access, bucket creation, credential creation/rotation, and
Bucket Lock are owner-attended actions. The application derives the official
account endpoint and uses region `auto`; it accepts no endpoint override. Keep
Registry writes and both relayers dark during the inventory/cutover window so
the audited local set cannot change underneath the migration.

1. Snapshot/backup `REGISTRY_ROOT`, record the deployed source commit, and
   confirm normal filesystem blob reads still work.
2. Create or select one private bucket and a bucket-scoped **Object Read &
   Write** credential. The application issues deletion only for temporary
   `pending/` objects; the later prefix Bucket Lock is the provider-side guard
   against deleting or overwriting final objects.
3. Add the four `REGISTRY_R2_*` secrets and `REGISTRY_BLOB_STORE=r2` to the service
   configuration without enabling writes.
4. From the exact deployed source, run the local-only audit first:

   ```sh
   cd website
   bun run registry:r2:migrate --dry-run
   ```

   Review `catalogRecordCount`, `catalogReferencedBlobCount`, `blobCount`,
   `unreferencedBlobCount`, `byteCount`, every digest, and the detected Anky
   release. Dry-run must make no R2 request and writes no audit file.
5. Apply once, then retain the resulting JSON evidence:

   ```sh
   bun run registry:r2:migrate --apply
   ```

   Apply re-audits every local permanent blob, uses conditional/create-only R2
   writes, streams every destination back through SHA-256/length verification,
   never deletes the local source, and records evidence under
   `REGISTRY_ROOT/r2-migration-audits/`. A failed item is not catalog promotion
   evidence; fix the concrete fault and rerun idempotently.
6. Restart/deploy with Registry reads using R2 while writes remain dark. Confirm
   the health/status routes, then verify a known Anky source through the public
   application route from outside the hosting platform:

   ```sh
   # Run inside the exact deployed service configuration first. This reads the
   # whole R2 object and its first range without printing bucket or credentials.
   bun run registry:r2:smoke \
     --confirm owner-attended-live-r2 \
     --digest 0xb39de082c43c69a3dc517578f319a3fe878c455961ea5ae015106cbd24884bec \
     --byte-length 461076480

   public_blob_url='https://tohseno.com/api/registry/v1/blobs/0xREPLACE_WITH_ANKY_SOURCE_SHA256'
   curl --fail-with-body --silent --show-error --head "$public_blob_url"
   curl --fail-with-body --silent --show-error "$public_blob_url" -o /tmp/tohseno-anky-source.tar
   shasum -a 256 /tmp/tohseno-anky-source.tar
   curl --fail-with-body --silent --show-error \
     -H 'Range: bytes=0-1048575' "$public_blob_url" -o /tmp/tohseno-anky-source.range
   cmp <(head -c 1048576 /tmp/tohseno-anky-source.tar) /tmp/tohseno-anky-source.range
   ```

   Use the exact digest/length from signed catalog evidence. If the object is
   shorter than 1 MiB, replace `1048575` with `byte_length - 1`. Confirm `HEAD`
   and full `GET` return `200`, range `GET` returns `206` with the inclusive
   `Content-Range`, and the full SHA-256 matches. Only an absent object may
   return `404`; an R2 outage must surface as `503`.
7. Enable the constrained write paths only after that external read works. Run
   one normal Companion-approved Ship or Update and confirm its job records
   durable pending bytes before the first transaction, publishes only after
   final readback, and cleans `pending/` afterward.
8. Retain the local pre-cutover blobs and migration audit through the observation
   period. After real traffic is healthy, the owner may apply Cloudflare Bucket
   Lock to the final `sha256/` prefix. Do not lock `pending/`, because verified
   cleanup requires deletion. Record the exact rule and retention semantics;
   application code must never create or weaken it.

Rotate a credential by creating an overlapping least-privilege credential,
changing the hosting secrets, restarting with writes dark, repeating the Anky
full/range check, and only then revoking the old credential. A partial or wrong
R2 configuration must stop startup rather than silently falling back.

## Safe activation order

1. Deploy backward-compatible Registry/catalog/blob code with write paths dark.
2. Deploy compatible Companion relay changes dark and verify health.
3. Merge and push the complete candidate source; run the full matrix and
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

For an R2 fault, first keep reads on R2 with writes dark while correcting a
credential/provider error. To return to `REGISTRY_BLOB_STORE=filesystem`, prove
that every digest referenced by the current catalog exists in the canonical
local sharded layout with the signed length and SHA-256. The retained migration
source already covers pre-cutover releases; any release published after cutover
must be downloaded privately from R2, independently hashed, and restored to its
exact local path before the configuration switch. Never point the catalog at a
different object, delete R2 bytes, or claim rollback while a current catalog
digest is absent locally.
