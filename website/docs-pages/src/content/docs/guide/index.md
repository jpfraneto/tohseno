---
title: Tohseno documentation
description: Choose the shortest path into the product, then inspect the machinery only when you need it.
---

The documentation is arranged around the thing you are trying to do, not the shape of the repository.

## Choose a path

- **New to Tohseno:** [what Tohseno is](/guide/start/what-is-tohseno/) → [requirements](/guide/start/requirements/) → [install and onboard](/guide/start/install-and-onboard/)
- **Make something small:** [create an app](/guide/start/create-an-app/)
- **Bring an existing app:** [adopt an Xcode project](/guide/start/adopt-an-app/)
- **Change what is already in your hand:** [evolve an app](/guide/start/evolve-an-app/)
- **Understand the private and public boundaries:** [trust boundaries](/guide/security/trust-boundaries/)

## Go deeper only when it helps

You do not need the protocol to make an app. Start with **Start here**, then use **The product** whenever a screen or word needs explanation.

Read **How the machine works** when you want the actual path from a request to source, build, signature, installation, and durable history. Read **Protocol** for exact identities, commitments, encodings, signatures, contract generation, activation, and conformance. **Security & privacy** states what each component can see and the conditions that stop the machine.

Operators and contributors should finish with **Operate & develop** and **Reference**.

## The whole machine in one view

```text
person on iPhone                         person on Mac
Tohseno Companion                       Tohseno.app
        │ signed + encrypted request          │
        └──────── content-blind relay ────────┤
                                               ▼
                                     Local Workspace Service
                                               │ durable admission
                                               ▼
                                     one bounded coding harness
                                               │ source changes
                                               ▼
                                 deterministic Xcode verification
                                               │ locally signed app
                                               ▼
                                          physical iPhone

optional explicit public path:
Companion approval → sanitized source → generation 0.8 witness
                   → one Ship → later Updates
                   → separately activated, non-transferable Claim receipt
```

The local app, private command channel, public Registry, and Claims contract are different boundaries. A local build is not a publication. A pending transaction is not a Claim. A successful build is not an installation. These docs keep those facts separate.

## Authority

This website explains the repository; it does not replace it. Exact public bytes and validation rules live in `protocol/`. Accepted product decisions live in `docs/adr/`. The current shipped/inactive/deferred snapshot lives in `docs/STATE.md`. If this site disagrees with those sources, the repository sources win.

[Open the source-of-truth map](/guide/reference/source-of-truth/)
