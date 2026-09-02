---
title: Source of truth
description: The authority hierarchy and direct repository sources behind these explanatory docs.
---

This site is a guide. The repository carries the authority.

## 1. Normative protocol

- [`protocol/SPECIFICATION.md`](https://github.com/jpfraneto/tohseno/blob/main/protocol/SPECIFICATION.md) — exact identities, encodings, commitments and transitions.
- [`protocol/CONFORMANCE.md`](https://github.com/jpfraneto/tohseno/blob/main/protocol/CONFORMANCE.md) — required fail-closed checks.
- [`protocol/IMPLEMENTERS.md`](https://github.com/jpfraneto/tohseno/blob/main/protocol/IMPLEMENTERS.md) — lifecycle integration.
- [`protocol/schemas/`](https://github.com/jpfraneto/tohseno/tree/main/protocol/schemas) — closed Draft 2020-12 schemas.
- [`protocol/test-vectors/`](https://github.com/jpfraneto/tohseno/tree/main/protocol/test-vectors) — frozen cross-language bytes.

## 2. Accepted decisions

The [ADR index](https://github.com/jpfraneto/tohseno/tree/main/docs/adr) records decisions and supersession. The current product arc is:

- 0015: persistent local factory and private Companion boundary.
- 0016: App → Intent → App on iPhone; deletion of the Studio dashboard.
- 0017: engine composes/accepts Genome; no Conception round trip.
- 0019: one bounded implementation plus at most one repair.
- 0024: integral `.tohseno/` with explicit private exclusions.
- 0025: native Mac app is primary; optional managed balance.
- 0026: Return sends, truthful Registry, fail-closed installer.
- 0027: Build/App/Source workspace and permanent phone stage.
- 0028–0031: Finder-first handoff, first-shot history, direct download, release-candidate acceptance.
- 0032: real Companion onboarding and persistent product presence.
- 0033: living existing projects become the primary path.
- 0034: person-to-person signed buildable native software.
- 0035: one Ship, later Updates, immutable edition and additive non-transferable Claim.

Later decisions supersede only the parts they say they supersede. They do not silently rewrite frozen protocol or deployed ABI.

## 3. Current implementation truth

- [`docs/STATE.md`](https://github.com/jpfraneto/tohseno/blob/main/docs/STATE.md) — plain-language shipped/inactive/deferred snapshot.
- [`docs/ARCHITECTURE.md`](https://github.com/jpfraneto/tohseno/blob/main/docs/ARCHITECTURE.md) — runtime components and persistence.
- [`docs/LIVING_CONNECTION.md`](https://github.com/jpfraneto/tohseno/blob/main/docs/LIVING_CONNECTION.md) — adoption, private request, Ship/Claim/receive acceptance.
- [`docs/GOLDEN_PATH.md`](https://github.com/jpfraneto/tohseno/blob/main/docs/GOLDEN_PATH.md) — boundary-by-boundary private command trace.
- [`docs/PRIVACY.md`](https://github.com/jpfraneto/tohseno/blob/main/docs/PRIVACY.md) and [`docs/THREAT_MODEL.md`](https://github.com/jpfraneto/tohseno/blob/main/docs/THREAT_MODEL.md) — privacy and controls.
- [`release/`](https://github.com/jpfraneto/tohseno/tree/main/release) — exact immutable activation and release evidence.

## Historical material

`MASTER_PROMPT.md` is the historical constitutional center of frozen v0.7 and is superseded as current deployment/protocol authority. `genome/LAWS.md` is compatibility law matching engine behavior, not ordinary prose. Historical release and readiness files describe their recorded moment; they are not automatically current.

When something here disagrees with a higher layer, follow the higher layer and fix this guide.
