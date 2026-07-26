# Ejection

Ejection means the owner can build, operate, modify, migrate, or replace a Shot
without TOHSENO’s permission or a hidden factory dependency. It is true at
creation time, not a later export feature.

## What a Shot owns

- SwiftUI source, tests, configuration, and generated Xcode project;
- canonical app manifest and static landing page;
- `SHOT.md`, `DONE.md`, requested composition, and exact composition lock;
- pinned app-skill descriptors and local skill instructions;
- pinned machine runtime, manifest validator, verifier, and release inventory;
- private gitignored creation provenance;
- an independent Git repository with a neutral factory baseline.

There are no symlinks into a checkout, cache, installation, or network package.
The global CLI authenticates and dispatches tools for convenience, but the
embedded low-level protocol remains available after removal:

```sh
bun .tohseno/machine.ts operations --json
bun .tohseno/machine.ts verify --json
open Shot.xcodeproj
```

Bun here is the ejected verification toolchain dependency. A published managed
artifact may supply it behind `tohseno verify <shot>` and
`tohseno run <shot>`; no 0.5.0 managed artifact has been published.

## Composition ownership

`tohseno.skills.lock` pins the factory release, kernel, template, ordered skill
dependency closure, versions, digests, file owners, and immutable hashes. The
Shot does not need the global catalog to explain what it received. It may later
change or remove code as ordinary owned source, but it must update its manifest
and lock truthfully before claiming pinned verification.

The sanitized plan is portable product context. Raw intention and references
are private local provenance; inspect or remove that gitignored directory
before sharing a whole filesystem archive.

## Data and external authority

The app manifest identifies every declared local/remote data category,
storage location, network purpose, generated-app runtime identity strategy, entitlement,
integration, production declaration, and irreversible operation. TOHSENO owns
none of the bundle IDs, signing teams, accounts, domains, data stores, backups,
or provider relationships.

The neutral kernel has no service dependency. A skill may add one only when
its descriptor, implementation, manifest declarations, and acceptance checks
make it explicit.

## Anti-lock-in acceptance

A Shot is not ejectable if its core action requires a TOHSENO account,
endpoint, secret, CLI, mutable global validator, factory cache, or undisclosed
service; if build assets are linked out of the repository; if only TOHSENO can
read canonical data; or if leaving requires publishing private input.

Open source alone is insufficient. Reproducible source, pinned checks, explicit
data and authority declarations, ordinary Git, and owner-held identifiers make
ejection real.
