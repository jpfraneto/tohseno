# TOHSENO website, Registry, and encrypted relays

The Bun site owns the public shipping page, public Registry/catalog and
content-addressed source transport, the constrained generation-0.8 transaction
relayer, the separately activated Claims index/relayer, and the retained
encrypted browser-intention relay. The private Companion relay remains a
separate service.

The landing page leads with “Ship iPhone apps. Person to person.” and the real
builder path:

```text
npm install --global tohseno
cd YourApp
tohseno init
tohseno deploy --app-slug your-app
```

The npm install owns only the CLI launcher and has no postinstall side effects.
Interactive `init` explains one fact at a time and waits for Enter before the
real Xcode adoption begins.

It describes the exact boundary: another person's Mac downloads verified
source, builds with Xcode, signs locally, and installs on their iPhone. It does
not claim Tohseno bypasses Apple signing, provisioning, Trust, Developer Mode,
or operating-system security.

## Public Registry

`apps/site/src/registry.ts` implements a versioned, closed HTTP boundary for:

- public Shot, Builder, release, signed-manifest, receipt, and immutable blob
  reads;
- private expiring source/icon staging with exact size and digest checks;
- Companion-signed catalog publication and current-chain verification;
- constrained factory/RegisterShot/AppendCheckpoint transaction submission;
- signed monotonic Builder profile updates; and
- signed global-alias requests that remain pending until an authenticated,
  append-only policy approval.

The catalog database is only an index. A release becomes visible only when the
Builder DeviceKey signature, authorized live BuilderAccount key, transaction
receipt and canonical block, Registry state extending that checkpoint, public
checkpoint, and staged source digest agree. Public reads recheck canonicality
on a short bounded cache so reorged receipts disappear; security-sensitive Mac
clients repeat the live chain and receipt checks instead of trusting the index.

The profile schema reserves optional external attestations, but production
rejects non-empty attestations until an official provider verifier is
configured. It never turns a self-asserted X URL into a “verified” identity.

Registry public discovery is a canonical software-event timeline rather than
an app grid. The closed event set is Ship, Update, Fork, and Claim Edition
closure. First registration produces exactly one Ship; later catalog releases
produce Updates. Canonical block/transaction/log evidence determines ordering,
including a timed edition's first block at or after its deadline. Individual
Claims are available through bounded Shot/receipt reads but never flood
Discover.

Registry state requires an explicit absolute durable `REGISTRY_ROOT` and HTTPS
`ROBINHOOD_RPC_URL`. `REGISTRY_RELAYER_ENABLED` additionally requires one
dedicated lowercase private key. Per-source/global rate limits, staging record
and byte capacity, thirty-minute expiry cleanup, bounded request bodies, atomic
writes, and content-addressed immutable promotion are fail-closed.

The deploy-time `--app-slug` is signed into every release and stays stable for
later Updates. It creates the Builder-local route immediately; a root route
such as `https://tohseno.com/your-app` still requires the Builder to select the
exact app and sign a global-alias request in Companion. The server keeps that
request pending. An operator can enable the one bounded approval action by
setting `REGISTRY_ALIAS_REVIEW_TOKEN_SHA256` to the lowercase SHA-256 of a
separate high-entropy bearer token, then approve a reviewed request ID:

```sh
curl --fail-with-body \
  -H "Authorization: Bearer $alias_review_token" \
  "https://tohseno.com/api/registry/v1/alias-reviews/0xREQUEST_ID"

curl --fail-with-body \
  -H "Authorization: Bearer $alias_review_token" \
  -H 'Content-Type: application/json' \
  --data '{"decision":"approve"}' \
  "https://tohseno.com/api/registry/v1/alias-reviews/0xREQUEST_ID"
```

The authenticated GET returns only a bounded operator projection of the signed
request, current app/release, signer key, digest, and approval state. Both GET
and POST recheck the immutable request signature, signed time window, current
Builder DeviceKey authority, and current installable Shot; POST additionally
checks alias availability before atomically writing approval evidence and the
route. Approval is idempotent for the same request and returns a conflict
instead of overwriting a different owner. The token itself must not enter
source, logs, or the durable Registry root.

After approval, `tohseno status` independently reads the public alias pointer
and current Shot projection, checks that they bind the local app's exact Shot,
release digest, and canonical route, and confirms the human root page responds
before labeling the link live. A mismatch or later Update is a **do not send**
conflict until the local project reconciles, not a route the CLI follows.

## Claims service

`apps/site/src/claims.ts` verifies the closed threshold-signed Claims
activation under the already pinned release-authority policy, rechecks live
runtime and immutable active Registry, rebuilds a canonical reorg-aware index
from the exact deployment block, and exposes edition, receipt, pagination,
normalized-mark SVG, and token-metadata reads. The database is an index; chain
events and receipts remain authority.

The write state machine persists exact deterministic BuilderAccount bootstrap,
edition-open, and Claim jobs before submitting only their allowlisted calls.
First Ship finalization is ordered Registry → Claim Edition → immutable
catalog/source promotion. Updates reject edition input. Duplicate JSON members,
opaque signed digests, stale heads, noncanonical receipts, per-source abuse,
global overload, and capacity exhaustion fail closed.

Claims has three configuration stages:

1. With no `CLAIMS_*` coordinates, every Claims surface is inactive.
2. Complete address, activation digest/evidence, authority policy, deployment
   block, and `CLAIMS_INDEXER_ENABLED=true` permit verified reads only.
3. `CLAIMS_RELAYER_ENABLED=true` additionally requires that indexer and one
   dedicated lowercase relayer key. It is allowed only during the governed
   physical acceptance in the Claims activation runbook.

Partial coordinates abort startup. An environment address is never activation,
and a relayer never signs for a Builder or claimant. The source intentionally
ships with no production Claims activation evidence or enabled write path.

## Browser intention compatibility

The retained browser handoff creates a noncanonical
`tohseno.intent-package/1`, encrypts it with AES-256-GCM, and uploads bounded
ciphertext chunks. The server never parses the package. It is a support path,
not the website's normal consumer door or a public Shot.

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

`apps/companion-relay` is a separate TOHSENO 1.0.0 service for the private
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
