# TOHSENO Constitution — GENESIS Candidate

**Version:** `1.0.0-rc.1`
**Codename:** `GENESIS`
**Status:** protocol candidate; not the canonical release

This file is the constitutional center of the TOHSENO repository. Normative
byte encodings, validation rules, and conformance requirements live in
`protocol/`; the Apple connective structure lives in `fascia/apple/`; deployed
contract coordinates, when they exist, live in signed candidate deployment
records. Prose never overrides those executable artifacts.

The previous local-factory constitution is preserved verbatim at
`history/MASTER_PROMPT-v0.6.0.md`.

## Declaration

> TOHSENO is an open protocol for creating and evolving Apple apps from a
> shared, machine-verifiable structure.

> A Shot is one app, its permanent identity, and its signed history.

> Every Shot belongs to its builder. Any compatible factory can understand and
> continue it.

> One canonical rulebook. No canonical doorway.

The application, CLI, Studio, node, website, and services made by JP and Anky,
Inc. are the first entry point. They are not privileged by the protocol.

## Canonical surfaces

Only these artifacts may define TOHSENO compatibility:

1. The protocol specification: ownership, signed actions, lineage, and public
   records.
2. The TOHSENO Apple Fascia: the finite connective anatomy shared by compatible
   generated applications.
3. Versioned schemas: exact fields, types, encodings, hashes, signatures, and
   allowed values.
4. A specifically identified, non-upgradeable registry deployment: a neutral
   public witness, never the creator of a Shot.
5. Conformance tests: deterministic checks that do not call generative
   intelligence.

No client, factory, server, relayer, node, company, token, app, or domain is
canonical.

## Identity law

Four identities remain separate:

- A **BuilderID** is the durable controller of Shots. In this candidate it is a
  deterministic smart-account address expressed as
  `eip155:4663:0x<address>`.
- A **Builder DeviceKey** is one replaceable P-256 signing authority held by a
  physical device. It is an authorized signer, not the builder's permanent
  identity.
- A **Recovery Root** is optional, separate from ordinary signing, and used
  only to recover BuilderID control. A BIP-39 recovery mnemonic derives a
  secp256k1 Ethereum authority at the documented BIP-44 path. Recovery material
  is never placed in a generated application.
- An **InstallationKey** is an app-specific P-256 identity created by one
  installation of one generated application. It does not identify the builder,
  a person across applications, an Apple ID, or a TOHSENO account.

No raw private key or mnemonic may be logged, embedded in a Shot, sent to a
server, or moved between devices. Hardware-backed Apple key storage is used
where available. Software-backed keys are visibly test-only.

Apple signing readiness is an Apple distribution gate. It is not protocol
identity.

## Shot law

A Shot has one cryptographically random 32-byte ShotID. The ShotID is stable
across every Evolution.

Each Evolution is one complete, append-only source world:

```text
Evolution sequence = filesystem Shot number = CFBundleVersion
```

Evolution 1 has no previous commitment. Evolution N points to the canonical
commitment of Evolution N-1. Failed or incomplete directories are not accepted
Evolutions and never become the recognized head.

A complete Evolution contains:

- the user's local intention and references, kept private by default;
- a complete Apple project;
- the Apple Fascia;
- a canonical Shot record;
- a detached P-256 signature;
- deterministic conformance evidence;
- build and installation evidence appropriate to the claimed state.

The Evolution commitment is SHA-256 over the RFC 8785 canonical bytes of the
closed Shot record. The signature sidecar is not part of the signed record and
therefore cannot make the commitment self-referential.

Finalized Evolution directories are immutable. Legacy adoption creates a new,
honest Evolution and never fabricates signatures for old work.

## Genome and Fascia

The **Genome** guides generative intelligence. It may contain prose, taste,
examples, and open-ended product requirements.

The **Fascia** is deterministic compatibility law. It defines required files,
interfaces, installation identity, local-first storage, privacy, consentful
continuity, provenance, distribution metadata, capabilities, and conformance
gates.

Intelligence may create any purpose or appearance. A simple program decides
whether the generated result kept the Fascia's finite promises.

Generated applications use SwiftUI and Apple frameworks with no third-party
runtime dependencies by default. They open to useful behavior without an
account wall. Storage is local first. Network access and sensitive Apple
capabilities are declared exactly; undeclared sensitive capability use fails
conformance.

## Consentful continuity

Installations are unlinkable by default. Two installations may establish only
a narrow relationship chosen by the user through an expiring, nonced, signed,
transport-neutral continuity envelope.

The envelope identifies its issuer, intended audience, originating Shot,
explicit scope, nonce, and expiration. It contains no universal human
identifier, global app graph, shared private key, or unrelated local data.

No TOHSENO or Anky identity server is required to create or verify continuity.

## Privacy and publication

Private by default:

- prompts and reference images;
- local application data and usage;
- unpublished source and Shots;
- continuity links;
- device inventory and recovery material.

Only an explicit signed action may publish:

- an Evolution commitment and public lineage head;
- its controller and publication state;
- a builder-selected public URI;
- a handle, appcoin relation, or App Store attestation.

The registry witnesses a Shot that already exists. It does not create the Shot
and does not receive private content.

## Signatures and authority

The protocol keeps four judgments distinct:

```text
Cryptography: this key signed this digest.
Schema: this digest means this exact action.
BuilderID: this key is authorized to act.
Contract: this public transition is therefore valid.
```

Local and public verifiers must reproduce the same judgment. P-256 signatures
use fixed-width components and require low `s`. Public action digests include
the chain ID, verifying contract, action type, replay protection, and deadline
where specified.

## Neutral public witness

The candidate registry is non-upgradeable and has no founder, official-client,
factory, node, relayer, or company privilege. Anyone may relay a valid signed
action. Relaying never grants ownership.

The core registry stores only controller, current head, integer sequence,
public state, and optional public content commitment. Human-readable handles,
appcoin associations, and App Store attestations live in a separate relations
contract.

`$TOHSENO` is optional. It is never required to create, own, verify, publish,
evolve, transfer, or use a Shot. Its coordinates are never guessed.

## Replaceability

Local creation, evolution, signing, verification, installation, and use do not
depend on an Anky, Inc. server. Publication may use any relayer. A node indexes
and mirrors public facts; it does not own them.

Another implementation is compatible only if it can consume the normative
schemas and vectors, reproduce commitments, verify signatures and lineage, and
pass conformance without importing the TOHSENO engine or Studio.

## Product experience

Protocol complexity stays below the ordinary loop:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
tohseno create my-app
```

The user describes an app. TOHSENO creates it, repairs it until it builds,
installs it, and records a valid Evolution. Identity appears as infrastructure,
not homework. The engine keeps its four voices: `Status`, `Handoff`, `Result`,
and `HarnessLine`.

## Candidate release discipline

This repository may build and exercise `1.0.0-rc.1` against Robinhood Chain
mainnet. Candidate addresses are experimental and must be accompanied by exact
source, bytecode, transaction, probe, and lifecycle evidence.

This candidate must not:

- tag or claim `v1.0.0`;
- declare any deployment immutable or canonical;
- replace the stable installer or public protocol page;
- update DEX Screener or announce the protocol;
- publish the final permanent Arweave Genesis Bundle;
- open an unrestricted public gas relayer;
- claim an unexecuted build, installation, deployment, or verification.

The release candidate is complete only when the closed loop has been exercised:

```text
intention
→ app
→ Fascia
→ signed Shot
→ deterministic verification
→ public witness
→ Evolution
→ independent verification
```

Until then, reports distinguish `implemented`, `automatically verified`,
`manually observed`, `deployed`, and `not completed`.
