---
title: Files and directories
description: A reference for owner-visible apps, app-local records, service state, native releases, relay data, and public service roots.
---

## Owner-visible source

| Path | Contents |
| --- | --- |
| `~/Desktop/Tohseno/<app>/` | Default generated app Git working tree |
| `<app>/.tohseno/` | Integral generated-Shot identity, history and execution boundary |
| `<app>/.tohseno/TASK.md` | Private exact task packet for one harness execution |
| `<app>/.tohseno/executions/<execution-id>/` | Prepared identity, events, logs, completion and private receipt |
| `~/Developer/Tohseno/` | Default visible source for verified network imports/forks |
| existing project path | Adopted source; Tohseno does not add or move repository files |

`.tohseno/` is never blanket-ignored. Exact private and transient children are ignored explicitly. Source-tree commitments exclude the directory under their separate hashing law.

## Mac private service

| Path | Contents |
| --- | --- |
| `~/.tohseno/bin/tohseno` | Stable CLI/service entry point |
| `~/.tohseno/releases/` and `current` | Verified installed factory releases and selection |
| `~/.tohseno/service/workspace.json` | Reference to Keychain-backed workspace identity |
| `~/.tohseno/service/command-journal/<id>/` | Durable canonical command and exact inputs |
| `~/.tohseno/service/{devices,inbox,outbox}` | Pairing grants, cursors, admitted private envelopes and receipts |
| `~/.tohseno/service/living-projects-v1/` | External adopted-project identities, evolutions and commands |
| `~/.tohseno/service/network-publications-v1/` | Durable Ship/Update approval jobs |
| `~/.tohseno/service/network-preferences-v1/` | Private Following and Updates |
| `~/.tohseno/service/intelligence-v1.json` | Selected harness/route configuration; secrets use Keychain refs |

## iPhone private state

Identity keys live in iOS Keychain. Encrypted state is in protected Application Support `TOHSENO/companion-state.bin`; exact encrypted queued envelopes and references live under `TOHSENO/outbox/` until verified receipt.

## Services

`COMPANION_RELAY_ROOT` is the configured absolute root for opaque relay mailboxes. `REGISTRY_ROOT` is the configured durable public catalog/blob/profile/index root. `MANAGED_COMPUTE_ROOT` owns managed balance authority. Production values are operator configuration and are not hard-coded documentation defaults.

## Immutable repository evidence

`release/contract-activations/` holds generation 0.8 trust policy and activation. `release/claims-activations/` holds additive Claims activation. `release/*READINESS.json` separates source facts from external physical and publication gates. These committed instances are never edited in place.
