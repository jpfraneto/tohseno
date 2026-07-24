# 0011: Compile intentions into kernel, template, and skill compositions

- Status: Accepted
- Date: 2026-07-24

## Context

The first factory always copied a continuity writing application. That proved
independent repositories, private provenance, pinned verification, and native
operations, but it made writing, seed-phrase identity, and a local backend look
universal. They are one app architecture, not a neutral factory foundation.

An unconstrained coding agent starting from an empty directory would remove
those assumptions but also remove deterministic ownership, dependency,
verification, and ejection guarantees.

## Decision

New shots compile one private intention into a strict sanitized plan using the
already-selected coding-agent provider. Planner output is constrained to the
released catalog. Invalid or unavailable planning selects Blank and never
switches providers.

The factory then deterministically composes:

1. one neutral platform kernel;
2. one starting template;
3. a dependency-closed, conflict-free ordered set of app skills.

Descriptors declare versions, overlays, dependencies, conflicts, replacement
authority, instructions, and acceptance files. The resulting lock records
content digests, ownership, and immutable file hashes. The verifier
authenticates both the released catalog inputs and the applied result.

The generic manifest describes app mechanics without continuity-specific
fields. Shot metadata dispatches generic schema 2 and legacy continuity schema
1 through separate validators and verifier branches.

AI interprets meaning and performs bounded product coding. Catalog parsing,
dependency resolution, collision handling, hashing, publication, and
verification remain deterministic.

## Consequences

- The factory can make materially different native apps without treating an
  empty directory as a reliable starting point.
- Installed capabilities are inspectable and testable rather than prose-only
  agent advice.
- Blank is a safe fallback, while richer templates can remain fast.
- Raw intent stays private; only a sanitized plan is tracked.
- Existing continuity shots retain their pinned behavior and are never
  silently migrated.
- Adding a template or skill requires a descriptor, actual source, acceptance
  evidence, release inventory coverage, documentation, and collision review.
