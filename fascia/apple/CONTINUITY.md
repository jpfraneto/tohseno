# Consentful continuity

Continuity is an explicit, narrow relationship between installations. Before a
continuity envelope is issued, independently generated apps have no common
identifier by which they can correlate a person.

The transport object uses schema `tohseno.continuity/1`. Its `statement` uses
`tohseno.continuity-statement/1` and contains:

- issuer installation identifier and P-256 public key;
- intended audience ShotID and optional recipient InstallationID;
- originating ShotID;
- 1 to 16 unique scope-token claims, strictly sorted in ASCII lexicographic
  order and matching `[a-z0-9]+(?:[._-][a-z0-9]+)*`;
- a cryptographically random 32-byte nonce;
- integer Unix issue and expiration times within the interoperable JSON safe
  integer range;
- SHA-256 digest and low-s P-256 signature.

It contains no BuilderID unless a separately specified claim truly requires
it, no universal human identifier, no app inventory, and no unrelated
continuity relationship.

## Signing bytes

The digest is SHA-256 of the RFC 8785 canonical JSON bytes of the complete
`statement` object. The P-256 signature signs that 32-byte prehash exactly; it
must not hash the digest a second time. `r` and `s` are fixed-width big-endian
32-byte values and `s` is low.

The closed statement shape uses only lowercase schema strings, `0x`-prefixed
fixed-width identifiers, sorted lowercase scope-token claims, and safe integer
timestamps. `ContinuityEnvelope.swift` emits the exact RFC 8785 bytes for that
restricted shape and includes fixtures in its tests. Its bounded transport
preflight rejects malformed JSON and duplicate object members before typed
decoding, including escaped spellings of an already-seen key.

InstallationID is SHA-256 of
`UTF-8("TOHSENO-INSTALLATION-ID-V1") || 0x00 || x32 || y32`.

QR, file, and deep-link encodings are transports. None is an authority, and no
official TOHSENO application or server is required to issue or verify the
envelope.
