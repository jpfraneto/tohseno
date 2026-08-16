# Architecture decisions

Accepted ADRs are authoritative architecture decisions beneath `protocol/`.

[ADR 0015](0015-persistent-local-factory-private-companion.md) defines the
current product boundary: one persistent local app factory with CLI, loopback
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
