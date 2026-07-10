# Email setup

Version one sends transactional email only. Telegram, Signal, WhatsApp, X, marketing automation, and tracking pixels are deliberately absent.

## Modes

- `EMAIL_MODE=disabled`: no delivery attempt; order behavior remains valid.
- `EMAIL_MODE=console`: local/test provider that logs safe delivery metadata only.
- `EMAIL_MODE=resend`: direct HTTPS requests to Resend, without an SDK.

Required for Resend:

```text
RESEND_API_KEY
EMAIL_FROM
```

`EMAIL_FROM` must be an address/domain authorized in the Resend account. Store the API key in the deployment secret manager. Do not put it in `.env.example`, source, operator messages, or build arguments.

## Transactional messages

When configured, the application sends:

1. Submission received.
2. Payment confirmed.
3. Self-hosted capsule ready.
4. Client-owned credentials required.
5. Anky-operated application received.
6. A customer-facing operator message supplied explicitly with a status change.

The self-hosted ready message states that the private agent capsule—not a completed native app—is ready.

The client-owned payment message uses this expectation:

> Your continuity app is entering production preparation.
>
> Your source contract, infrastructure plan, store materials, and production candidate are expected within eight hours after all required account access and credentials are ready.
>
> Public App Store and Google Play availability follows platform review and cannot be guaranteed within that window.

It must not imply that TOHSENO controls Apple or Google approval.

## Privacy and logging

The recipient address is encrypted in SQLite and decrypted only for an explicit delivery attempt or operator inspection. The source Markdown is never sent to the email provider. Email subjects never contain source text, content hashes, capability tokens, or customer-authored message bodies.

Console mode and server logs may include a request/delivery ID, provider mode, template/event name, submission ID, result, and latency. They should not print the decrypted recipient or rendered body. Resend response errors must be sanitized before logging; do not dump request headers/payloads.

Email is not a secure content transport. Prefer a short transactional notice and the intended private bearer link. A capability in an email body grants whoever can read/forward the email access; do not additionally place it in analytics/tracking redirects. Messages use direct first-party links and no pixels.

## Failure semantics

Email delivery is a side effect, not order truth. A provider timeout or rejection does not roll back a committed submission, verified payment, or legal state transition. The matching notification intent is committed in the same SQLite transaction as intake/payment state, then claimed through a durable outbox. Static template bodies live in source; customer-authored operator messages are encrypted before their outbox row is committed.

Outbox rows move through `pending → sending → sent|failed`, or `pending → suppressed` when email is disabled, with an atomic claim, a stale-claim recovery window, safe provider reference, and a stable idempotency key sent to Resend. Concurrent drains cannot claim the same row. The supported one-replica startup resets any persisted `sending` claim to `failed` before recovery, because no earlier process can still own it. Rows for one submission are delivered sequentially in intent order; different submissions may prepare in parallel, while the Resend adapter paces its shared outbound calls. After server construction, a caught background task drains a bounded batch of up to 100 committed pending/failed rows; repeat the operator retry for a larger backlog. Submission capability responses and Stripe webhook acknowledgements never wait for email delivery. Duplicate verified payment events may also trigger a drain without duplicating the intent. An operator can inspect safe message status and deliberately retry failed or suppressed notices with:

```sh
bun run operator -- retry-email <submission-id>
```

Provider idempotency is an additional safeguard, not the database authority. A `sent` row is not selected again. Failed deliveries are logged without recipient/body content. The returned private status capability remains the customer’s source of truth even when email is delayed.

`EMAIL_MODE=disabled` marks new pending intents `suppressed`; later enabling Resend does not unexpectedly send historical notices. An authenticated, deliberate `retry-email` call may release suppressed rows after the operator confirms they are still appropriate. The disabled mode is visible in the safe startup summary without making the underlying workflow claim that delivery occurred. `EMAIL_MODE=console` is local/test behavior and is rejected in production.

## Setup checklist

1. Verify the sending domain/address with Resend.
2. Configure SPF/DKIM as instructed by the provider only after DNS owner approval.
3. Set `EMAIL_MODE=resend`, `RESEND_API_KEY`, and `EMAIL_FROM` through deployment secrets.
4. Send only synthetic test orders first.
5. Verify the six templates use the correct state and ownership mode.
6. Inspect logs for absence of recipient/body/capability.
7. Test provider failure and confirm committed order state remains correct.

DNS changes and provider account creation can be externally consequential; prepare them, then wait for explicit owner approval.
