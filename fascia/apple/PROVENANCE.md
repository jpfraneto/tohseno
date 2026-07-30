# Provenance

Every generated app embeds public facts sufficient to connect the running
artifact to its signed Shot and exact expression state:

- ShotID, ExpressionID, and VersionID for new lineage;
- accepted genome revision and digest;
- lineage sequence and head;
- BuilderID or public creator reference;
- `tohseno.apple/1`;
- factory implementation, version, and source commitment;
- source-tree, build when available, and Fascia commitments;
- bundle identifier and `CFBundleVersion`;
- configured registry coordinates when published.

The generated resource is `TOHSENO/embedded-provenance.json`, decoded through
exact schema dispatch in `TohsenoMetadata.swift`. Historical expressions keep
the frozen closed `tohseno.app-metadata/1` shape. New expressions use the
distinct closed `tohseno.app-metadata/2` shape. Unknown versions and hybrid
objects fail closed; v1 never gains permissive optional fields.

Before typed decoding, the bounded transport parser rejects malformed JSON and
duplicate members, including keys with equivalent escaped spellings. The
resource is compared in meaning with the signed local records. Because it
contains commitments to the source state and lineage, this one established
filename remains explicitly excluded from the v1 source-tree hash to avoid
self-reference. No second excluded identity file is introduced. Its own
digest, exact bundle inclusion, and semantic equality remain conformance
evidence.

Provenance must never contain a Builder DeviceKey private value, Keychain
persistent reference, recovery mnemonic, InstallationIdentity private
representation, private prompt, or private application content.

`Provenance.swift` verifies the embedded bundle identifier and integer bundle
version against the running bundle before presenting either version's facts.
New applications can read `expressionID`, `versionID`, `genomeRevision`, and
`genomeDigest`; those values are absent, not invented, for v1 history.
