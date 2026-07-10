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
| `TRUST_PROXY` | Enable only when the exact ingress topology is understood |
| `TOHSENO_DATA_KEY` | Base64-encoded 32 random bytes, stored in the platform secret manager |
| `TOHSENO_OPERATOR_TOKEN` | Independent high-entropy bearer secret |
| `PAYMENTS_MODE` | `stripe` or `disabled`; never `mock` |
| Stripe variables | Required together when payment mode is `stripe`; see [Stripe](STRIPE.md) |
| `EMAIL_MODE` | `resend` or `disabled` for production |
| Resend variables | Required when email mode is `resend`; see [Email](EMAIL.md) |

Startup validates core configuration and fails closed when the data key, operator token, production HTTPS origin, or selected Resend settings are missing. Stripe mode can start with incomplete price/webhook settings so the private status page can explain exactly which safe configuration name is missing; it cannot create Checkout or fake success until all required Stripe values exist. The startup summary contains safe modes, paths, origins, and provider availability only—never secret values.

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

The included `railway.toml` defines the start command, `/healthz` check, and restart behavior. In the Railway project:

1. Create or select the service from the repository.
2. Add a persistent volume and mount it at `/data`.
3. Set `DATABASE_PATH=/data/tohseno.sqlite`.
4. Add all required secrets through Railway variables, not repository files.
5. Set `BASE_URL` to the final HTTPS origin before enabling Stripe Checkout or email links.
6. Keep one replica.
7. Configure the Stripe webhook only after the stable HTTPS endpoint exists.
8. Confirm the volume survives a restart before accepting real submissions.

Railway attaches a newly mounted volume with platform ownership rather than the image-layer `/data` ownership. Keep the included entrypoint intact: it corrects that single directory and drops privileges before starting Bun. The Railway start command replaces the image command but does not replace this entrypoint.

Creating a Railway service or volume may incur cost and requires explicit owner approval. This repository does not run `railway up`.

## Migration and release sequence

1. Back up the SQLite database, WAL state through a SQLite-consistent method, and separately verify custody of the matching data key.
2. Build and test the exact commit: `bun run check`.
3. Apply migrations using `bun run migrate` against a staged copy or during a controlled single-instance release.
4. Start one new process and wait for `/healthz`.
   After server construction, a caught background task atomically drains a bounded batch of pending/failed transactional email intents when email is enabled; health binding does not wait on Resend.
5. Smoke-test landing, privacy, synthetic intake/status, invalid capability behavior, operator authentication, and configured provider readiness.
6. Enable traffic.
7. Monitor structured status/error logs without adding private request bodies.

Migrations are forward changes. A container rollback cannot automatically undo a schema change or decrypt data with a previous key. Every future destructive migration needs its own compatibility and rollback plan.

## Backups and restore drill

A useful backup includes:

- a transactionally consistent SQLite database containing all committed tables;
- the exact migration/version information;
- secure, separate access to the data key version required for its encrypted values;
- the deployment configuration names and image/commit identifier, without secret values.

Do not place the data key in the same unaudited archive as the database. Conversely, losing the key makes encrypted source/contact/messages unrecoverable.

Periodically restore to an isolated environment with email/payments disabled, run migrations in dry/staged form, retrieve only synthetic seeded records, and verify capabilities/crypto. A backup that has never been restored is not a demonstrated recovery path.

## TLS, proxy, and DNS

Production requires HTTPS. `BASE_URL` must match the public origin used in Checkout and transactional links. Trust forwarded client-address headers only when requests can arrive exclusively through a known proxy that overwrites them. A false `TRUST_PROXY` configuration makes rate limiting spoofable.

DNS creation or change is an external, potentially irreversible ownership action. Prepare the record and verification plan, then wait for explicit owner approval.

## Post-deploy smoke checks

- `/healthz` is healthy without exposing counts or private configuration.
- `/` and `/privacy` have security headers and no third-party assets/trackers.
- oversized and invalid intake is rejected without echoing content.
- a synthetic valid submission produces a private status URL.
- an invalid capability returns `404` and private pages are `no-store`.
- disabled/missing payment settings render honestly, or Stripe creates the expected line items.
- Stripe rejects an invalid signature and accepts a verified test event once.
- operator endpoints reject missing/wrong tokens and accept the configured token.
- database files exist on `/data`, not the container filesystem.
- a container restart preserves the synthetic order.
- SQLite/database and logs do not contain the synthetic plaintext document, email, or raw capability.

Request and verify cleanup of the synthetic submission through the current operator/support procedure. This first slice intentionally has capability revocation but no automated destructive submission-deletion command; production retention and verified deletion remain an operator responsibility that must be completed before accepting real data.

## Rollback

If a release fails before a migration, restore the previous image. If a migration has run, follow that migration's compatibility plan; do not replace the database blindly. Keep payments disabled if webhook behavior or price configuration is uncertain. Preserve verified incoming webhook events/idempotency rather than replaying success redirects.

For a suspected data-key problem, stop writes and investigate. Do not “fix” it by generating a new key; that would strand existing ciphertext. Follow [Key rotation](KEY_ROTATION.md).
