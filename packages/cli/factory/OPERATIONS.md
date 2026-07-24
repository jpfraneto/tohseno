# Shot operations

These operations are for coding agents and automation. The global CLI is a
convenient dispatcher; the pinned shot-local form remains available after
ejection:

```sh
tohseno machine operations --json
bun .tohseno/machine.ts operations --json
```

In JSON mode stdout is exactly one protocol document. Diagnostics go to
stderr. Exit codes are stable: `0` success, `2` invalid shot/configuration, `3`
missing dependency, `4` unhealthy service/readiness failure, and `5` internal
failure. Never parse human prose when JSON is available.

## Bring development alive

From anywhere inside this shot:

```sh
tohseno machine dev start --json
tohseno machine dev status --json
tohseno machine ios launch --json
```

`dev start` is idempotent. A detached shot-owned supervisor starts the API,
runs SQLite migrations, waits for `/health`, writes the Debug endpoint
atomically, and keeps monitoring its children. The API binds to `127.0.0.1`
and asks the OS for an available port unless `--port` is explicit. Persistent
data lives under `.tohseno/data/`; state and logs are separate under
`.tohseno/run/`.

Use a Quick Tunnel only when a physical device or explicit remote test cannot
reach localhost:

```sh
tohseno machine dev start --tunnel --json
```

The supervisor runs `cloudflared tunnel --url http://127.0.0.1:<port>`, captures
the random `trycloudflare.com` URL, and injects it only into the gitignored
Debug endpoint file. If `cloudflared` is unavailable, the operation fails with
exit `3`; never fall back to an unapproved or insecure transport.

Quick Tunnels are public, development/testing-only endpoints: no uptime SLA,
a random hostname, limited concurrent requests, and no server-sent events.
They provide reachability, not authentication. Never use one in Release,
production, a store archive, or DNS.

Inspect and clean up through owned operations:

```sh
tohseno machine dev logs --service all --lines 100 --json
tohseno machine dev stop --json
```

`stop` signals only PIDs whose command identity matches this shot and runtime
instance. It never kills by port. It removes process state and the generated
endpoint while preserving SQLite data and logs. If a child died or the machine
rebooted, `status` reports stale/unhealthy ownership and the next `start`
recovers it intelligibly.

## iOS endpoint boundary

Debug includes only `Config/DevelopmentEndpoint.xcconfig`. Localhost is valid
for the simulator; an HTTPS Quick Tunnel is valid for an owner-controlled
physical Debug build. No Swift source editing is needed when the URL changes.
The Settings screen shows the endpoint host and health, and says unavailable
without risking local writing data when the API cannot be reached.

Release includes only tracked `Config/Production.xcconfig`. Configure a public
bare origin using xcconfig URL escaping, for example:

```xcconfig
PRODUCTION_API_BASE_URL = https:/$()/api.example.com
```

The Release build gate and shot verification reject HTTP, localhost, loopback,
paths/credentials, and every `*.trycloudflare.com` URL. A missing production
origin remains an explicit production blocker.

`ios launch` requires Xcode and an available iPhone simulator. It boots an
actual UDID, builds Debug into gitignored DerivedData, installs, launches, and
checks that the built app endpoint matches the active runtime. If Xcode or a
simulator is absent, the shot and API remain intact; inspect the limitation with:

```sh
tohseno machine ios inspect --json
```

For a physical device, start with `--tunnel`, choose the signing team in Xcode,
and run the Debug app on that device. TOHSENO does not automate signing-account
or device trust decisions.

## Token operations

A token launch is an external, irreversible financial action on the same side
of the approval boundary as deployment and store submission: the agent
prepares, the human approves, the machine executes deterministically through
the owner's own Bankr CLI. TOHSENO ships no server, holds no keys, and takes
no fees.

```sh
tohseno machine token status --json
tohseno machine token launch --name <name> --symbol <sym> --chain base|robinhood --json
tohseno machine token fees --json
```

`token status` is read-only: whether the Bankr CLI resolves, whether Bankr
credentials exist (existence only — key material is never read or printed),
and the token record from the manifest if one has been launched.

`token launch` requires `--name`, `--symbol` (1-10 chars), and `--chain`
(`base` or `robinhood`; no silent default). Optional: `--image <https-url>`,
`--website <https-url>`, `--fee-recipient <handle|ens|address>` with
`--fee-type x|farcaster|ens|wallet` (default fee recipient is the owner's
Bankr wallet). In `--json` mode the launch refuses without `--yes` (exit `2`)
and the refusal carries the full economics summary plus the exact rerun
command — `--yes` is itself the approval, typed by the human or by an agent
the human has explicitly instructed. The launch is permanent. Provider
economics, vesting, fees, rate limits, and beneficiary rules can change;
TOHSENO does not independently quote or guarantee them. Review Bankr's
current terms before approving.

One token per shot: a second launch exits `2`. A missing Bankr CLI or missing
credentials exits `3` with explicit install/login guidance (`npm install -g
@bankr/cli`, then `bankr login` — human rituals, never performed by an agent).
TOHSENO never uses `npx` as financial authority and requires Bankr's
non-broadcasting `--simulate` path to pass before launch. A Bankr-side failure
exits `5` without persisting or echoing provider output. On success the token record
(provider, chain, name, symbol, address, txHash, launchedAt) is written to
`continuity.manifest.json` — a fact about the shot, committed like any other
manifest change. The gitignored token log contains event type, chain,
simulation state, and exit status only.

Credentials live only in the owner's `~/.bankr/config.json` (written by
`bankr login`) or `BANKR_API_KEY`. They are never copied into the shot repo,
the manifest, logs, or factory releases. Fee management after launch
(`bankr fees claim` and friends) is the owner's business, outside TOHSENO.

## Verification and production

After every app, backend, configuration, manifest, or runtime change:

```sh
tohseno machine verify --json
# ejectable human equivalent
bun run verify
```

For “put this online,” “ship this,” or “send this to TestFlight,” inspect before
asking for authority:

```sh
tohseno machine production inspect --json
```

Inspection is read-only. It reports the production API origin, stable-HTTPS
validation, single-instance SQLite configuration, backups, unresolved secret
references, readiness blockers, and capability status. This release implements
inspection, local runtime, and simulator launch. Production deploy, monitoring,
recovery, VPS provisioning, DNS changes, store submission, and `fastlane beta`
are not implemented operations. Explain that boundary; prepare deterministic
commands where possible; request explicit approval before any account, cost,
credential, publishing, or external mutation.

The SQLite contract is honest: development and the initial production shape
share single-instance semantics. Do not claim Postgres, horizontal scaling, or
automatic backups. The baseline API stores operational metadata only and never
writing content. Any later network/content behavior must first be expressible
in and declared by `continuity.manifest.json`.
