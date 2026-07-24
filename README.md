# TOHSENO

## Take another one.

Give TOHSENO one intention. It turns that private input into a sanitized app
plan, composes a native starting point from a pinned catalog, opens the
independent repository to your selected coding agent, verifies the result, and
helps you run it in Apple Simulator.

```sh
curl -fsSL https://tohseno.com/install.sh | bash
tohseno
```

You get ordinary SwiftUI source, Git history, tests, a truthful manifest,
composition locks, and local operating rails. TOHSENO operates no backend for
generated-app content and a shot never needs TOHSENO credentials to build.
Most shots miss. The working, owned prototype is the payoff.

## One intention, then a plan

The terminal and local Studio use the same engine:

1. keep the raw intention and references private and gitignored;
2. ask the already-selected Codex or Claude Code provider for a strict,
   sanitized plan;
3. fall back to the Blank template if planning is offline, times out, or is
   invalid—never switch providers silently;
4. show the proposed app, template, skills, data, identity, and first
   definition of done;
5. compose the accepted plan deterministically;
6. commit and publish an independent repository atomically;
7. let the coding agent finish the app;
8. run pinned manifest, privacy, composition, and skill checks;
9. build and launch in Simulator when the Mac supports it.

Run `tohseno` for the direct path, or `tohseno studio` for the local contact
sheet. Explicit automation is also available:

```sh
tohseno create my-app --no-launch --no-interactive
tohseno create --file intention.md --reference sketch.png
tohseno verify my-app
tohseno run my-app
tohseno my-app
```

Normal handoffs use cwd-independent `tohseno` commands. Bun is a pinned
internal implementation detail; owners do not need to install or invoke it.

## The native factory

New shots use four separate layers:

- `templates/ios-kernel` — a neutral, compiling SwiftUI shell with no writing,
  identity, backend, analytics, or account assumptions;
- `templates/blank` and `templates/daily-game` — bounded starting shapes;
- `skills/` — versioned capabilities with dependencies, conflicts, file
  ownership, acceptance checks, and immutable digests;
- `packages/skills` — the shared loader, resolver, composer, locker, and
  verifier used by CLI and Studio.

The bundled Daily Game composition is implemented with real native skills:
deterministic daily choices, local progress, rank progression, and
owner-initiated share cards. Blank remains the safest fallback.

Every generic shot carries:

- `app.manifest.json`;
- `tohseno.skills.json` and `tohseno.skills.lock`;
- `SHOT.md` and `DONE.md`;
- pinned descriptors and machine/verifier code under `.tohseno/`;
- a generated Xcode project, native tests, and its own Git history.

The lock records the exact kernel, template, ordered skills, digests, and
immutable file hashes. The verifier rejects catalog drift, undeclared
composition changes, unsafe links, tracked private intent, or failed
acceptance files.

## Privacy and ownership

Raw input lives only under gitignored `.tohseno/provenance/` in the shot. The
sanitized plan is tracked; the private intention is not. The chosen coding
agent can read private input under that provider’s account, privacy, and
retention terms. TOHSENO does not forward it to a second provider or a TOHSENO
service.

Every coding-agent exit, including failure, is followed by pinned verification.
If protected provenance or rails cannot be trusted, the result is isolated
instead of presented as ready.

Each shot remains ejectable from birth: no symlink to this repository, no
global CLI dependency, no TOHSENO account, no cloud control plane, and no
silent rewrite after a factory upgrade.

## Compatibility boundary

The earlier continuity writing app remains supported as metadata-v1 legacy
architecture. Its BIP39 identity, crash-safe writing, local API/SQLite,
AppConfig flags, production inspection, and token rails retain their pinned
tests and behavior. They are no longer injected into every new app and are not
claims about the generic kernel.

Generic app creation, composition, Studio/CLI planning, verification, and
Simulator launch are **Implemented**. External deployment, DNS, paid services,
store submission, and other irreversible actions remain owner-approved and are
never performed automatically.

## Development

```sh
bun install
bun run tohseno -- --help
bun run validate templates/ios-kernel/overlay/app.manifest.json
bun test packages/skills packages/manifest packages/cli/tests
bun run check
```

Learn more:

- [What TOHSENO is](WHAT_TOHSENO_IS.md)
- [CLI and machine operations](docs/CLI.md)
- [System architecture](docs/SYSTEM_ARCHITECTURE.md)
- [Local development](docs/LOCAL_DEVELOPMENT.md)
- [Ownership and ejection](docs/EJECTION.md)
- [Deployment boundary](docs/DEPLOYMENT.md)

Apache License 2.0. The license grants no trademark rights to TOHSENO or Anky;
see [TRADEMARKS.md](TRADEMARKS.md).
