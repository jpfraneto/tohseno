---
title: Persistence and recovery map
description: Where state lives, which component owns it, and what survives a restart.
---

Default paths are shown. Tests override them with isolated roots.

| State | Default location | Owner |
| --- | --- | --- |
| Phone identity | iOS Keychain | Companion |
| Phone projection, replay state, outbox | encrypted Application Support `TOHSENO/companion-state.bin` | Companion |
| Exact outbound envelopes/references | protected `TOHSENO/outbox/` | Companion |
| Relay mailboxes | configured `COMPANION_RELAY_ROOT` | relay service |
| Mac workspace identity | Keychain + `~/.tohseno/service/workspace.json` reference | workspace service |
| Paired devices, relay cursors, inbox/outbox | `~/.tohseno/service/{devices,inbox,outbox}` | Companion coordinator |
| Commands and exact inputs | `~/.tohseno/service/command-journal/<command-id>/` | application service |
| Generated app source/history | normally `~/Desktop/Tohseno/<app>/` | engine + owner |
| Generated execution | `<app>/.tohseno/executions/<execution-id>/` | application + engine |
| Adopted-project records | `~/.tohseno/service/living-projects-v1/` | workspace service |
| Recipient network source | normally `~/Developer/Tohseno/` | workspace service + owner |
| Installed factory releases | `~/.tohseno/releases/`, `current`, `bin/` | installer/native app |
| Intelligence selection | `~/.tohseno/service/intelligence-v1.json` + optional Keychain refs | workspace service |
| Publication jobs | `~/.tohseno/service/network-publications-v1/` | Mac + Companion approvals |
| Private follows and Updates | `~/.tohseno/service/network-preferences-v1/` | Mac + Companion |
| Public catalog, indexes, upload staging, jobs, profiles, aliases | configured `REGISTRY_ROOT` | Registry service |
| Immutable public source/icon blobs | `REGISTRY_ROOT/blobs/sha256` in filesystem mode or private Cloudflare R2 `sha256/<digest>` objects in R2 mode | Registry blob store |
| Claims index and relayer jobs | configured durable Registry root | Claims service |

## Permissions and write discipline

Private stores use bounded versioned schemas, owner-only directory/file permissions, symlink rejection, safe relative paths, and atomic replacement. Unknown store versions fail closed. Relay and public services use configured absolute roots, capacity bounds, create-exclusive or atomic writes, and restart-safe metadata ordering. R2 changes only the immutable blob byte store: catalogs, Claims, jobs, and incoming staging remain local. Remote pending and final objects are create-only and are read back through SHA-256 and length verification before publication; an ETag is never treated as content evidence.

## Generated app boundary

For a generated app, `.tohseno/` is integral app-local durable state and must not be blanket-ignored. Safe identity, expression, capability, protocol-version, and immutable Evolution structure can remain Git-visible. Exact intentions, private inline lineage, references, feedback, executions, logs, retained artifacts, and `.tohseno/private/` have explicit ignore rules.

Source-tree commitment excludes `.tohseno/` to avoid self-reference. That is different from repository durability, and neither implies public Registry publication.

## Known restart limit

Commands, outboxes, service state, prepared work, and device-waiting artifacts survive ordinary restarts. Arbitrary in-progress harness mutation cannot be resumed safely after a whole-Mac restart when no runner remains. It is finalized as failed rather than automatically repeated.
