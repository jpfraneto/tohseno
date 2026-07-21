# TOHSENO

The fastest path from one prompt to an iOS app on your phone. You run one
command, hand your coding agent one sentence, and get a working app. No
logins — not for you, not for your users: a seed phrase instead of a signup
form.

## Start here

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
```

The script clones these rails at an exact pinned commit and creates your
workspace — which is already a compiling, running iOS app. It accepts no
secrets, sends no telemetry, creates no accounts, and deploys nothing. Read
it first if you prefer: `curl -fsSL https://tohseno.com/oneshot.sh | less`.

Then:

1. **Hand off.** `cd` into the workspace and start your coding agent:
   *"Read AGENTS.md. Build this: \<your app, in one sentence\>."*
2. **One line is enough.** The agent asks at most three questions, picks
   sensible defaults for the rest, and records every default as `ASSUMED` in
   its completion report so you correct after seeing the app, not before.
3. **Run it.** `open Writing.xcodeproj`, ⌘R. The TestFlight command comes
   prepared behind one approval.

## What a continuity app is

An app where cryptography replaces accounts. On first launch it silently
generates a BIP39 seed phrase; a keypair derived from it is the user's
identity, and recovery is the phrase. Data lives on the device as plain
files — a local log instead of a cloud profile. No auth screens, no OAuth,
no email capture. That is the category, and the reason this exists.

## What works now

**Implemented:** the base app (SwiftUI writing app with seed-phrase identity,
atomic crash-safe persistence, session log, settings with reveal/restore,
local share card, packaged landing page, and a green invariant-test suite),
the flag-gated module seams (paywall/notifications off, share card on,
SessionLink reserved), the manifest schema 0.2.0 with validator, the agent
build protocol, `bun run setup`, the pinned oneshot bootstrap, and the public
site with its check gate.

**Prepared, not executed:** the fastlane `beta` TestFlight lane — agents
print it; owners run it.

**Not yet:** Android, SessionLink (QR browser pairing — declared as a
reserved manifest field and flag only), encrypted sync, and any backend for
generated apps (deliberately: there isn't one).

## Repository architecture

```text
apps/site                 one Bun server: hero landing, /docs, /privacy, oneshot.sh
templates/continuity-app  the base app — a compiling iOS writing app every
                          workspace starts from (sources, tests, site/, fastlane,
                          bun run setup)
packages/manifest         the manifest schema, types, validator
packages/contracts        draft event/artifact contract schemas and fixtures
skills/continuity-app     the coding-agent build protocol
docs                      doctrine, architecture, deployment, ADRs
scripts                   the bun run check gate
```

See [Doctrine](docs/DOCTRINE.md) — five pillars, one page — and
[System architecture](docs/SYSTEM_ARCHITECTURE.md).
[Local development](docs/LOCAL_DEVELOPMENT.md) covers running everything;
`bun run check` is the before-commit gate. The intake product that once
lived here is preserved intact on the `archive/intake-product` branch.

## License and names

Apache License 2.0 ([LICENSE](LICENSE)). The license grants no trademark
rights to the TOHSENO or Anky names; see [TRADEMARKS.md](TRADEMARKS.md).
