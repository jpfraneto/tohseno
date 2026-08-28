# Managed compute and creation-balance runbook

Managed compute is an optional, configuration-gated service. It is never a
gate on local, subscription-backed, custom-executable, or configured loopback
inference. Stripe collects prepaid pack payments; the append-only TOHSENO
micro-USD ledger is balance authority; Bankr is the inference supplier. Do not
treat one system's balance or event as authority for another.

## Secrets and durable state

Store these only in the production secret manager:

- `BANKR_API_KEY`: a least-privilege LLM Gateway key, never a wallet or token
  launch key;
- `STRIPE_SECRET_KEY` and `STRIPE_WEBHOOK_SECRET`;
- the random operator token whose lowercase SHA-256 is configured as
  `TOHSENO_OPERATOR_TOKEN_SHA256`.

Configure absolute durable `MANAGED_COMPUTE_ROOT`, the three Stripe Price IDs,
trusted checkout return URLs, an explicit Bankr model allowlist, and the
per-installation service rate. Back up the root with filesystem permissions
and encryption appropriate for payment records. Restore drills must preserve
every immutable entry and capability/use marker; never restore a mutable
balance because none exists.

Bankr's base is `https://llm.bankr.bot`. The adapter uses `/v1/models`,
`/v1/chat/completions`, `/v1/credits`, and `/v1/usage`. The server, not the Mac,
holds the key, allowlists models, adds the documented retail margin, admits
privacy tiers, bounds bodies/tokens/rates, and records usage. Source and prompts
exist in proxy memory only for the admitted request and are not written to the
TOHSENO ledger or access logs. Upstream retention remains governed by the
selected tier and actual provider policy; “zdr” or “private” must be advertised
and sent explicitly and is not a promise about infrastructure outside that
policy.

## Stripe staging and activation

Create one-time USD Prices whose authoritative totals are exactly $10, $25,
and $50. Configure the webhook
`/api/managed/v1/stripe/webhook` for Checkout success/failure, refund, and
dispute events used by the implementation. In Stripe test mode, use Stripe CLI
forwarding to the staging HTTPS service and exercise completed, asynchronous,
duplicate, reordered, refunded, disputed, and won-dispute events. Redirects do
not credit value: raw-body signature verification plus a retrieved paid
Checkout Session and configured Price do.

Production startup needs the values documented in `.env.example`,
`NODE_ENV=production`, and an HTTPS `BASE_URL`. Keep
`MANAGED_COMPUTE_ENABLED=false` until staging proves checkout, provider use,
reconciliation, backup/restore, rate limiting, and secret scanning. Never reuse
the legacy recurring-subscription Prices for creation-balance packs.
Managed balance projections are authenticated by the installation-signed
request and the pinned HTTPS service origin; they do not use the legacy Pro
receipt key. If retained recurring billing is also enabled, provision
`BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL` in the secret manager and ship its
matching compressed P-256 public key as
`billing/verification-key-p256.txt`, covered by the release manifest. Follow
`BILLING_1_0_0.md` for rotation and never reuse that key for managed claims.

## Welcome grants and revocations

The native app displays its opaque installation binding. After verifying the
human and exact binding out of band, use the protected routes with a unique
idempotency key and a reason. Do not paste the raw operator token into tickets
or shell history; the examples show structure only.

```text
POST /api/managed/v1/operator/grants
{installation_binding, amount_microusd, reason, idempotency_key, operator}

POST /api/managed/v1/operator/revocations
{installation_binding, amount_microusd, reason, idempotency_key, operator}
```

Authenticate with `X-Tohseno-Operator-Token`. Grant/revocation operator and
reason fields remain private in the raw ledger and are stripped from the Mac
balance projection. Revocation is a negative promotional compensating entry
and cannot consume paid or already-reserved value. There is no automatic
welcome credit; release copy may offer only a configured contact action.

## Daily health and reconciliation

`GET /api/managed/v1/operator/health` requires operator authentication and
returns live Bankr credits/usage, the count of pending reconciliation records,
and launch-fee funding status. Alert before Bankr credits reach the service
reserve, on sustained `401`, `402`, `429`, or `5xx`, on pending reconciliation,
on storage/capacity errors, and on Stripe webhook failures. Access logs must
remain semantic and content-free.

Authentication, explicit provider-balance, and rate-limit failures release the
reservation. A timeout, gateway failure, malformed usage, or over-cap provider
report is ambiguous and deliberately leaves value held. Compare the private
capability, provider request/usage view, and Bankr `/v1/usage`; then use exactly
one protected decision:

```text
POST /api/managed/v1/operator/reconciliations
{installation_binding, reservation_id, action:"release",
 retail_charge_microusd:0, reason, idempotency_key, operator}

POST /api/managed/v1/operator/reconciliations
{installation_binding, reservation_id, action:"charge",
 retail_charge_microusd, provider_request_id, reason, idempotency_key, operator}
```

The charge may not exceed the outstanding reservation. Both choices release
the hold, append the exact compensating charge if selected, and append a
settled audit entry. Never edit or delete ledger JSON to make totals agree.

## Launch-fee and release truth

`BANKR_LAUNCH_FEE_FUNDING_CONFIRMED=true` is permitted only with a real private
account-evidence reference and operational confirmation. It is a health/report
field, not a token-launch authorization. If evidence is absent, leave it false
and say unverified. The historical `$TOHSENO` token-launch checklist is not the
managed inference runbook and does not authorize a deploy.
