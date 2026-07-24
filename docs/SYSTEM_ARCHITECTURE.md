# System architecture

## Product boundary

TOHSENO is a local intention compiler and app factory. It converts private
human input into a sanitized plan, then deterministically turns that plan into
an independent iOS repository. It operates no generated-app content backend.

## Creation pipeline

```text
CLI or loopback Studio
        |
        v
input normalization ──> private .tohseno/provenance/
        |
        v
selected-provider planner ──> strict sanitized ShotPlan
        |                          |
        | invalid/offline          v
        └──────────────> Blank fallback / owner review
                                   |
                                   v
released catalog resolver
                                   |
                                   v
iOS kernel + template + dependency-ordered skills
                                   |
                                   v
composition lock + generic manifest + SHOT/DONE
                                   |
                                   v
atomic Git publication
                                   |
                                   v
coding agent -> pinned verifier -> native Simulator runner
```

The AI boundary ends at semantic planning and coding work. Catalog loading,
dependency resolution, collision checks, file ownership, hashing, manifest
validation, publication, and verification are deterministic.

## Catalog layers

The released `catalog/` contains:

- `kernels/ios-kernel`: neutral SwiftUI shell and build configuration;
- `templates/blank` and `templates/daily-game`: starting shapes;
- `skills/<id>`: descriptors, instructions, overlays, dependencies,
  conflicts, and acceptance files.

`packages/skills` is the shared engine. It bounds input sizes, rejects links
and unsafe paths, validates exact descriptor fields, sorts dependency closure,
prevents undeclared overwrites, applies explicit replacements, and emits the
lock.

The lock records release ID, kernel/template/skill versions and digests,
ordered skills, immutable applied-file hashes, and ownership. Files intended
for later customization are excluded from immutable hashes explicitly, not
implicitly.

## Generic manifest

`app.manifest.json` declares application identity, composition, local and
remote data, storage, network, identity strategy, entitlements, integrations,
native operations, privacy, production readiness, and irreversible operations.
It does not inherit continuity-specific fields.

Metadata schema 2 identifies `generic-app-v1` and pins the composition plus a
sanitized-plan digest. The verifier dispatches on metadata schema. Schema 1
continues through the legacy continuity manifest and verifier.

## Ownership and privacy

Raw intention and reference bytes are mode-restricted and gitignored.
Protected provenance is snapshotted before the coding agent runs and restored
or isolated if modified. Public worktree verification searches for copied
private input, unsafe links, changed pinned machinery, manifest drift, and lock
violations.

The selected provider is the only planning/coding provider used. Provider
credentials are removed from child environments unless a recognized,
explicitly authorized operation needs its own credential path. Logs and
progress journals are bounded and content-free.

## Runtime doors

The global CLI authenticates cached release tools before dispatch. Each shot
also embeds its verifier and machine protocol for ejection. Generic iOS
operations read project, scheme, and product names from the generic manifest
and need no development backend. Legacy continuity operations retain their
local API, SQLite, tunnel, production, and token surfaces.

Studio is a loopback application over the same services. It does not own shot
state and can be closed without affecting the CLI or an app.
