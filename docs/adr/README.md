# Architecture decisions

Accepted ADRs are authoritative architecture decisions beneath `protocol/`.

ADR 0014 defines the current product boundary: an app-local recording layer
whose `.tohseno/` directory stores Versions of the surrounding app folder.
ADR 0012's intention-led factory and ADR 0013's unattended iPhone delivery
remain historical decisions and continue to govern records made under those
flows, but they are superseded as descriptions of current `create`, `evolve`,
and Studio behavior.

ADR 0011 still defines the historical web-to-local intention transport. ADR
0006 remains authoritative for public-witness and contract-generation
boundaries. None of these ADR summaries override canonical encodings or
validation rules in `protocol/`.
