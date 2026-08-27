# ADR 0023: Studio may choose the implementation harness and model per Shot

Status: accepted

Date: 2026-08-26

Extends ADR 0016's creation surface and supersedes only ADR 0019's statement
that harness and model choice appear exclusively under Details. ADR 0019's
single implementation harness invocation, one targeted repair ceiling, shared
wall-clock budget, and private operational logs remain unchanged.

## Context

The local factory can discover several authenticated coding harnesses, and a
harness can advertise more than one model. Selecting one globally through
machine configuration makes an important cost and capability choice invisible
at the moment a new app is admitted. Showing only the eventual choice under
Details is too late for an owner who deliberately keeps multiple harnesses on
the same Mac.

The choice must not become a factory dashboard, expose credentials or routes,
or introduce another planning pass. It also must survive command recovery: a
restart cannot silently replace the harness/model pair that the owner chose.

## Decision

Studio creation has one compact **Build with** dropdown. Each option is an
installed, currently usable harness paired with one model that harness
advertises. The configured default pair is selected initially. Inference route,
authentication material, command arguments, logs, and cost internals remain
outside the creation surface.

The local service validates the submitted harness/model pair against current
machine discovery and chooses that harness's available authenticated route. It
persists the complete resolved selection in the private durable creation
command before execution. Recovery therefore uses the exact same selection.
Older clients and Companion creation may omit the choice and continue using the
configured local default.

The selector chooses the one ADR 0019 implementation harness; it does not add
another invocation, model round trip, agent role, or execution mode.

## Consequences

- The owner can make the implementation choice when creating an app without
  entering Settings or editing configuration.
- Models are always scoped to their associated harness and cannot be combined
  across adapters.
- A stale, unavailable, unauthenticated, or unsupported selection fails closed
  before a harness starts.
- Details still records the harness, model, route, usage, and outcome that
  actually ran.
- Public protocol schemas, Companion capabilities, canonical encodings, and
  accepted Shot lineage do not change.
