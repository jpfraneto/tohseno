# TOHSENO website and encrypted intention relay

The Bun service in `apps/site` owns the public static pages and, only when
explicitly activated, the temporary ciphertext relay. The browser creates the
noncanonical `tohseno.intent-package/1`, encrypts it with AES-256-GCM, and
uploads bounded ciphertext chunks. The server never parses the package.

## Local development

The relay is fail-closed by default. A complete local handoff may be exercised
without a published release:

```sh
relay_root="$(mktemp -d /tmp/tohseno-relay.XXXXXX)"
cd website
NODE_ENV=development \
BASE_URL=http://127.0.0.1:3000 \
INTENT_RELAY_ENABLED=true \
CLAIM_INSTALLER_READY=true \
INTENT_RELAY_ROOT="$relay_root" \
bun run dev
```

Use a source-built CLI with a temporary data root and the explicit debug-only
loopback override:

```sh
TOHSENO_DATA_ROOT=/tmp/tohseno-intent-state \
TOHSENO_INTENT_RELAY_ORIGIN=http://127.0.0.1:3000 \
cargo run -p tohseno -- intent claim --stdin --no-open
```

The site relay's package, routing, origin, expiry, and claim behavior are
covered by the checked-in Bun suites (`bun test apps/site/tests`).

## Production configuration

The production relay was activated on 2026-08-03 after immutable `v0.8.3`
publication and public-installer pin verification. The deployment evidence and
repeatable ordering are in `release/WEB_INTENTION_HANDOFF_ACTIVATION.md`.

Production activation requires all of the following:

- `NODE_ENV=production` and an HTTPS `BASE_URL` matching the canonical origin;
- `INTENT_RELAY_ENABLED=true`;
- `CLAIM_INSTALLER_READY=true`, set only after the public installer has been
  pinned to the published claim-capable release;
- an explicit absolute `INTENT_RELAY_ROOT` on an owner-controlled durable
  volume; the root must be writable, private, and not a symlink;
- capacity and rate limits reviewed through `INTENT_RELAY_MAX_RECORDS`,
  `INTENT_RELAY_MAX_BYTES`, `INTENT_RELAY_GLOBAL_RATE`, and
  `INTENT_RELAY_SOURCE_RATE`.

Invalid enabled configuration aborts startup. Do not use an ephemeral deploy
filesystem. Run bounded cleanup independently with the same environment:

```sh
bun run relay:cleanup
```

All relay APIs are no-store, same-origin browser mutations use exact Origin
and Fetch Metadata checks, and CLI claims use bearer capabilities at the fixed
official origin. Logs are content-free operational events; authorization,
relay capabilities, keys, prompts, filenames, content digests, and full relay
IDs must never be added to them.

## Private companion relay

`apps/companion-relay` is a separate TOHSENO 0.9.0 service for the private
iPhone companion channel. It does not share routes or state with the temporary
browser-intention relay. The companion relay stores only bounded opaque pairing
responses and signed encrypted envelopes. It supplies short-lived pairing
rendezvous, capability-protected device mailboxes, cursor catch-up,
acknowledgements, server-sent live delivery, revocation, and optional
content-free APNs wake-ups. It never interprets a command or invokes local
factory work.

Start an isolated development relay without push delivery:

```sh
companion_root="$(mktemp -d /tmp/tohseno-companion-relay.XXXXXX)"
cd website
NODE_ENV=development \
BASE_URL=http://127.0.0.1:3100 \
HOST=127.0.0.1 \
PORT=3100 \
COMPANION_RELAY_ENABLED=true \
COMPANION_RELAY_ROOT="$companion_root" \
bun run companion-relay:dev
```

Run its tests and independent bounded cleanup with:

```sh
bun run companion-relay:test
bun run companion-relay:cleanup
```

From `website/`, one command now starts a real isolated Companion Relay and
Local Workspace Service, waits for both verified health responses, pairs the
real CLI companion simulator, decrypts and validates the initial workspace
snapshot, verifies paired-device state, and cleans up its exact temporary
processes, files, and Keychain entries:

```sh
bun run companion:local
```

The corresponding companion end-to-end smoke is
`bun run companion:local:test`. The separate isolated LaunchAgent lifecycle is
`../scripts/test-macos-service-lifecycle.sh`. These commands require the
repository's development toolchain (`cargo` and `bun`); the installed end-user
release does not require either runtime.

Production activation additionally requires HTTPS, an absolute durable relay
root, and `COMPANION_RELAY_ACTIVATION_READY=true`. Leave
`COMPANION_RELAY_PUSH_MODE=noop` when APNs is intentionally disabled. To enable
APNs, set the mode to `apns` and supply a valid team ID, key ID, bundle topic,
environment, and absolute path to a bounded regular `.p8` key file. Startup
fails closed for missing, malformed, non-P-256, symlinked, non-owner-only, or
non-key credentials. The
`fake` provider is test-only and is rejected in production.

Mailbox retention defaults to seven days. Pairing sessions default to two
minutes. Envelope time validation tolerates five minutes of clock skew by
default and can be configured only up to fifteen minutes. Capacity, body,
catch-up, retention, and source/global rate limits are all explicit
`COMPANION_RELAY_*` settings. `COMPANION_RELAY_MAX_ENVELOPE_BYTES` bounds the
decoded ciphertext, including its AEAD tag, to the shared Rust/Swift limit; the
outer JSON request bound is derived from it with fixed encoding overhead. Run
cleanup independently in production.

The versioned wire surface is:

```text
POST   /v1/companion/pairing-sessions
POST   /v1/companion/pairing-sessions/:id/respond
GET    /v1/companion/pairing-sessions/:id
DELETE /v1/companion/pairing-sessions/:id
POST   /v1/companion/mailboxes
POST   /v1/companion/mailboxes/:id/envelopes
GET    /v1/companion/mailboxes/:id/envelopes?cursor=...
GET    /v1/companion/mailboxes/:id/live?cursor=...
POST   /v1/companion/mailboxes/:id/ack
DELETE /v1/companion/mailboxes/:id
POST   /v1/companion/push/register
DELETE /v1/companion/push/register/:device_id
```

Pairing responses are raw bounded `application/octet-stream`; outer envelopes
are direct `tohseno.companion-envelope/1` JSON objects. A client creates an
independent random 32-byte capability for each protected operation. Only its
lowercase SHA-256 verifier is sent in a create body, while the base64url
capability itself is sent later as `Authorization: Bearer ...`. The digest is
over the decoded 32 bytes, not the base64url text. Session and mailbox IDs are
unguessable relay-generated values. Cursor pages wrap each opaque envelope with
its delivery cursor; delivery acknowledgements contain only the cursor.

The companion service logs only semantic route names, status, timing, and
aggregate push outcomes. It never logs paths containing IDs, capabilities,
public keys, nonces, ciphertext, content digests, device tokens, or source
addresses. `/healthz` and `/metrics` expose only content-free service and
capacity facts.
