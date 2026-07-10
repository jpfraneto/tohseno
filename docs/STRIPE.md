# Stripe setup

Stripe is an optional payment adapter around the order state machine. It never receives the private source document and it does not decide whether a continuity app has been generated.

## Payment modes

- `PAYMENTS_MODE=disabled`: no Checkout is created; status explains that payment is unavailable.
- `PAYMENTS_MODE=mock`: local/test success path using no network. Startup fails if `NODE_ENV=production`.
- `PAYMENTS_MODE=stripe`: Stripe Checkout plus raw-body, signature-verified webhooks.

Missing Stripe configuration never falls back to a fake success.

## Required Stripe configuration

```text
STRIPE_SECRET_KEY
STRIPE_WEBHOOK_SECRET
STRIPE_SELF_HOSTED_PRICE_ID
STRIPE_CLIENT_SETUP_PRICE_ID
STRIPE_CLIENT_MONTHLY_PRICE_ID
```

Create prices in the same Stripe account and currency expected by product configuration:

- self-hosted: one-time price for $88;
- client setup: one-time price for the founding $888 setup fee;
- client monthly: recurring monthly price for $88/month.

The server uses Stripe Price IDs as the commerce authority. After Stripe creates a session, the adapter inspects expanded Price objects and rejects/expires the session unless the exact IDs, quantities, one-time/recurring types, amounts, currency, per-unit billing, and licensed monthly interval match product configuration. Customer-facing configured prices must match those Stripe objects before enabling payments. Treat a mismatch as a release blocker, not as display-only drift.

Client-owned Checkout uses subscription mode with both the one-time setup line item and recurring monthly line item. Self-hosted uses one-time payment mode. Anky-operated orders never request Checkout.

## Webhook

Configure the HTTPS endpoint:

```text
POST <BASE_URL>/api/webhooks/stripe
```

Copy the endpoint signing secret—not a Dashboard login or API key—into `STRIPE_WEBHOOK_SECRET` in the deployment secret manager.

The server verifies Stripe's signature against the exact raw request body before parsing the event. Do not place JSON middleware, proxies that rewrite the body, or form conversion in front of that handler. Invalid signatures are rejected without revealing secret/config detail.

Webhook events are idempotent at both provider-reference and database transition boundaries. Stripe may deliver the same event more than once; the second delivery must not append duplicate payment transitions or duplicate emails.

Completed, asynchronous-success, asynchronous-failure, and expiry events are reconciled separately. Amount, currency, mode, provider, and both safe submission references must match the reserved attempt. A mismatch enters `requires_review` and cannot release a capsule. A latest failed/expired attempt returns the order to `READY_FOR_PAYMENT`; an older late failure cannot move a newer attempt backward. A paid record is monotonic against later failure delivery.

Stored Checkout URLs are offered for resumption only while the same provider is configured and its verified-webhook boundary is currently available. Disabling payments or removing webhook configuration pauses new and resumed Checkout links instead of asking a customer to pay while reconciliation is offline.

The browser's `/checkout/success` route is informational. It states that the return from Checkout is not payment proof and directs the person back to the private status URL they already hold. It never marks an order paid or receives a capability token from Stripe.

## Metadata policy

Allowed metadata is limited to safe identifiers needed to reconcile Checkout, such as a submission ID and operating-mode code.

Never include:

- submitted Markdown, excerpts, title, or prompt-derived summary;
- contact email as custom metadata;
- capability token or capability URL;
- encryption key, operator token, or provider secret;
- operator/customer message body;
- content hash as an authorization mechanism.

The application may supply the customer email through Stripe's standard Checkout customer field only if the implementation and privacy notice deliberately enable that behavior. The first-slice invariant is that no private email is placed in metadata.

## Local testing

The automated suite uses fake requests/providers and does not require Stripe or network access. Use `PAYMENTS_MODE=mock` to exercise the full self-hosted and client-owned state paths locally.

For Stripe CLI/manual test mode after explicit setup, keep test keys and test Price IDs separate from production. Forward signed webhook test events to the local webhook endpoint and verify:

1. an invalid signature is rejected;
2. a valid completed Checkout advances only the matching order/mode;
3. duplicate delivery has no duplicate effect;
4. self-hosted reaches capsule `READY` only after the verified event;
5. client-owned reaches `NEEDS_CREDENTIALS`;
6. Anky-operated creates no session;
7. Stripe request metadata contains only allowlisted safe IDs.

Do not run provider tests with live keys or real charges as part of `bun run test`.

## Refunds and cancellations

Stripe commerce state and TOHSENO order state are related but not identical. A paid event that arrives after local cancellation is recorded as paid and flagged for human review without releasing delivery. After the provider refund is actually performed and verified manually, the operator can record the explicit `CANCELLED → REFUNDED` resolution. Automatic refund/cancellation webhooks are not implemented. Operators must not assume changing Stripe automatically revokes a capsule, deletes intake, or ejects an application.
