# ADR 0001: Make every generated app ejectable from birth

- Status: Accepted
- Date: 2026-07-10

## Context

Visible source alone does not prevent lock-in. Store identifiers, signing
authority, private formats, credentials, and undocumented build or release
steps can still make an app dependent on its factory.

## Decision

Every Shot materializes as an independent repository containing ordinary app
source, its manifest and composition lock, tests, build and verification
commands, and an owner-reviewable landing page. It builds and runs without a
TOHSENO account, credential, node, server, wallet, or unpublished dependency.

App-specific remote services are mechanics declared by that app. They never
turn TOHSENO infrastructure into hidden runtime authority. Store, signing, and
external-service accounts remain owner-controlled.

## Consequences

- A Shot can leave the factory immediately without conversion.
- Evolutions preserve the same Shot identity and repository rather than
  creating factory-owned forks.
- Generated apps cannot require a TOHSENO runtime endpoint.
- Protocol participation and distribution are optional operations, not
  prerequisites for local ownership.
- Export, deletion, and migration behavior belong to the app manifest when its
  mechanics require them.

## Non-goals

This ADR does not grant trademarks, transfer third-party accounts, or perform
publication. Those remain separate ownership and external-action boundaries.
