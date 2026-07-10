# Operator runbook

This runbook is part of the ejection package. Replace bracketed operational
facts only after they are real; never place secret values in this file.

## Approval boundary

Stop and obtain explicit owner approval before any paid infrastructure is
created, money is spent, DNS is changed, an application is submitted to a
store, or a production credential is rotated. Ask for scoped account access,
not passwords. Record who owns each bundle ID, package ID, store listing,
domain, repository, service, database, backup, and signing identity.

## Required release evidence

- The checked-in manifest validates and its diff has owner approval.
- Fresh install reaches the core action before authentication or recovery UX.
- Action, checkpoint, completion, local commit, export, and return work offline.
- One seal transition creates exactly one stable event and immutable artifacts.
- Process death and interruption behave as the manifest declares.
- Logs, diagnostics, analytics, and crash reports contain no private artifact.
- Reflection, synchronization, proof, and payment are absent unless declared.
- Accessibility and target-platform lifecycle tests pass.
- Backup, deletion, migration, rollback, and server-disable procedures are
  exercised against a non-production environment.

## Handoff inventory

Give the owner the source repositories and commit hashes, manifest and prompt,
build/test commands, application identifiers, account-ownership map, scoped
access list, environment-variable names, deployment topology, production URLs,
store records, backup/restore test, deletion inventory, rollback procedure, and
known limitations. Never include secret values in an ejection document; hand
them over through the owner's approved secret manager.

## Ejection test

From a clean checkout under owner-controlled accounts, build and test every
target, restore a representative encrypted backup, disable every TOHSENO-owned
optional service, and verify that the core continuity loop still works. Ejection
is incomplete until the owner can operate, update, delete, and redeploy without
TOHSENO credentials.
