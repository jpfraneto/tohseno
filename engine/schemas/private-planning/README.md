# Private birth-planning schemas

These schemas describe local planning and acceptance artifacts used by the
TOHSENO engine. They are deliberately outside `protocol/schemas/`: they do not
change canonical lineage serialization, v1/v2 commitments, signatures, or the
meaning of historical accepted Shots. Accepted lineage continues to
authenticate the app-specific Genome and final Version facts; the engine keeps
these richer artifacts local and binds them by digest in tasks and receipts.

The Rust types and validators in `engine/src/{birth_plan,apple_capabilities,
experience}.rs` are authoritative for current engine behavior. These JSON
Schemas make the strict harness boundary inspectable and editor-friendly.
