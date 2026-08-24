# Activate TOHSENO 1.0.0 billing

Billing is inactive in repository source. Activation is a manual owner action
and is independent from Apple Developer Program membership, npm publication,
relay/APNs activation, DNS, contracts, and the public installer.

## Prepare outside the repository

1. In Stripe, create recurring prices for exactly USD $9.99 monthly and USD
   $99 yearly. Enable the hosted customer portal.
2. Generate a P-256 receipt-signing key on an offline administrative machine.
   Store the PKCS#8 private key in the website secret manager. Never commit it,
   paste it into chat, or put it in an npm/native artifact.
3. Export only its compressed SEC1 public key as unpadded base64url and place
   that single line at `billing/verification-key-p256.txt` in the clean release
   commit. Independently compare it with the public key derived from the
   secret-manager key.
4. Configure the Stripe webhook endpoint as
   `https://tohseno.com/api/billing/v1/webhook` for
   `customer.subscription.created`, `.updated`, and `.deleted`.

The website deployment requires these secret-manager/environment values:

```text
BILLING_ENABLED=true
BILLING_PROVIDER=stripe
BILLING_ROOT=/absolute/private/durable/path
BILLING_MONTHLY_PRICE_ID=price_...
BILLING_YEARLY_PRICE_ID=price_...
BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL=...
STRIPE_SECRET_KEY=sk_live_...
STRIPE_WEBHOOK_SECRET=whsec_...
```

`NODE_ENV=production` and `BASE_URL=https://tohseno.com` are also required.
Startup fails closed for missing values, a fake provider, a non-live Stripe key,
or a relative storage root. Logs contain only semantic route/status records;
do not add checkout claims, tokens, customer IDs, payment data, or receipt
bodies.

## Verify before activation

```sh
cd website
bun run typecheck
bun test apps/site/tests/billing.test.ts
cd ..
cargo test --locked -p tohseno-application billing::tests
./scripts/test-1.0.0-golden-path.sh
```

Build the clean native release and verify that
`share/billing/verification-key-p256.txt` is covered by `CHECKSUMS.sha256`.
Exercise checkout, a signed test webhook, refresh, renewal, cancellation at
period end, lapse, restoration, and customer portal in Stripe test mode against
a staging origin. Independently verify a staging receipt with the native
verifier and the expected installation binding.

Only then deploy the website with live credentials and publish a native release
containing the matched public key. If either side is unavailable, disable
`BILLING_ENABLED`; the local product remains locked/preserved and never trusts
an unverifiable receipt. Key rotation requires a new native release and an
explicit overlap/migration design; do not replace a key in an immutable release.
