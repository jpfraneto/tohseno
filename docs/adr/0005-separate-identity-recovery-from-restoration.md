# ADR 0005: Separate practice-identity recovery from other restoration

- Status: Accepted
- Date: 2026-07-10

## Context

A recovery phrase can recreate a cryptographic key while leaving archives, backups, subscriptions, server records, provider history, and infrastructure ownership behind. Treating “recover identity” as “restore everything” creates false promises and can mix content with a different owner.

## Decision

Recovering or rotating a practice identity is a distinct operation from:

- restoring local content;
- restoring encrypted backups;
- recovering subscription/entitlement state;
- transferring server/account ownership;
- restoring reflection history;
- linking another device or app;
- migrating infrastructure or store ownership.

User interfaces, manifests, runbooks, and migration plans name each operation and its result separately. Importing recovery material over existing content requires a reviewed migration plan; it never silently reassigns that content.

## Consequences

- Recovery reports can be partial and truthful rather than a single success flag.
- Identity rotation needs old/new ownership mapping, rollback, and provider/subscription analysis.
- Cross-app identities remain separate absent explicit selective bridging consent.
- Deletion inventories also separate key, content, cloud, server, provider, and commercial systems.

## Non-goals

This ADR does not select a recovery UI, cloud provider, or generic cryptographic suite. Android off-device recovery and cross-device bridging remain open.
