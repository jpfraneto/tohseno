# ADR 0004: Compile intentions into kernel, template, and skill compositions

- Status: Accepted
- Date: 2026-07-24

## Context

A Shot needs enough deterministic structure to build, verify, and remain
independently owned, while still allowing materially different app mechanics.
An empty directory provides no such contract, and a product-specific base
would make one app category look universal.

## Decision

New Shots compile one private intention into a strict sanitized plan using the
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

The app manifest describes runtime mechanics through one canonical schema.
There is no compatibility schema or alternate Shot ontology.

AI interprets meaning and performs bounded product coding. Catalog parsing,
dependency resolution, collision handling, hashing, repository
materialization, and verification remain deterministic.

## Consequences

- The factory can make materially different native apps without treating an
  empty directory as a reliable starting point.
- Installed capabilities are inspectable and testable rather than prose-only
  agent advice.
- Blank is a safe fallback, while richer templates can remain fast.
- Raw intent stays private; only a sanitized plan is tracked.
- Adding a template or skill requires a descriptor, actual source, acceptance
  evidence, release inventory coverage, documentation, and collision review.
