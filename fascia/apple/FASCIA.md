# TOHSENO Apple Fascia 1

The TOHSENO Apple Fascia is the finite connective structure shared by every
compatible TOHSENO Apple app. It is compatibility law, not a prompt for a
coding model. `FASCIA.json` is the machine-readable authority for required
files, interfaces, capabilities, and conformance gates; these documents state
their meaning.

That authority is a Fascia *definition* with schema
`tohseno.apple-fascia-definition/1`, validated by `FASCIA.schema.json`. It is
not the per-Shot `TOHSENO/fascia.json` sidecar. Each concrete Shot sidecar uses
the distinct `tohseno.fascia/1` schema and records the Fascia identifier and
commitment applied to that Shot.

The Fascia does not standardize an app’s purpose or visual expression. It
standardizes only the organs needed for portable identity, local-first storage,
consentful continuity, provenance, privacy inspection, and distribution.

## Required shape

A generated Shot contains protocol sidecars and these documents under
`TOHSENO/`. Its application target contains the five sources under
`TohsenoFascia/`. The paths may be adapted to an Xcode project’s established
layout, but their meaning and target membership must be discoverable
deterministically.

The application must:

- be a standard SwiftUI application targeting iOS 17 or newer;
- reach a useful screen without an account wall or mandatory onboarding;
- have no third-party runtime dependencies by default;
- prepare its app-specific `InstallationIdentity` during first launch;
- keep storage local first and make CloudKit opt-in;
- declare every network use, protected API, and entitlement;
- embed public Shot provenance without embedding signing or recovery secrets;
- keep `CFBundleVersion` equal to the integer Evolution sequence.

## Deterministic enforcement

Conformance is performed by a simple verifier. It checks files, Xcode target
membership, project settings, imports and package references, Info.plist,
entitlements, capability declarations, embedded metadata, signed sidecars,
source commitments, and build output. It never asks a language model whether
the app complied.

`TOHSENO/embedded-provenance.json` is generated after the source-tree
commitment is known and is the single explicit source-tree exclusion needed to
avoid a self-referential commitment. The verifier still hashes and compares
that file independently against `shot.json`; it is excluded from only the
source-tree commitment, not from conformance.

The Fascia commitment follows the length-prefixed tree rule in `FASCIA.json`.
For every recursively enumerated regular file, it encodes
`u64be(path_length) || path_bytes || u64be(content_length) || raw_content`,
sorts entries by unsigned normalized path bytes, concatenates those entries
without a domain prefix or file count, and hashes the resulting stream with
SHA-256. The empty tree therefore hashes as SHA-256 of the empty byte string.

Exclusions are root-relative. An exclusion `E` matches exactly `E` and every
path beginning with `E + "/"`; it never acts as an unanchored basename match.
The verifier does not follow symbolic links, and any symbolic link or
non-regular included filesystem entry is a conformance failure. Build
products, `.swiftpm`, `Package.resolved`, and `.build` are excluded. All other
normative Fascia files are included.

This is the `1.0.0-rc.1` GENESIS protocol candidate. It is not yet a canonical
shipped Fascia.
