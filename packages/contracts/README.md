# TOHSENO continuity contract harness

These version `0.1.0` JSON Schemas and golden fixtures are executable beginnings,
not final wire protocols. They characterize five distinct concepts before any
Anky production code is extracted:

- `ContinuityEvent` is a stable lifecycle fact. Its `eventId` never derives from
  mutable or replaceable artifact bytes.
- `ContinuityArtifact` is immutable sealed content, embedded or referenced. Its
  SHA-256 digest identifies exact bytes and detects mutation; it does not prove
  authorship, elapsed time, or human intent.
- `ContinuityReflection` is derived, consent- and provider-attributed, linked to
  an event and optional artifact, and independently deletable.
- `ContinuityProof` states exactly what a signer attests while excluding raw
  artifact content. The initial type does not claim independent proof of human
  action or honest duration.
- `SignedRequestEnvelopeV1` binds a suite-specific signature to the request's
  actual method, path, body hash, timestamp, nonce, and signer.

## Signed request canonical bytes

Before suite-specific signing, encode this exact LF-separated UTF-8 string with
no trailing newline:

```text
TOHSENO-SIGNED-REQUEST-V1
<UPPERCASE ACTUAL METHOD>
<ACTUAL ORIGIN-FORM PATH WITHOUT QUERY OR FRAGMENT>
sha-256:<64 lowercase hexadecimal body digest>
<RFC 3339 timestamp>
<nonce>
<signer suite>
<signer key ID>
<signer public key or suite-defined verification key>
```

The signature encoding and public-key representation remain the responsibility
of an explicitly versioned suite, but the exact represented public key is part
of the canonical signed bytes and cannot be substituted. Signer fields must be
single-line values. A verifier must first compare `method`, `path`,
and the exact body digest with the HTTP request it is authorizing, then enforce
timestamp and nonce/replay policy, then verify the suite signature. Reconstructing
fixed `POST /anky` values is never valid for another route.

## Fixture policy

All fixtures contain invented data. The corpus freezes Unicode normalization,
empty bytes, lifecycle ordering, canonical UTF-8 bytes, digest mutation, and
route-binding behavior. Implementations in any language should consume these
JSON fixtures without normalizing, repairing, or reserializing artifact bytes.

No generic production practice-identity suite, recovery protocol, sync engine,
or trustworthy-proof scheme is implemented here. Those remain versioned product
and security decisions.
