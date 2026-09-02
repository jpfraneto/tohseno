---
title: Architecture overview
description: Components, responsibilities, and the boundaries between private execution and public witnessing.
---

Tohseno is one product made from several narrow components. Each owns one kind of truth.

## Runtime components

| Component | Responsibility | Must not do |
| --- | --- | --- |
| `Tohseno.app` | Native Mac navigation and product state | Implement a second factory |
| Local Workspace Service | Authenticate local clients, own journals, recover and reconcile work | Bind publicly or treat UI memory as authority |
| Application service | Admission, idempotency, factory lease, detached execution | Invent protocol history |
| Engine | Source lifecycle, bounded harness, deterministic gates, accepted history | Treat an agent result as accepted by itself |
| Tohseno Companion | Private remote requests and human authorization | Hand DeviceKey secrets to Mac/server |
| Companion relay | Durable opaque mailbox transport | Decrypt or authorize commands |
| Registry service | Signed catalog, indexes, constrained transaction jobs | Become chain authority or a generic wallet |
| Protocol crate | Exact bytes, records, digests, signatures, reducers, conformance types | Depend on UI, Apple, RPC, harness, or global filesystem policy |
| Contracts | Builder authorization, narrow Shot witness, additive Claims receipts | Store prompts, repositories, installs, prices, or private app data |

## Three planes

**Private execution plane:** Companion, encrypted relay, Mac service, local source, coding harness, Xcode, and the physical phone.

**Public evidence plane:** Companion-approved catalog release, content-addressed source, active generation-0.8 BuilderAccount and ShotRegistry, transaction receipt, and current head.

**Additive Claim plane:** separately activated `TohsenoClaimsV1`, exact Claim action, non-transferable receipt, and Claim index.

The planes meet through exact digests and signatures, not shared mutable trust. A server can transport source and transactions but cannot impersonate a Builder. The Mac can build but cannot authorize a public Builder action. The Companion can authorize but does not execute Xcode. The chain can witness public state but cannot prove a private installation.

## One causal path

```text
request persisted
  → delivered as opaque ciphertext
  → authenticated and admitted on Mac
  → one stable execution prepared
  → bounded harness changes source
  → deterministic gates verify candidate
  → recipient-local Apple signing
  → physical install and bundle inventory observation
  → accepted history and durable receipts
```

Every arrow is a separate failure and recovery boundary. This is why Tohseno can say where work stopped without rewriting the meaning of success.
