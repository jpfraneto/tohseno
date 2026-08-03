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

The checked-in end-to-end test performs this flow automatically, including
Studio inspection, without launching a harness:

```sh
cd website
bun test apps/site/tests/local-handoff.e2e.test.ts
```

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
