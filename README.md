# TOHSENO

## Take another one.

Give TOHSENO one intention. It turns that private input into a sanitized app
plan, composes a native starting point from a pinned catalog, opens the
independent repository to your selected coding agent, verifies the result, and
helps you run it in Apple Simulator.

That intention is one **Shot**. Later changes are **Evolutions** of the same
Shot, with the same stable identity and repository. Public distribution has
three precise protocol states: `EVOLVING`, `PUBLISHED`, and `APP_STORE`.

```sh
bun install --frozen-lockfile
bun run tohseno
```

This checkout is the implemented `0.5.0` source and its managed release is
**Prepared**, not published. Publishing the artifact, updating the canonical
installer, and deploying the site remain owner-approved external actions.

You get ordinary SwiftUI source, Git history, tests, a truthful manifest,
composition locks, and local operating rails. New apps start with local,
account-free data defaults and declare any other storage or network behavior
in their manifest. TOHSENO itself operates no generated-app content backend,
and a Shot never needs TOHSENO credentials to build. Most shots miss. The
working, owned prototype is the payoff.

## One intention, then a plan

The terminal and local Studio use the same engine:

1. keep the raw intention and references private and gitignored;
2. ask the already-selected Codex or Claude Code provider for a strict,
   sanitized plan;
3. fall back to the Blank template if planning is offline, times out, or is
   invalid—never switch providers silently;
4. show the proposed app, template, skills, data, runtime identity, and first
   definition of done;
5. compose the accepted plan deterministically;
6. commit and materialize an independent repository atomically;
7. let the coding agent finish the app;
8. run pinned manifest, privacy, composition, and skill checks;
9. build and launch in Simulator when the Mac supports it.

In this prepared source checkout, use `bun run tohseno`; the managed
`tohseno` executable is not published. Explicit source commands are:

```sh
bun run tohseno -- create my-app --no-launch --no-interactive
bun run tohseno -- create --file intention.md --reference sketch.png
bun run tohseno -- evolve my-app
bun run tohseno -- status my-app
bun run tohseno -- verify my-app
bun run tohseno -- run my-app
bun run tohseno -- my-app
```

The prepared managed artifact supplies cwd-independent `tohseno` commands and
its pinned runtime, but no installer or package for 0.5.0 has been published.

## The native factory

New Shots use four separate layers:

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

Every Shot carries:

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

Raw input lives only under gitignored `.tohseno/provenance/` in the Shot. The
sanitized plan is tracked; the private intention is not. The chosen coding
agent can read private input under that provider’s account, privacy, and
retention terms. TOHSENO does not forward it to a second provider or a TOHSENO
service.

Every coding-agent exit, including failure, is followed by pinned verification.
If protected provenance or rails cannot be trusted, the result is isolated
instead of presented as ready.

Each Shot remains ejectable from birth: no symlink to this repository, no
global CLI dependency, no TOHSENO account, no cloud control plane, and no
silent rewrite after a factory upgrade.

## Optional public protocol

Signed public records can identify a Shot, append Evolutions, advance its
distribution lifecycle, and attach deployment-agnostic Appcoin links. Records
use deterministic serialization, hash chaining, and role-qualified Builder
identity. They contain no wire fields for raw prompts, app-user content,
credentials, local databases, or unpublished source bytes.

The Bun/SQLite reference node validates and indexes those records. Each valid
signature makes a portable Builder attestation over the declared public
claims; it does not independently prove ownership, claim accuracy, or a
globally preferred history. Given the same accepted record sequence, another
registry derives the same public projection. Production trust roots and
resolution of competing valid histories across nodes remain Open. Taking,
evolving, building, verifying, and running locally require no node, server,
account, wallet, chain, or TOHSENO mobile app.

The TOHSENO mobile application is intentionally absent. The first stable
factory release must generate it as that release's first Shot; see the
[Genesis invariant](docs/GENESIS.md).

App creation, composition, Studio/CLI planning, verification, and Simulator
launch are **Implemented**. External deployment, DNS, paid services,
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
- [Shot protocol](docs/PROTOCOL.md)
- [Genesis invariant](docs/GENESIS.md)
- [System architecture](docs/SYSTEM_ARCHITECTURE.md)
- [Local development](docs/LOCAL_DEVELOPMENT.md)
- [Ownership and ejection](docs/EJECTION.md)
- [Deployment boundary](docs/DEPLOYMENT.md)

Apache License 2.0. The license grants no trademark rights to TOHSENO;
see [TRADEMARKS.md](TRADEMARKS.md).
