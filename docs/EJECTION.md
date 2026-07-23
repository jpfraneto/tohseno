# Ejection

Ejection means the owner can operate, modify, migrate, or replace a shot
without TOHSENO’s permission or a hidden factory dependency. It is a property
of every created repository, not a later cancellation service.

## What the shot owns

The factory owns reusable source and an immutable local cache. A shot receives
complete copies of:

- SwiftUI application source, XcodeGen source, generated project, and tests;
- the Bun API, SQLite migrations, and non-secret production declarations;
- its manifest, static landing page, configuration seams, and runbooks;
- canonical `AGENTS.md`, a Claude adapter, and the continuity skill;
- the exact machine runtime, manifest validator, and verifier used at creation;
- factory/schema/template provenance and the release file inventory;
- an independent `.git` directory and neutral baseline commit.

There are no symlinks into the TOHSENO checkout, release cache, or managed
installation. `.tohseno/` inside the repository is owned audit and operational
material, not a network client.

From the shot root, without invoking the global CLI:

```sh
bun .tohseno/machine.ts operations --json
bun .tohseno/machine.ts dev start --json
bun .tohseno/machine.ts verify --json
bun .tohseno/machine.ts dev stop --json
open Writing.xcodeproj
```

Bun and Xcode are documented toolchain dependencies, just as Swift and Git
are; they do not have to come from TOHSENO. Removing or upgrading the global
CLI does not rewrite the pinned scripts or select a newer validator.

## Process and data ownership

Runtime state is shot-scoped:

- `.tohseno/data/` contains persistent development SQLite data;
- `.tohseno/run/` contains ephemeral state, logs, locks, and derived build data;
- `Config/DevelopmentEndpoint.xcconfig` contains the current Debug endpoint.

All are gitignored. The supervisor binds the API to localhost, records an exact
instance marker, and stops only PIDs whose command identity matches this shot.
It never takes ownership of a port or another repository’s process. Stop
preserves the development database and logs while removing active state and
the generated endpoint.

An owner can back up, inspect, migrate, or delete the SQLite file directly.
Production inspection names the required path and backup declarations; no
undisclosed TOHSENO database exists.

## App data, identity, and infrastructure

- The writing action and history stay on the device as owner-readable files.
- Identity is a BIP39 phrase with a separate synchronizable Keychain backup;
  identity backup is not content backup.
- The baseline API receives health requests and stores operational metadata
  only. TOHSENO operates no backend that receives generated-app content.
- Bundle identifiers, signing team, store account, production API, domain,
  database, backup destination, and any future provider account belong to the
  owner.
- Setup writes public identifiers and secret *paths* to gitignored local
  configuration; it never copies key material into Git.
- A Quick Tunnel URL is disposable development transport and is not an
  infrastructure asset or production dependency.

`MASTER_PROMPT.md` remains private product input. It is gitignored and never
becomes provenance, ejection metadata, a command argument, or a log entry.

## Existing and adopted repositories

Existing Phase 1 shots continue unchanged with their own pinned verifier.
Global `machine verify` has an explicit legacy fallback; other new runtime
operations report that those repositories do not contain the newer pinned
machinery. There is no silent upgrade or rewrite.

`tohseno adopt <path>` remains an explicit, narrow operation for a compatible
independent iOS repository. It adds `.tohseno` provenance and tooling only
after confirmation, validates before publication, and removes its addition if
the new verifier fails. It does not move, rewrite, stage, or commit owner code,
nor replace existing agent instructions.

## Anti-lock-in acceptance

A shot is not honestly ejectable if any of these become true:

- its core action requires a TOHSENO account, endpoint, secret, CLI, or cache;
- pinned validation resolves to mutable global or network code;
- source or required assets are symlinked into the factory;
- only TOHSENO can read or export app artifacts or SQLite data;
- the build cannot be reproduced from source and documented toolchains;
- process shutdown claims ownership by port instead of exact instance;
- a Debug tunnel or local bypass can enter a Release archive;
- domains, bundle IDs, store accounts, backups, or infrastructure are held in
  an undisclosed third-party account;
- leaving requires publishing private content or abandoning the recovery
  phrase.

Open source is necessary but not sufficient. Pinned local operations, data and
identifier ownership, explicit production references, and a reproducible build
make ejection real.
