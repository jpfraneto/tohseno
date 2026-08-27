# Contract 0.8 release-authority runbook

Status: proposed operational design; not an accepted trust root and not
deployment authority.

Current-state note (2026-08-26): the owner later approved the concrete policy
and generation 0.8.0 was activated. The authoritative evidence is in
`release/contract-activations/`; proposed and pre-activation language below is
historical and grants no authority to repeat the ceremony.

This runbook applies the closed records in `protocol/` and ADR 0006. If it ever
disagrees with either source, the protocol and accepted ADR win. No private key,
mnemonic, seed, keystore, recovery phrase, or password belongs in this
repository or in ceremony evidence.

## Proposed production policy

Production should use three dedicated offline P-256 release keys with a
threshold of two. Each key has exactly one purpose:
`contract_generation_activation`. Release keys are not Builder DeviceKeys,
installation keys, Apple signing keys, transaction payers, recovery keys, or
ordinary workstation credentials.

The owner must explicitly accept this 2-of-3 design and the final canonical
policy digest before any client pins it. Until then, no policy instance or
public key is committed and every client must continue to resolve
`active_generation: null`.

## Custody

Each key is generated on a different offline device from fresh device entropy.
Each custodian records only the public P-256 coordinates and derived release
key ID on the ceremony workstation. The private key stays on its originating
offline device and is never exported to the repository, shell history,
environment, clipboard synchronization, screenshots, prompts, logs, CI, or a
shared password manager.

The three devices and their encrypted offline backups must occupy separate
failure domains. The policy record orders authorities by the raw byte order of
their derived key IDs, not by custodian name or creation order. Custodian names
and physical locations belong in a private owner-controlled inventory, not the
public policy.

## Policy ceremony

1. Start from a reviewed, clean ceremony workstation and record its exact
   source commit and tool versions.
2. Generate each P-256 key offline and independently display its public `x`
   and `y` coordinates twice.
3. On two independent verifier implementations, prove curve membership and
   derive
   `SHA-256("TOHSENO-RELEASE-AUTHORITY-KEY-V1\0" || x32 || y32)`.
4. Compare the three key IDs out of band, reject duplicates, and sort them
   strictly.
5. Construct the closed `tohseno.release-authority-policy/1` record with
   protocol major 2, purpose `contract_generation_activation`, threshold 2,
   the three ordered public authorities, and one canonical UTC issuance time.
6. Validate the JSON Schema and Rust semantic verifier; independently
   canonicalize the record and reproduce its RFC 8785/SHA-256 digest.
7. Have the owner explicitly approve the exact policy digest before any client
   or release configuration pins it.
8. Publish only the policy, its digest, public verification transcripts, and
   owner approval evidence.

The current Rust fixture drill is exercised by:

```sh
cargo test --locked -p tohseno-protocol --test contract_activation
```

That test proves threshold, ordering, uniqueness, domain separation, low-s
signatures, unknown-key refusal, replay refusal, generation binding, and
successor causality with test-only keys. It is not a production key ceremony.

After the owner accepts this design and the three offline custodians have
independently generated their keys, the ceremony workstation can construct the
candidate public policy without accessing private material. Each custodian
supplies only one `{ "x": "0x…", "y": "0x…" }` object in a closed input:

```json
{
  "schema": "tohseno.release-authority-public-keys/1",
  "keys": [
    { "x": "0x<32-byte-lowercase-x>", "y": "0x<32-byte-lowercase-y>" },
    { "x": "0x<32-byte-lowercase-x>", "y": "0x<32-byte-lowercase-y>" },
    { "x": "0x<32-byte-lowercase-x>", "y": "0x<32-byte-lowercase-y>" }
  ]
}
```

Construct the exact 2-of-3 policy at an agreed canonical UTC issuance time:

```sh
scripts/prepare-release-authority-policy.py \
  --public-keys /absolute/path/to/public-keys.json \
  --issued-at YYYY-MM-DDTHH:MM:SSZ \
  --output /absolute/path/to/candidate-release-policy.json
```

The preparer independently checks P-256 curve membership, derives and sorts
the dedicated key IDs, refuses duplicate keys and existing outputs, writes only
the public policy, and reports its RFC 8785/SHA-256 digest. It has no private-key,
signature, network, Keychain, or trust-root installation path. A separate Rust
implementation must reproduce the digest:

```sh
cargo run --quiet --locked -p tohseno-protocol \
  --example verify_release_authority_policy -- \
  /absolute/path/to/candidate-release-policy.json
```

The cross-implementation drill is exercised by:

```sh
./scripts/tests/test-prepare-release-authority-policy.sh
```

These tools are prepared now, but must not be run with production keys before
the owner's explicit design acceptance. A matching digest still does not
approve the policy or install it as a client trust root.

The separate offline implementation is exercised by:

```sh
./scripts/tests/test-verify-contract-activation.sh
```

`scripts/verify-contract-activation.py` shares no Rust protocol or cryptography
code. It strictly rejects duplicate JSON and floating-point values, independently
reproduces the applicable RFC 8785/SHA-256 digests and source inventory, requires
an explicit trusted policy digest and exact P-256 evidence file, and asks OpenSSL
to validate each P-256 public key and low-s prehash signature. It also uses
OpenSSL Keccak-256 to bind portable creation bytecode and independently
recompute both CREATE2 addresses. Its negative suite rejects the wrong trust
root, insufficient and reordered approvals, high-s signatures, runtime
tampering, changed probe evidence, invalid CREATE2 coordinates, malformed
predecessor activations, and duplicate members.
Independent review of this verifier remains a production trust-root gate; its
existence does not authorize the proposed policy digest.

The final public-evidence invocation is explicit about every input:

```sh
python3 scripts/verify-contract-activation.py \
  --repository-root "$PWD" \
  --generation contracts/generations/0.8.0/generation.json \
  --policy /absolute/path/to/public-release-policy.json \
  --signed-activation /absolute/path/to/signed-activation.json \
  --p256-probe /absolute/path/to/ceremony-p256-evidence.json \
  --trusted-policy-sha256 0x<owner-approved-policy-digest>
```

The verifier emits one closed report to standard output and emits no policy or
activation file. A matching command-line digest proves only that the operator
supplied that trust root; the owner's separately preserved approval is still
required.

## Activation signing ceremony

Activation begins only after the immutable candidate is deployed inactive and
the complete three-day production canary has passed. The coordinator constructs
one canonical `tohseno.contract-activation/1` record binding:

- generation `0.8.0` and its exact definition digest;
- the approved policy digest and chain ID 4663;
- observed factory, registry, and BuilderAccount runtime hashes;
- both deployment transactions and canonical blocks;
- an activation block at or after both deployments; and
- the digest of the fresh, ceremony-bound Robinhood EIP-7951 evidence.

ADR 0010 governs runtime-hash interpretation. The generation's runtime fields
are compiler deployed-bytecode templates. The factory has no immutable
references and its observed hash must equal its template. BuilderAccount and
ShotRegistry contain constructor-patched immutables: custodians must reproduce
their exact instantiated bytes from the generation-bound creation inputs and
approve the observed instance hashes, never substitute zero-placeholder
template hashes. The deployed registry instance hash is
`0xfc80358a8f52ac8ae96691fecb611ade47447df79274e70defdd37989f3ca5e0`;
the canary must independently establish the production BuilderAccount instance
hash before signing.

Two offline custodians independently inspect those facts and the exact
domain-separated activation digest. Each signs that digest once with low-s
P-256 and returns only the detached public approval. The coordinator sorts
approvals by key ID, validates them under the policy, and publishes the signed
envelope. A second implementation repeats canonicalization and signature
verification before client configuration changes.

Valid threshold signatures prove approval under the supplied policy only. A
client trusts the activation only when its separately authorized trust root
already pins the same policy digest. Deployment, a transaction-payer signature,
or a Builder signature never supplies that trust.

## Ceremony tooling

Every step above now has a non-authorizing tool, drilled end to end with test
keys by `scripts/tests/test-activation-ceremony-tools.sh`:

- `scripts/generate-release-authority-key.py` — one custodian key on one
  offline device; prints only public coordinates and the derived key ID.
- `scripts/prepare-release-authority-policy.py` — the 2-of-3 policy from
  public keys only.
- `scripts/prepare-contract-activation.py` — the canonical activation record
  from the generation, approved policy, published deployment evidence, fresh
  probe evidence, canary-established BuilderAccount instance hash, and a
  fresh activation block; refuses ADR 0010 template-hash substitution.
- `scripts/sign-contract-activation.py` — one low-s P-256 approval on the
  key's own device; recomputes the digest from the inspected record and
  refuses non-canonical input.
- `scripts/assemble-signed-activation.py` — threshold-checked envelope
  assembly with per-approval OpenSSL verification.
- `scripts/verify-contract-activation.py` and
  `cargo run -p tohseno-protocol --example verify_signed_contract_activation`
  — the two independent verifier implementations run before any client
  configuration change.

Tool existence authorizes nothing; the hard stops below still hold until each
is explicitly cleared.

## Loss, rotation, incident, and successor rules

- One lost or unavailable key leaves a 2-of-3 policy usable; record the event
  privately and schedule a successor-policy ceremony before another loss.
- Two unavailable keys make the policy unable to activate anything. Do not
  lower the threshold or substitute another key. Establish a new policy only
  through a new explicit owner trust-root decision.
- Suspected key compromise freezes activation work. Preserve public evidence,
  abandon any unsigned or partially signed envelope, and establish a new
  policy with newly generated keys.
- A signature over the wrong digest or unexpected facts is an incident. Do not
  collect a replacement signature for the same candidate until the discrepancy
  is explained and recorded.
- A contract defect creates a new semantic generation. Immutable deployed code
  is abandoned rather than patched, proxied, paused, or administratively
  rewritten.
- A successor activation increments the activation sequence, names the exact
  prior activation signing digest, advances the canonical block, and never
  moves issuance time backward.

## Ceremony record (2026-08-02)

The owner ceremony of 2026-08-02 executed this runbook with two recorded
deviations, both owner-approved in writing: the three authority keys were
generated on the owner's Mac rather than three separate offline devices, and
the production canary was waived before signing. The production policy
(digest `0xf144…943c`), the sequence-1 signed activation, both independent
verification results, and the owner decision evidence live in
`release/contract-activations/`; the engine pins the policy digest as its
trust root. Independent human review of the second verifier implementation
remains outstanding. A successor-policy ceremony with separated custody, and
a retroactive canary run, remain advisable next steps.
