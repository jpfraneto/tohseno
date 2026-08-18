# Architecture decisions

Accepted ADRs are authoritative architecture decisions beneath `protocol/`.

[ADR 0016](0016-app-intent-app-on-your-iphone.md) defines the current
user-facing surface: the canonical abstraction is App → Intent → App on your
iPhone, and Studio and the Companion are thin projections over the same durable
local application service. It supersedes ADR 0015 as the description of what a
person sees, deliberately including what was deleted to get there. It does not
change ADR 0015's service, journal, capability, transport, or relay
architecture.

[ADR 0015](0015-persistent-local-factory-private-companion.md) defines the
current internal boundary: one persistent local app factory with CLI, loopback
Studio, and a private paired-companion channel. It supersedes
[ADR 0014](0014-app-version-feedback-product-boundary.md) as the description
of current `create`, `evolve`, and Studio behavior while preserving ADR 0014's
exact app-local recording format through explicit `init` and `record`
commands. Recording-only folders are not silently migrated into factory Shots.

[ADR 0011](0011-encrypted-web-to-local-intention-handoff.md) still defines the
historical web-to-local intention transport.
[ADR 0006](0006-public-witness-and-contract-generation.md) remains
authoritative for public-witness and contract-generation boundaries. None of
these ADR summaries override canonical encodings or validation rules in
`protocol/`.
