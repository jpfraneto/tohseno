# Post-activation product lifecycle gap audit

Status: preparatory implementation audit; not protocol authority, activation,
publication evidence, or permission to change a frozen format.

This audit answers whether deploying and threshold-activating generation 0.8.0
would, by itself, make the requested creation → evolution → publication →
discovery → feedback → Bankr lifecycle operational. It would not. Contract
activation is necessary for public authority, but multiple client and transport
surfaces are intentionally absent under current accepted decisions.

## Already implemented

- The private local Shot lifecycle supports coherent intention, accepted Genome,
  Apple expression, build, accepted Version, exact-version private feedback,
  selected evolutionary intent, Evolution, verification, export and import.
- Protocol generation 0.8.0 defines the immutable build, release-authority
  policy, activation envelope, registry v2 actions, registration commitment and
  ancestry-free public checkpoint.
- The node has bounded static-peer synchronization for ordinary signed lineage,
  but treats candidate authority as unresolved.
- Studio's optional Bankr surface binds one launch to one selected verified Shot,
  accepts a session-only user API key and explicit recipient, previews HTTPS
  images, simulates first, and requires single-use confirmation.

## Gap 1 — no activated-generation resolver

`engine/src/contract_generation.rs` has only the `Inactive` state. It embeds the
0.8.0 definition, sets both the trusted policy digest and signed activation head
to `None`, and returns `false` from both public-authority gates. There is no
release-manifest loader that pins an owner-approved policy, verifies a signed
activation chain, checks canonical on-chain evidence, and resolves an active
generation.

The same absence is projected deliberately:

- CLI protocol status reports `active_generation: null`;
- Studio reports publication disabled and has no registry RPC/relayer path;
- generated public pages report no active generation; and
- nodes report `active_generation: null`.

Required work after an accepted trust-root decision: define the immutable
release-manifest location and update path, verify the complete activation chain
under the pinned digest, bind canonical target-chain evidence, expose explicit
active/inactive resolution, and keep every failure mode fail-closed. Tests must
prove that build definitions, deployments, untrusted policies and otherwise
valid activations under the wrong policy never activate clients.

## Gap 2 — app metadata `/3` and successor Apple Fascia do not exist

ADR 0007 forbids reinterpreting the shipped optional `/2` registry coordinates
as publication proof. `engine/src/app_metadata_policy.rs` therefore accepts v2
metadata only when registry evidence is absent under the current inactive
generation. There is no `tohseno.app-metadata/3` schema, Rust type, canonical
test vector, engine generator/verifier, or versioned successor Apple Fascia.

ADR 0007 requires the successor receipt to bind at least:

- contract-generation definition and trusted activation;
- chain ID and registry address;
- Shot ID and exact public checkpoint/action;
- transaction and canonical block evidence; and
- an explicit evidence state distinguishing declared, observed,
  cryptographically verified and unavailable.

Required authority: an accepted additive protocol/ADR design for `/3` and the
successor Fascia. The frozen `/2` schema, decoder, fixture and sealed v0.7 Fascia
must not be mutated. Implementation then needs cross-language fixtures,
negative tests, deterministic receipt verification and materialization gates.

## Gap 3 — no public Builder or registry transaction workflow

The product has no active path to create a secure hardware-backed Builder
identity under an activated generation, deploy its BuilderAccount, construct
and persist a registration salt, submit a permissionless commitment, wait the
inclusive maturity window, produce the live ERC-1271 reveal authorization,
relay the reveal, append checkpoints, transfer control, or verify receipts.

Required work after activation and publication-receipt authority: implement
explicit RPC/relayer configuration, Secure Enclave signing with exact EIP-712
domains, durable pending-operation state, bounded fee display, confirmation,
one-shot submission, ambiguous-response recovery, canonical receipt/event/state
verification, and privacy tests proving no private lineage or guessable secret
enters the checkpoint.

## Gap 4 — nodes do not inventory public checkpoints or receipts

`node/README.md` explicitly says this node revision does not inventory public
checkpoint records or receipts. The node stores and synchronizes bounded
ordinary lineage evidence, reports candidate authority as unresolved, and does
not treat a successful `POST /v1/actions` as publication.

Required work: add closed checkpoint/receipt inventory types only after their
protocol formats are accepted; verify them against the trusted activation and
canonical registry evidence; advertise them without implying artifact
availability or global consensus; synchronize them between two explicitly
configured peers; and prove that peer-supplied labels cannot influence local
verification.

## Gap 5 — bounded remote feedback does not exist

Current feedback is local/private and bound to an exact accepted Version. There
is no node or public service endpoint for remote feedback, spam/authentication
policy, rate and body limits, sender evidence model, moderation/retention law,
or owner-controlled import and selection path.

Required authority: an explicit privacy and abuse model. Required
implementation: a bounded transport whose receipt never authorizes a Shot
transition; quarantine by default; exact Shot/Expression/Version binding;
owner-controlled review, acceptance and deletion; attachment policy; and tests
for replay, spam, impersonation, malicious content and unavailable versions.

## Gap 6 — Bankr remains a private optional relation

The Bankr launch surface can operate independently for a selected local Shot,
but its Token Association is intentionally private. ADR 0006 says it cannot
become Shot identity, ownership or a registry relationship until a separate
closed ancestry-free public relation format is accepted. A token launch cannot
stand in for publication, discovery, active generation or controller proof.

## Required product sequence

1. Recover and complete both audits; disposition every finding.
2. Establish the owner-approved release trust root.
3. Authorize and perform the exact inactive deployment.
4. Complete the real-time production canary and threshold activation.
5. Accept an additive `/3` publication-receipt and successor-Fascia design.
6. Implement and test activated-generation resolution and public transaction
   orchestration.
7. Implement receipt-aware node inventory and prove two-node discovery.
8. Define and implement bounded remote feedback, then prove owner-controlled
   acceptance into a later Evolution.
9. Exercise the optional Bankr lifecycle without conflating it with any prior
   step.

Until these gaps close, the private creation/evolution lifecycle is real, the
contract generation is an inactive definition, the node is a neutral lineage
replicator, and Bankr is an optional private Shot relationship. No narrower
test or deployment fact proves the complete public lifecycle.
