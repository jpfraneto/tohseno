# ADR 0007: app metadata does not self-prove publication

- Status: accepted for generation `0.8.0`; still applies after activation
- Date: 2026-07-30

## Context

`tohseno.app-metadata/2` contains an optional `registry` object with a Robinhood
chain ID, contract address, and optional transaction hash. Those coordinates
do not prove that the named contract is an activated TOHSENO registry, that
the transaction registered this Shot, that the controller was authorized, or
that the observed chain state is canonical.

No contract generation is active. An arbitrary nonzero address and transaction
therefore cannot turn an embedded app claim into publication evidence.

The permissive `/2` shape cannot be narrowed in place. It shipped in v0.7.1,
along with its Swift decoder and fixture:

- `app-metadata-v2.schema.json`:
  `674ee6f9690806d20afa07f8af08a7b0b7ec312674927ad50762ee1a9a3d776f`;
- `TohsenoMetadata.swift`:
  `3b1ddcaec28108a58857b267b026dace3263048bed9db0da2840c7d9a82076d6`;
- `app-metadata-v2.json`:
  `309585d1db2c98d777a74343b6c13229ff8dd3a502fc0947a60eb8da1e36502c`.

Those v0.7.1 bytes remain frozen compatibility material.

## Decision

Generic protocol decoding continues to accept the shipped `/2` registry
shape for offline inspection. Acceptance is not trust.

Current engine policy applies an additional fail-closed rule to every v2
generation, materialization, Shot-body verification, and retained-artifact
verification path:

```text
active contract generation = none
=> app-metadata/2 registry MUST be null or absent
=> app-metadata/2 cannot establish publication
```

When projecting a frozen v1 record into newly generated v2 metadata, the
engine deliberately drops any v1 registry reference before writing the
embedded resource. A received v2 object with a non-null registry may still be
decoded as historical compatibility data, but current engine policy refuses
to generate, accept, or verify it.

The sealed v0.7 Apple Fascia and its Swift `Provenance.isPublished` behavior
remain unchanged. They are historical decoders, not current registry
verification authority. Changing them in place would mutate the Fascia
commitment of existing artifacts.

## Successor rule

Activated publication evidence requires both:

1. a new app-metadata schema (at least `/3`) containing a closed,
   generation-scoped receipt rather than bare coordinates; and
2. a versioned successor Apple Fascia whose decoder validates that receipt
   against a client-trusted activation and the exact Shot registry action.

The successor receipt must bind the contract-generation definition and
activation, chain ID, registry address, Shot ID, exact public checkpoint or
action, and sufficient transaction/block evidence for deterministic
verification. Its design must distinguish declared, locally observed,
cryptographically verified, and unavailable evidence. It must not reinterpret
the optional `/2` registry object as that receipt.

## Consequences

- v0.7.1 metadata, schema, fixture, and Swift bytes remain verifiable.
- A forged chain-4663 address or transaction cannot make current v2 metadata
  pass engine verification or become an accepted Version.
- Contract activation alone will not silently change `/2` semantics.
- Publication support remains explicitly deferred until `/3` and a successor
  Fascia are reviewed together.
