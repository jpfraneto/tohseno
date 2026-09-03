# Durable Registry blob storage

Status: implemented, migrated, externally verified, rollback-proven, and active
in production on 2026-09-03. Prefix Bucket Lock remains owner-attended.

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

The final backward-compatible source milestone was first deployed to the real
service with the filesystem selector on 2026-09-03 as Railway deployment
`7ad998ce-62e5-4172-b2d0-1a351e5557eb`. The four server-only credentials were
then installed together while the selector remained filesystem. The dedicated
bucket was confirmed private: no `r2.dev` access and no custom domain.

The mounted-production dry-run and apply both audited one catalog record, one
referenced blob, zero unreferenced blobs, and 461,076,480 bytes. Both preserved
catalog fingerprint
`0xc0ea7995eb3ed8889147c3bd31f20f529b6070b094679bb9cdaef38d1575e3f1`.
Apply returned zero failures and retained audit
`2026-09-03T14-43-22.214Z.json` on the Registry volume. Direct R2 smoke read the
complete object and its first 1 MiB and matched the expected hashes.

Railway deployment `b0f4b895-80b8-482c-bfff-5aad002a2a56` activated R2. An
external full `GET` through the unchanged public URL returned `461076480` bytes
and SHA-256
`0xb39de082c43c69a3dc517578f319a3fe878c455961ea5ae015106cbd24884bec`.
The public 1 MiB range returned `206`, exact inclusive `Content-Range`, and
matched the same bytes from the full stream.

Rollback deployment `4842146c-c81d-4f29-8d8e-dcbad6a0f68e` selected the
retained filesystem copy without reconstructing or rewriting it. Its external
full download and 32-byte range matched the same signed digest and length.
Deployment `13c62e73-910f-4d6e-812b-a32c5325ac61` then restored R2; health and
the public 32-byte range passed. No Ship, Update, Claim, signature, catalog
record, or chain state was created or changed by this operation.

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

With the verified R2 read path still healthy, an authorized owner may run this
in an attended Cloudflare session and review the confirmation before accepting:

```sh
wrangler r2 bucket lock add "$REGISTRY_R2_BUCKET" \
  registry-final-objects sha256/ --retention-indefinite
```

The rule must cover exactly `sha256/`. Do not add `--force`, do not lock
`pending/`, and do not lock the whole bucket. The application credential must
remain unable to create or weaken Bucket Lock rules.
