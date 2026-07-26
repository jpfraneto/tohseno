# Public Shot protocol

Protocol v1 is a closed, signed, hash-linked public record format. A stable
random Shot ID is allocated independently of private intent. The only
lifecycle values are `EVOLVING`, `PUBLISHED`, and `APP_STORE`; Evolutions keep
the same ID in every state.

Record hashes are computed over canonical JSON for the complete signed record
and are not embedded in that record. The next record puts the prior hash in
`previousRecordHash`. Signing uses canonical unsigned-record bytes under a
protocol domain, while the signer package separately binds its identity and
verification material.

Only deliberately public names, summaries, identities, distribution evidence,
and external linkage evidence have wire fields. Unknown fields are rejected.
There are no fields for prompts, private provenance, secrets, source bytes,
model reasoning, conversations, or generated-app user content. A public
summary can still disclose an idea; submission therefore remains an explicit
owner action.

The JSON Schemas define the closed portable shapes. The executable validators
are also required: they enforce cross-field identity equality, exact calendar
timestamps and URL semantics, canonical encodings, signature validity, and
history-dependent registry rules that JSON Schema cannot express alone.

Appcoin records are inert, generic external links. They do not deploy, mint,
trade, endorse, or grant authority over an asset.

The tracked signed fixture contains only a public key and signature from an
ephemeral key that was discarded immediately. No private-key fixture exists.
