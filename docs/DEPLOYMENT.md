# Deployment

This repository is prepared for one boring Bun server with a persistent SQLite volume. These instructions prepare or execute a deployment only when the owner explicitly chooses to do so. Repository implementation does not deploy production, alter DNS, create paid resources, or submit applications to stores.

## Supported topology

```text
public HTTPS endpoint
        |
one Bun container/process
        |
persistent volume mounted at /data
        |
/data/tohseno.sqlite (+ SQLite WAL/SHM files)
```

Use one application replica. The rate limiter is in memory, and SQLite/WAL is not a distributed multi-writer design. Horizontal scaling requires a separate architecture decision for database coordination, idempotency, rate limits, backups, and token/auth failure state.

## Required production values

| Variable | Requirement |
|---|---|
| `NODE_ENV` | `production` |
| `PORT` | Platform-provided or chosen listen port |
| `BASE_URL` | Canonical public HTTPS origin, with no trailing private path |
| `DATABASE_PATH` | Persistent path; Railway: `/data/tohseno.sqlite` |
| `TOHSENO_BACKUP_PATH` | Optional one-off backup output; default is a timestamped file under `/data/backups/` |
| `TRUST_PROXY` | Enable only when the exact ingress topology is understood |
| `TOHSENO_DATA_KEY` | Base64-encoded 32 random bytes, stored in the platform secret manager |
| `TOHSENO_OPERATOR_TOKEN` | Independent high-entropy bearer secret |
| `PAYMENTS_MODE` | `stripe` or `disabled`; never `mock` |
| Stripe variables | Required together when payment mode is `stripe`; see [Stripe](STRIPE.md) |
| `EMAIL_MODE` | `resend` or `disabled` for production |
| Resend variables | Required when email mode is `resend`; see [Email](EMAIL.md) |

Startup validates core configuration and fails closed when the data key, operator token, production HTTPS origin, or selected Resend settings are missing. Stripe mode can start with incomplete price/webhook settings so the private status page can explain exactly which safe configuration name is missing; it cannot create Checkout or fake success until all required Stripe values exist. When payment is disabled, the landing page must disclose that fact before private intake. A reachable Stripe test-mode Checkout must be labeled as internal verification rather than a live founding-product purchase. The startup summary contains safe modes, paths, origins, and provider availability only—never secret values.

## Container build and local production rehearsal

Build from the repository root:

```sh
docker build -t tohseno:local .
```

Create an untracked environment file containing production-shaped test values, then run with a named volume and no mock payments:

```sh
docker volume create tohseno-data
docker run --rm \
  --env-file .env.production.local \
  -e DATABASE_PATH=/data/tohseno.sqlite \
  -v tohseno-data:/data \
  -p 3000:3000 \
  tohseno:local
```

The image uses the official Bun base. Its entrypoint starts briefly as root only to give the `bun` user ownership of the mounted `/data` directory, then replaces itself with the application under that non-root user. This is necessary because a mounted volume hides ownership prepared in the image layer. Do not bypass the image entrypoint, and do not bake `.env`, a SQLite database, or generated secrets into the image.

Verify `GET /healthz`, the landing/privacy pages, a synthetic intake, and the disabled or test Stripe boundary. Never send real customer data through a local production rehearsal.

## Railway preparation

The included `railway.toml` selects the Dockerfile builder and defines the `/healthz` check and restart behavior. The Dockerfile supplies the start command. In the Railway project:

1. Create or select the service from the repository.
2. Add a persistent volume and mount it at `/data`.
3. Set `DATABASE_PATH=/data/tohseno.sqlite`.
4. Add all required secrets through Railway variables, not repository files.
5. Set `BASE_URL` to the final HTTPS origin before enabling Stripe Checkout or email links.
6. Keep one replica.
7. Configure the Stripe webhook only after the stable HTTPS endpoint exists.
8. Confirm the volume survives a restart before accepting real submissions.

Railway attaches a newly mounted volume with platform ownership rather than the image-layer `/data` ownership. Keep the included Docker `ENTRYPOINT` intact: it corrects that single directory and drops privileges before starting Bun. Do not configure a Railway `startCommand` for this Dockerfile. Railway Docker start commands replace the image `ENTRYPOINT`, which would bypass both volume initialization and the non-root runtime boundary. The image's `CMD` already runs `bun run start`.

### Capability transport and Railway HTTP logs

Railway HTTP observability records request paths. A raw bearer capability therefore must never appear in a path or query string. TOHSENO returns private links with the bearer only after the `#capability=` fragment, which a browser does not send to Railway or include in a referrer. Browser bootstrap code reconciles that fragment and safe submission ID in a bounded JSON `POST` body for an expiry-bounded, host-only cookie dedicated to that submission and marked `Secure`, `HttpOnly`, and `SameSite=Strict`. Coding agents extract the fragment locally and send it through `Authorization: Bearer` to the matching safe-ID-scoped capsule route.

Expected Railway path fields include safe path families such as `/status/<submission-id>`, `/api/capability/session`, `/api/checkout`, `/c/<submission-id>`, and `/c/<submission-id>/MASTER_PROMPT.md`. Application logs use the corresponding `:id` route templates. Do not preserve legacy token-in-path routes, move the token to a query parameter, paste it into a log-search filter, or use a monitoring tool that reconstructs a bearer in its requested URL. Application logs must never record request bodies, Authorization headers, or cookies.

Creating a Railway service or volume may incur cost and requires explicit owner approval. This repository does not run `railway up`.

## Migration and release sequence

1. Back up the SQLite database, WAL state through a SQLite-consistent method, and separately verify custody of the matching data key.
2. Build and test the exact commit: `bun run check`.
3. Apply migrations using `bun run migrate` against a staged copy or during a controlled single-instance release.
4. Start one new process and wait for `/healthz`.
   After server construction, a caught background task atomically drains a bounded batch of pending/failed transactional email intents when email is enabled; health binding does not wait on Resend.
5. Smoke-test landing, privacy, synthetic intake/status, fragment-to-session exchange, coding-agent Authorization access, invalid capability behavior, operator authentication, and configured provider readiness.
6. Enable traffic.
7. Monitor structured status/error logs without adding private request bodies.

Migrations are forward changes. A container rollback cannot automatically undo a schema change or decrypt data with a previous key. Every future destructive migration needs its own compatibility and rollback plan.

## Backups and restore drill

Create an application-consistent backup while the service is running:

```sh
bun run backup
```

The command uses SQLite `VACUUM INTO`, so its standalone output includes all committed data visible through the active database connection, including committed pages currently represented by WAL state. It does not decrypt Markdown, contact details, or messages. It refuses in-memory databases, the active database/WAL paths, and any existing destination. It creates the backup with mode `0600`, runs `PRAGMA integrity_check`, and verifies that `schema_migrations` exactly matches the source before reporting success.

For `DATABASE_PATH=/data/tohseno.sqlite`, the default is a timestamped file such as:

```text
/data/backups/tohseno-2026-07-10T13-14-15.678Z.sqlite
```

Choose a specific new file when an operator-controlled destination is required:

```sh
bun run backup -- --output /data/backups/pre-release.sqlite
```

`TOHSENO_BACKUP_PATH` is the equivalent one-off environment setting; `--output` takes precedence. Never aim either setting at `DATABASE_PATH`. Successful JSON output contains only the backup path, timestamp, byte size, and migration versions. It contains no rows, decrypted values, capabilities, or keys.

Run the backup inside the mounted Railway service, note the safe pathname from its result, and copy it to owner-controlled storage:

```sh
railway ssh -- bun run backup
railway volume files download /backups/<timestamped-file>.sqlite ./tohseno-production-<timestamp>.sqlite --json
chmod 0600 ./tohseno-production-<timestamp>.sqlite
```

The remote path for `railway volume files` is relative to the volume root, so `/data/backups/file.sqlite` is downloaded as `/backups/file.sqlite`. The backup remaining under `/data/backups` is not an off-platform backup. Record a local SHA-256 digest after download and keep the owner-held copy under access control.

A useful recovery set includes:

- a transactionally consistent SQLite database containing all committed tables;
- the exact migration/version information;
- secure, separate access to the data key version required for its encrypted values;
- the deployment configuration names and image/commit identifier, without secret values.

Do not place the data key in the same unaudited archive as the database. Conversely, losing the key makes encrypted source/contact/messages unrecoverable.

To restore without writing over a live database:

1. Create an isolated, single-replica service and a clean volume mounted at `/data`. Keep `PAYMENTS_MODE=disabled` and `EMAIL_MODE=disabled`, and do not attach public customer traffic.
2. Supply the exact `TOHSENO_DATA_KEY` held for this backup. A newly generated key cannot decrypt it.
3. Initially set `DATABASE_PATH=/data/bootstrap.sqlite` and deploy. This gives Railway an active deployment for volume file operations without opening the intended restore pathname.
4. Upload the owner-held backup to the unused target path:

   ```sh
   railway volume files upload ./tohseno-production-<timestamp>.sqlite /tohseno.sqlite --json
   railway ssh -- chmod 0600 /data/tohseno.sqlite
   ```

5. In the isolated service, open `/data/tohseno.sqlite` read-only with `bun:sqlite`; require `PRAGMA integrity_check` to return exactly `ok` and inspect only the safe `schema_migrations` versions. Do not print application rows.
6. Change only `DATABASE_PATH` to `/data/tohseno.sqlite` and redeploy. Startup reapplies no recorded migration destructively; it applies only genuinely pending migrations and re-enables WAL.
7. Verify `/healthz`, the migration versions, synthetic capability resolution, and explicit synthetic source inspection. Confirm that payments and email remain disabled during the drill.
8. Remove the isolated service/volume only after recording the drill result and only with the owner's normal infrastructure approval.

For an in-place restore on another host, stop every writer first, retain the failed database for investigation, install the verified standalone backup at `DATABASE_PATH` with mode `0600`, remove stale WAL/SHM files belonging to the replaced database, and start exactly one process. Never copy a backup over a running SQLite database.

Periodically perform this isolated restore. A backup that has never been restored is not a demonstrated recovery path.

## TLS, proxy, and DNS

Production requires HTTPS. `BASE_URL` must match the public origin used in Checkout and transactional links. HTTPS responses include a one-year HSTS policy. When the preferred canonical origin is the apex, token-free `GET` and `HEAD` requests on the corresponding `www` host receive a permanent `308` redirect; mutation requests are rejected instead of replayed across hosts. URL fragments remain client-side across that navigation, so a bearer is never placed in the redirect request or `Location` value.

Trust forwarded client-address headers only when requests can arrive exclusively through a known proxy that overwrites them. With `TRUST_PROXY=true`, TOHSENO uses Railway's canonical, syntactically valid `X-Real-IP` value for rate limits and deliberately ignores `X-Forwarded-For`. A false trust configuration or a direct path around Railway makes rate limiting spoofable.

The landing page must disclose the actual commerce boundary before intake: disabled or incomplete configuration says Checkout is unavailable and no payment will be taken; Stripe test or mock behavior is explicitly labeled test mode; only a fully configured non-test provider receives the normal separate-Checkout wording.

DNS creation or change is an external, potentially irreversible ownership action. Prepare the record and verification plan, then wait for explicit owner approval.

## Post-deploy smoke checks

- `/healthz` is healthy without exposing counts or private configuration.
- `/` and `/privacy` have security headers and no third-party assets/trackers.
- public `HEAD` requests succeed without bodies, known wrong methods return `405` with `Allow`, HSTS is present, and `www` permanently redirects token-free reads to the canonical apex.
- oversized and invalid intake is rejected without echoing content.
- a synthetic valid submission produces a private status link whose raw capability appears only after `#capability=`, never in an HTTP path or query.
- browser fragment exchange sets an expiry-bounded, host-only `Secure`, `HttpOnly`, `SameSite=Strict` cookie dedicated to the named submission; coding-agent access succeeds with `Authorization: Bearer` on the matching safe-ID-scoped route.
- an invalid capability returns `404` and private pages are `no-store` with a `no-referrer` policy.
- disabled/missing payment settings are disclosed on the landing page before intake and repeated on status, or a clearly internal Stripe test Checkout creates the expected line items.
- Stripe rejects an invalid signature and accepts a verified test event once.
- operator endpoints reject missing/wrong tokens and accept the configured token.
- database files exist on `/data`, not the container filesystem.
- a container restart preserves the synthetic order.
- SQLite/database and application logs do not contain the synthetic plaintext document, email, or raw capability.
- Railway HTTP log path fields for the synthetic journey contain only safe-ID-scoped, bearer-free routes; no path has a capability-shaped segment. Do not paste the raw capability into a log-search query while verifying this.

Request and verify cleanup of the synthetic submission through the current operator/support procedure. This first slice intentionally has capability revocation but no automated destructive submission-deletion command; production retention and verified deletion remain an operator responsibility that must be completed before accepting real data.

## Rollback

If a release fails before a migration, restore the previous image. If a migration has run, follow that migration's compatibility plan; do not replace the database blindly. Keep payments disabled if webhook behavior or price configuration is uncertain. Preserve verified incoming webhook events/idempotency rather than replaying success redirects.

For a suspected data-key problem, stop writes and investigate. Do not “fix” it by generating a new key; that would strand existing ciphertext. Follow [Key rotation](KEY_ROTATION.md).
