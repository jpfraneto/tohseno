# Ejection

Ejection means the owner can continue operating, modifying, migrating, or replacing the application without TOHSENO permission or a hidden control-plane dependency. It is an architectural property from the beginning, not a cancellation favor.

## Current-slice meaning

The current repository does not yet generate a complete native application. For self-hosted orders, the first ejection artifact is the private capsule, original `MASTER_PROMPT.md`, source contract, manifest/runbook references, and explicit instruction to return all resulting repositories, credentials, production URLs, ownership details, and ejection instructions.

For client-owned mode, the customer already owns the production accounts and identifiers; TOHSENO access is scoped and revocable. For Anky-operated mode, ownership/ejection terms require an explicit acceptance/agreement and cannot be inferred from submission.

## Ejection package contract

A complete generated-application ejection package should contain:

- original `MASTER_PROMPT.md`, owner-authorized private evolution history, and
  the locked/current manifest and release lineage;
- all source repositories at specific commits, including generated and handwritten changes;
- dependency manifests/locks and third-party license notices;
- reproducible local build, test, migration, and release commands;
- database schemas/migrations and a documented export in an open format;
- private artifact/export codecs and compatibility fixtures;
- infrastructure definition, topology, health checks, backup/restore and rollback procedures;
- environment-variable names and secret-purpose inventory, never secret values in Git;
- customer-owned domain/DNS records and registrar ownership details;
- Apple/Google organization, bundle/package IDs, signing and store listing ownership;
- provider accounts, billing ownership, webhooks, callback URLs, and scoped roles;
- operational state, outstanding incidents, retention/deletion inventory, and runbooks;
- practice identity, content, backup, subscription, server ownership, and reflection-history migration plans as separate items;
- production URLs and a verified post-handoff smoke test;
- a list of TOHSENO/Anky trademarks or licensed assets that may need replacement.

Credentials should be transferred through an approved secret manager or owner-controlled provider invitation, then rotated where appropriate. Never put them in the package archive, Markdown, Git history, operator event metadata, or email body.

## Data export and privacy

The owner must be able to export user-owned continuity artifacts without TOHSENO availability. Export must preserve canonical bytes when integrity/signature relationships depend on them and clearly distinguish a human-readable transformed export from a canonical artifact.

Ejection does not make private user content public. The transfer plan must name encryption keys, attachment stores, backups, server processors, and deletion duties. Moving infrastructure does not automatically move practice identities, subscriptions, provider retention, or app-store ownership.

## Handoff sequence

1. Inventory source, accounts, identifiers, data stores, processors, backups, secrets, domains, stores, and current operators.
2. Confirm the receiving owner and exact target accounts.
3. Produce and validate the package without changing production.
4. Restore/build it in an isolated owner-controlled environment.
5. Transfer scoped roles and secret custody; rotate only with explicit owner approval.
6. Cut over traffic/DNS/store operations through a reviewed plan.
7. Run action, local persistence, privacy, payment/reflection where applicable, deletion, and rollback smoke tests.
8. Revoke TOHSENO access after the owner confirms operation.
9. Record `EJECTED` and the safe handoff facts; do not put credentials or private data in the event.
10. Apply the agreed TOHSENO-side retention/deletion procedure.

## Anti-lock-in acceptance tests

An application is not honestly ejectable if any of these are true:

- the core local action requires a TOHSENO endpoint or secret;
- only TOHSENO can read/export the artifacts;
- bundle/package IDs, domains, or customer-mode infrastructure are held in an undisclosed third-party account;
- migrations or deployment can run only through an undocumented internal tool;
- source does not reproduce the production candidate;
- revoking TOHSENO access breaks unrelated customer credentials;
- leaving requires publication of private content or loss of identity/recovery material;
- a trademark or proprietary asset dependency is hidden until handoff.

Open source is necessary but not sufficient. Operational clarity, data portability, identifier ownership, and tested recovery make ejection real.
