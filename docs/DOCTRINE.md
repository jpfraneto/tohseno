# TOHSENO doctrine

TOHSENO exists to make one repeated action durable. It is a product with a constrained contract, not a custom software agency or a generic app generator.

## The continuity loop

Every supported application must make this loop concrete:

```text
act → record → reflect → continue
```

- **Act:** one primary, observable action is reachable quickly.
- **Record:** progress and completion survive locally without depending on a network or model provider.
- **Reflect:** feedback helps the person recognize what happened. It may be deterministic, local, provider-assisted with consent, or absent.
- **Continue:** accumulated continuity and a respectful invitation make returning natural.

An application is not a continuity app merely because it stores records, has a daily reminder, or includes AI. The loop must center one meaningful practice.

## Product laws

### One app, one primary action

The manifest must express the core action in one sentence and define observable start, completion, and interruption conditions. Secondary capabilities may support that action; they may not compete with it.

If two actions need independent success conditions, accumulation, and return loops, they are probably two products. Do not conceal that ambiguity behind navigation or a dashboard.

### Practice before profile; action before account

Fresh use should reach the action before asking for a public identity, profile, or conventional account. Contextual cryptographic identity may be created invisibly only when the manifest requires it. Recovery should be offered after first value unless immediate recovery is necessary to avoid a demonstrated risk.

### Local and contextual identity

A practice identity is not a person's universal identity. It is scoped to an app or practice by default. Cross-device recovery, cross-app linking, content restoration, subscriptions, server ownership, and backup restoration are distinct operations with distinct consent.

### Local-first private content

The action, active checkpoint, continuity event, and user-owned artifact must remain useful without the TOHSENO control plane. Private content stays local or application-encrypted by default. Any disclosure must be named in the manifest, bound to a purpose, and presented at the consent boundary.

The control plane may know that an application is healthy, paid, deployed, or failing. It must not become a warehouse of the private content that flows through generated continuity apps.

### Determinism underneath the magic

AI may help translate human meaning into a manifest or enrich an explicitly configured reflection. It does not decide runtime storage, signing, completion, interruption, privacy, payment, or deployment behavior. Continuity must survive when the model or network is unavailable.

### Refusal is product behavior

Features outside the supported manifest are not silently accepted as bespoke work. A requested change must be representable as a valid manifest diff with updated invariants. If it cannot be represented, classify it as unsupported and explain which product boundary it crosses.

### Ownership without captivity

Applications are ejectable from birth. Source, manifest, artifacts, runbooks, environment-variable inventory, data migrations, identifiers, and ownership details must be transferable. Customers should stay because the system works, not because leaving is obstructed.

Open source does not make customer data public. Public source and private prompts, contacts, credentials, messages, and production data belong on opposite sides of a hard boundary.

## Default refusals

TOHSENO rejects these patterns unless the confirmed core action and manifest make a narrow, explicit case:

- a dashboard as the primary surface;
- a profile or account form before first value;
- a social or activity feed;
- generic CRUD around arbitrary entities;
- unrelated AI chat;
- analytics that centralize private content;
- manipulative streaks, shame, or artificial urgency;
- broad settings before the ritual;
- blockchain, payments, gates, subscriptions, or synchronization as mandatory prerequisites for local value;
- speculative modules added because generated software is expected to look “complete.”

## Feature acceptance test

Before adding a capability, answer all of these:

1. Which of `act`, `record`, `reflect`, or `continue` does it serve?
2. What valid manifest diff represents the change?
3. Does it preserve one dominant action and action-before-account?
4. What private data crosses a boundary, to whom, for how long, and with what consent?
5. Does the core local loop still work when the capability fails or is removed?
6. Who owns the resulting source, identifier, account, data, and credential?
7. How is the capability included in an ejection package?

A missing or evasive answer is a reason to refuse or defer the feature, not to expand the framework.

## Evidence over aspiration

Claims in product copy and runbooks must use the repository's real state:

- The present shell accepts, encrypts, routes, charges when a provider is configured, and releases a private agent capsule at the applicable gate.
- The manifest and contract harness establish early boundaries; they are not final universal standards.
- `READY` for the current self-hosted slice means the capsule and source contract are ready, not that native binaries exist.
- No complete native compiler, production app generator, sync engine, or generic cryptographic identity package exists yet.
