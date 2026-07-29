# Provenance

Every generated app embeds public facts sufficient to connect the running
artifact to its signed Shot:

- ShotID and integer Evolution sequence;
- Evolution commitment and previous commitment where applicable;
- BuilderID or public creator reference;
- `tohseno.apple/1`;
- factory implementation, version, and source commitment;
- source-tree and Fascia commitments;
- bundle identifier and `CFBundleVersion`;
- configured registry coordinates when published.

The generated resource is `TOHSENO/embedded-provenance.json`, decoded through
the closed `tohseno.app-metadata/1` model in `TohsenoMetadata.swift`. Before
typed decoding, the bounded transport parser rejects malformed JSON and
duplicate members, including keys with equivalent escaped spellings. It is
compared byte-for-byte in meaning with the signed local record. Because it
contains the Evolution commitment that commits to the source-tree hash, it is
explicitly excluded from that one hash to avoid self-reference. Its own digest
and equality are still conformance evidence.

Provenance must never contain a Builder DeviceKey private value, Keychain
persistent reference, recovery mnemonic, InstallationIdentity private
representation, private prompt, or private application content.

`Provenance.swift` verifies the embedded bundle identifier and integer bundle
version against the running bundle before presenting the facts.
