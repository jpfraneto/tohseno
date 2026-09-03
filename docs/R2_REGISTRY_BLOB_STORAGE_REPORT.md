# Durable Registry blob storage

Status: implemented and locally verified on 2026-09-03; production cutover is
not performed.

## Result

The Registry now has one injected persistence seam for immutable public source
and icon bytes. `REGISTRY_BLOB_STORE=filesystem` preserves the existing sharded
layout. `REGISTRY_BLOB_STORE=r2` selects a private Cloudflare R2 bucket and uses
only these object namespaces:

```text
pending/<32-lowercase-hex>/source
pending/<32-lowercase-hex>/icon
sha256/<64-lowercase-hex>
```

Catalog records, indexes, publication jobs, profiles, aliases, Claims state,
incoming upload staging, and migration evidence remain under `REGISTRY_ROOT`.
No protocol encoding, catalog schema, Claim rule, generation-0.8 ABI, or public
blob URL changed.

Every remote write is conditional/create-only. The store verifies declared
length and SHA-256 before upload, reads the pending object back, promotes it to
the final content-addressed key, and reads the final object back before the
catalog can become visible. Stream-bearing operations retry at most three times
with a fresh body on each attempt; integrity conflicts are never retried. ETags
are ignored as integrity evidence. A relayed
publication records durable pending completion before its first chain call;
retries preserve transaction hashes and therefore do not create a second
Registry or Claims transaction.

The stable public route supports full `GET`, `HEAD`, and one inclusive byte
range. Missing objects are `404`, provider/transient failures are `503`, and an
integrity disagreement is a hard `500`. Pending keys have no public route.

## Configuration

R2 selection requires all four server-only secrets and fails startup if any is
absent or malformed:

```text
REGISTRY_R2_ACCOUNT_ID
REGISTRY_R2_BUCKET
REGISTRY_R2_ACCESS_KEY_ID
REGISTRY_R2_SECRET_ACCESS_KEY
```

The endpoint is derived as
`https://<account-id>.r2.cloudflarestorage.com` with region `auto`; there is no
endpoint override. Startup summaries expose only the selected backend, never a
bucket, account, key, or secret.

## Existing production inventory

A read-only check of the current production volume on 2026-09-03 found one
catalog release and one permanent local blob:

| Fact | Observed value |
| --- | --- |
| App | Anky |
| Release digest | `0xbfedc96908c631e6cb65bade0e7ee3d3002e0afb08d82a797d435f50211a0744` |
| Source digest | `0xb39de082c43c69a3dc517578f319a3fe878c455961ea5ae015106cbd24884bec` |
| Source bytes | `461076480` |
| Checkpoint sequence | `1` |

The mounted source was streamed through SHA-256 and matched that catalog
digest. This proves the local migration source, not an R2 copy.

The unchanged public filesystem-backed URL was also checked from outside the
hosting platform: `HEAD` returned `200` with length `461076480` and the exact
digest header; `Range: bytes=0-31` returned `206`, an inclusive
`Content-Range`, and exactly 32 bytes. This is the working pre-cutover baseline,
not evidence that production uses R2.

The linked production service has no `REGISTRY_R2_*` variables. The accessible
Cloudflare account has no dedicated Tohseno Registry bucket. Therefore no R2
bucket was created, no credentials were minted, no bytes were migrated, no
selector was changed, and no Bucket Lock rule was applied.

The backward-compatible source was deployed to the real service with the
filesystem selector on 2026-09-03 as Railway deployment
`9f35f2fe-9e55-4544-91b9-311fb276cad3`. Afterward `/healthz` and the Registry
status route succeeded, Anky `HEAD` returned its signed length/digest, and
`bytes=0-31` returned `206` with the exact inclusive range and 32 bytes. This
is deployment/cutover-baseline evidence only; it is not an R2 migration.

## Migration and rollback

`bun run registry:r2:migrate --dry-run` inventories every canonical permanent
local blob, independently hashes it, checks all catalog references and declared
source lengths, counts unreferenced objects, identifies the newest Anky record,
fingerprints the exact catalog and Anky record, and makes no R2 call or local
audit write. An explicit `--source-commit=<40-lowercase-hex>` keeps the evidence
usable in CLI-uploaded production images that have no `.git` directory.

`bun run registry:r2:migrate --apply` repeats that audit, writes each digest
create-only through the same blob-store seam, verifies every destination by
full readback, confirms the catalog fingerprint did not change during the
operation, removes only its temporary pending object, retains every local byte,
and writes a machine-readable audit under
`REGISTRY_ROOT/r2-migration-audits/`. Reruns are idempotent and deduplicate by
final digest.

Before cutover, Registry writes and both relayers must stay dark. Rollback is a
selector change only while every catalog-referenced digest still exists in the
local canonical layout. Any release first published after cutover must be
copied back and independently verified before selecting `filesystem`.

## Verification completed

Focused Bun tests cover filesystem compatibility, exact source and icon keys,
conditional writes, metadata, readback hashing, corrupt-existing-object
rejection, typed provider failures, fail-closed configuration, secret-safe
startup output, full/range/HEAD public reads, invalid ranges, private pending
keys, no relay before durable storage, post-chain retry without duplicate
transactions, active-job expiry retention, migration dry-run, corrupt-local
failure, apply, preservation, deduplication, and rerun. Website TypeScript also
passes.

The owner-attended cutover sequence, live smoke command, credential rotation,
Bucket Lock boundary, and rollback checks are in
`docs/runbooks/PERSON_TO_PERSON_NETWORK.md`.

## Exact remaining action

An authorized Cloudflare/Railway owner must create a private dedicated bucket
and bucket-scoped Object Read & Write credential, place the four secrets in production,
keep writes dark, run and review dry-run then apply, deploy with the R2 selector,
and execute the Anky full/range read smoke. Only after healthy observation may
the owner lock the final `sha256/` prefix; `pending/` must remain deletable.
