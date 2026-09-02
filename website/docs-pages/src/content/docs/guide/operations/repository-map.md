---
title: Repository map
description: Where each product, protocol, service, contract, and compatibility responsibility lives.
---

## Authority first

1. `protocol/` — normative bytes and verification law.
2. `docs/adr/` — accepted architecture/product decisions.
3. `MASTER_PROMPT.md` — historical v0.7 implementation input, not current authority.
4. `genome/LAWS.md` — historical planning law retained for verification compatibility.
5. `docs/STATE.md` — current shipped/inactive/deferred snapshot.

## Product and private loop

| Path | Responsibility |
| --- | --- |
| `macos/Tohseno/` | Native SwiftUI Mac app, factory client, packaging and product tests |
| `companion/apple/TohsenoCompanion/` | Native iPhone Companion product |
| `sdk/apple/TohsenoCompanionKit/` | Swift private wire, crypto, storage and client SDK |
| `companion/` | Rust private wire, pairing, capability, envelope and vectors |
| `cli/` | CLI, loopback Workspace Service, LaunchAgent lifecycle and Companion coordinator |
| `application/` | Command admission, journals, idempotency, presentation, factory lease and execution manager |
| `engine/` | Shot lifecycle, source materialization, harness and deterministic gates |
| `studio/` | Retained thin loopback UI; deleted dashboard features stay deleted |

## Apple materialization

| Path | Responsibility |
| --- | --- |
| `apple-identity/` | Apple identity support and tests |
| `fascia/apple/` | Reusable Apple Fascia, capability definitions and Swift package |
| `fixtures/` | Cross-surface presentation and deterministic fixtures |
| `oneshot/` | Canonical installer source retained under release gates |

## Public network

| Path | Responsibility |
| --- | --- |
| `protocol/` | Neutral public records, exact encodings and conformance |
| `contracts/` | BuilderAccountFactory, BuilderAccount, ShotRegistry and additive Claims contract |
| `network/` | Catalog, sanitized-source, recipient, Claim and public-network logic |
| `node/` | Public action validation/storage and rebuildable indexes |
| `website/apps/companion-relay/` | Content-blind private mailbox transport |
| `website/apps/site/` | Public site, Registry/Claims service, managed compute, web handoff |
| `release/` | Immutable activation/readiness/evidence records |

## Documentation site

`website/docs-pages/` is the standalone Astro Starlight site for `docs.tohseno.com`. It owns the familiar documentation shell, Pagefind search, quiet character guides, per-page AI handoff, and generated `llms.txt` corpus. `website/apps/site/public/docs.html` remains only a compatibility surface on the main website and is not included in the standalone docs build.

## Historical and compatibility material

`history/`, frozen tags, legacy schemas, recording-layer files, `MASTER_PROMPT.md`, and old readable execution variants remain for byte compatibility and audit. Their presence does not make them active product architecture. In particular, readable names such as Conception do not authorize a new Conception phase.
