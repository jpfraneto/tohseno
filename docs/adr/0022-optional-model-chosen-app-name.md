# ADR 0022: An app name is optional; the implementation model chooses it

Status: accepted

Date: 2026-08-26

Extends ADR 0016's App → Intent → App surface and ADR 0017's single
implementation invocation. ADR 0019's shared harness budget and maximum of one
implementation plus one targeted repair remain unchanged.

## Context

The creation surface required a lowercase technical app name before a person
could describe what the app should do. That asks for packaging syntax before
the product idea exists and makes naming a separate prerequisite even when the
person wants TOHSENO to exercise product judgement.

The local factory still needs a stable, path-safe key before it can reserve a
Shot folder, bundle identifier, command receipt, and Xcode target. Asking a
model for that key in a preliminary naming call would reintroduce the planning
round trip removed by ADR 0017 and consume work outside ADR 0019's one bounded
intent-to-usable-app transition.

## Decision

The app-name input is optional on every ordinary creation door. A supplied
name remains authoritative and is normalized and validated exactly as before.

When no name is supplied, TOHSENO derives a collision-safe technical slug from
the exact intention on the Mac. The slug exists only to reserve stable local,
protocol, bundle, and build identity before materialization. It is not a
second interpretation of the intention and it is not produced by another
model invocation.

The one implementation model receives an explicit naming responsibility in
the complete harness task: infer a concise, distinctive user-facing product
name from the app's primary use, do not ask the person for another decision,
and apply that name to the experience people see on the iPhone. The technical
Xcode project, scheme, target, product, and bundle identity stay bound to the
pre-reserved slug so naming cannot invalidate protocol or build identity.

The durable private creation command records whether the person supplied the
name. Recovery preserves that fact, so a restarted service gives the model the
same naming instruction rather than silently converting an engine-derived
slug into a user decision. Older private command records default to the
historical supplied-name behavior.

## Consequences

- A person can provide only intent and press Create App.
- `tohseno create --prompt ...`, piped creation, Studio, pending-intention
  handoff, and Companion creation can all omit a name.
- Supplying a name remains available for people and scripts that need exact
  identity.
- No Conception phase, preliminary model call, naming endpoint, public schema,
  canonical encoding, contract, or deployment ceremony is introduced.
