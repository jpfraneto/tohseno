# TOHSENO website and encrypted intention relay

The Bun service in `apps/site` owns the public static pages and, only when
explicitly activated, the temporary ciphertext relay. The browser creates the
noncanonical `tohseno.intent-package/1`, encrypts it with AES-256-GCM, and
uploads bounded ciphertext chunks. The server never parses the package.

## The landing page is a terminal

`/` is one prompt and nothing above it. Its placeholder is the command a
person will actually run on their Mac, and the page speaks the same words the
Mac and the phone speak: App, Create, Evolve, and the six human states of
ADR 0016.

```text
tohseno create <name>   write the intention, attach up to eight images
tohseno demo            replay a build; nothing is installed
tohseno install         the one-line installer
tohseno book            the paid day
help · clear · source · community · whitepaper · token
```

Anything else that reads like prose is the intention itself. The page asks a
person to describe an app, so answering that at the prompt opens the composer
with their words already in it. Only a lone unrecognized word is treated as a
mistyped command. Typing the `tohseno` prefix is always a command attempt and
is never reinterpreted.

`docs` and `privacy` are deliberately absent from the status bar and from the
help list, and resolve through `FALLBACK_LINKS`; both pages stay published and
the HTTP suite asserts they still serve.

RUN pressed on an empty prompt types `tohseno create my-app` and runs it. The
button is never a dead control, and a first-time visitor reaches the composer
without knowing a single command.

## What can be dropped, from anywhere

Dropping or pasting anywhere on the page is the whole gesture, in any mode. A
`.md` file *is* the intention: its text lands in the composer, editable, and
its filename names the app. Images stay reference material carried alongside
the words. Neither one requires running a command first — if nothing is open,
the drop opens the composer itself.

On a phone the software keyboard opens from a tap anywhere on the page, since
iOS raises it only for a focus inside a real gesture. `app.js` measures how
much of the window the keyboard covers through `visualViewport` and hands that
much back through the `--keyboard` custom property, so the prompt is never
underneath the keys it is receiving.

A person writes the whole intention before anything is asked of them. Only at
send does the page offer three doors:

- **send it to my mac** — the ADR 0011 handoff. The browser builds and encrypts
  the package, uploads ciphertext chunks, and prints the single-use
  `--claim` command. If the relay is not activated it downloads the private
  `.tohseno-intent` file instead and says plainly that the file is not
  encrypted.
- **link the tohseno app** — deliberately not built. The iPhone Companion is
  not published, so the door explains what linking will be and hands back. When
  it exists it will link *the browser to the phone*, the way WhatsApp Web links
  a browser to a phone: the phone holds the identity and does the signing, the
  browser holds a capability the phone can revoke, and there is still no
  account. Do not turn this into one.
- **see a demo** — replays `application/src/presentation.rs` with that file's
  exact headlines. `apps/site/tests/terminal.test.ts` asserts every replayed
  state against `fixtures/presentation-v1.json`, so the website joins the Mac
  and the phone in the one presentation contract and cannot invent a state.

`public/modules/terminal.js` holds every decision that is not the DOM — command
resolution, the app-name rule mirrored from `engine/src/ledger.rs`, the doors,
the demo table, and reference filenames — so the whole surface is covered by
`bun test` without a browser. `public/app.js` is only wiring. It builds every
line as a node with `textContent`; nothing typed or dropped can become markup.
The stylesheet revision in the markup is derived from the stylesheet itself in
`server.ts`, so it is never hand-maintained.

Nothing sits above the prompt. Every sentence that used to introduce the page
stood between a person and the one thing to do, so the whole offer — including
the paid day — now lives in the `<noscript>` block, which is exactly where it
was ever read from: by a crawler, or by a reader without JavaScript.

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
