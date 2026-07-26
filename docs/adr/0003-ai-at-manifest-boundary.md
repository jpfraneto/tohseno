# ADR 0003: Put AI interpretation at the manifest boundary

- Status: Accepted
- Date: 2026-07-10

## Context

Human intentions are ambiguous, while composition, identity, persistence,
privacy, and verification require reproducible behavior that cannot depend on
a model at runtime.

## Decision

AI interpretation belongs between private human intent and a sanitized,
validated app plan. The deterministic factory then composes one kernel, one
template, and an ordered dependency-closed skill set. Runtime-enforced
mechanics live in the app manifest and code; agent guidance remains separate.

## Consequences

- A model cannot silently substitute prose for an installed capability.
- Composition locks and manifests remain deterministic and testable.
- Provider failure selects a declared fallback rather than changing providers
  or inventing mechanics.
- Generated apps do not require AI or TOHSENO at runtime unless the app itself
  explicitly declares such a mechanic.
- Unsupported requests are surfaced rather than represented by invented fields.

## Non-goals

This ADR does not make model output authoritative for identity, storage,
protocol validation, or distribution state.
