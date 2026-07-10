# ADR 0008: Make every generated application ejectable from birth

- Status: Accepted
- Date: 2026-07-10

## Context

Open source alone does not prevent operational lock-in. Domains, store identifiers, infrastructure accounts, opaque migrations, credentials, private formats, and undocumented deployment tools can make leaving impossible even when source is visible.

## Decision

Every generated application begins with an ejection contract. It preserves the source prompt/manifest, source and dependency locks, data schemas and export formats, infrastructure/runbooks, environment-variable inventory, identifiers/accounts, ownership map, build/test/release commands, and migration/deletion procedures.

Operating mode determines the current custodian, but no mode may rely on an undocumented TOHSENO-only secret or endpoint for the core local continuity loop. Client-owned assets remain in customer accounts with scoped TOHSENO access.

## Consequences

- Changes must document migration and handoff impact, not only implementation.
- Private artifact formats require export/compatibility fixtures.
- Credential transfer uses owner-controlled mechanisms and is followed by scoped revocation/rotation.
- `EJECTED` records a safe operational handoff; it is not automatic deletion.
- Customers stay by choice and service quality, not technical captivity.

## Non-goals

This ADR does not grant TOHSENO or Anky trademarks under Apache-2.0 or settle the long-term license for generated applications.
