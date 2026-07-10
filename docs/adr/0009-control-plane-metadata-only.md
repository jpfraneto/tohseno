# ADR 0009: Keep end-user continuity content out of the control plane

- Status: Accepted
- Date: 2026-07-10

## Context

TOHSENO needs to operate orders and eventually application health. Centralizing raw writings, photos, recordings, reflections, and practice secrets would create a high-risk warehouse and contradict local-first ownership.

## Decision

The TOHSENO control plane stores operational metadata necessary for intake, commerce, deployment, health, and ownership. It does not store or transit future end-user continuity content by default.

Submitted `MASTER_PROMPT.md` is encrypted customer intake used to provide the service; it is not end-user continuity content. An app-specific reflection/sync service, if declared, is a separate data plane with explicit consent, retention, encryption, ownership, deletion, and ejection contracts. It may not reuse the control-plane database merely for convenience.

## Consequences

- Operators can inspect application/order health without seeing runtime private content.
- Logs and analytics remain content-free.
- Generated apps commit action/events locally and degrade safely when services fail.
- Cross-plane identifiers and telemetry are minimized and purpose-bound.
- Backups, access roles, incident response, and deletion are documented separately for each data plane.

## Non-goals

This ADR does not prohibit all app-specific hosted processing. It requires that processing to be optional/manifested and outside the general control-plane warehouse.
